// Copyright 2017 Zephyr Pellerin
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! When serializing or deserializing S-expression goes wrong.

use std::error;
use std::fmt::{self, Debug, Display};
use std::io;
use std::result;

use serde::de;
use serde::ser;

/// This type represents all possible errors that can occur when serializing or
/// deserializing S-expression data.
pub struct Error {
    /// This `Box` allows us to keep the size of `Error` as small as possible. A
    /// larger `Error` type was substantially slower due to all the functions
    /// that pass around `Result<T, Error>`.
    err: Box<ErrorImpl>,
}

/// Alias for a `Result` with the error type `sexpr::Error`.
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// One-based line number at which the error was detected.
    ///
    /// Characters in the first line of the input (before the first newline
    /// character) are in line 1.
    pub fn line(&self) -> usize {
        self.err.line
    }

    /// One-based column number at which the error was detected.
    ///
    /// The first character in the input and any characters immediately
    /// following a newline character are in column 1.
    ///
    /// Note that errors may occur in column 0, for example if a read from an IO
    /// stream fails immediately following a previously read newline character.
    pub fn column(&self) -> usize {
        self.err.column
    }

    /// Categorizes the cause of this error.
    ///
    /// - `Category::Io` - failure to read or write bytes on an IO stream
    /// - `Category::Syntax` - input that is not syntactically valid S-expression
    /// - `Category::Data` - input data that is semantically incorrect
    /// - `Category::Eof` - unexpected end of the input data
    pub fn classify(&self) -> Category {
        match self.err.code {
            ErrorCode::Message(_) => Category::Data,
            ErrorCode::Io(_) => Category::Io,
            ErrorCode::EofWhileParsingList
            | ErrorCode::EofWhileParsingAlist
            | ErrorCode::EofWhileParsingString
            | ErrorCode::EofWhileParsingValue => Category::Eof,
            ErrorCode::ExpectedPairDot
            | ErrorCode::ExpectedListEltOrEnd
            | ErrorCode::ExpectedPairOrEnd
            | ErrorCode::ExpectedList
            | ErrorCode::ExpectedSomeIdent
            | ErrorCode::ExpectedSomeValue
            | ErrorCode::ExpectedSomeString
            | ErrorCode::InvalidEscape
            | ErrorCode::InvalidNumber
            | ErrorCode::NumberOutOfRange
            | ErrorCode::InvalidUnicodeCodePoint
            | ErrorCode::KeyMustBeAString
            | ErrorCode::LoneLeadingSurrogateInHexEscape
            | ErrorCode::TrailingCharacters
            | ErrorCode::UnexpectedEndOfHexEscape
            | ErrorCode::RecursionLimitExceeded => Category::Syntax,
        }
    }

    /// Returns true if this error was caused by a failure to read or write
    /// bytes on an IO stream.
    pub fn is_io(&self) -> bool {
        self.classify() == Category::Io
    }

    /// Returns true if this error was caused by input that was not
    /// syntactically valid S-expression.
    pub fn is_syntax(&self) -> bool {
        self.classify() == Category::Syntax
    }

    /// Returns true if this error was caused by input data that was
    /// semantically incorrect.
    ///
    /// For example, S-expression containing a number is semantically incorrect when the
    /// type being deserialized into holds a String.
    pub fn is_data(&self) -> bool {
        self.classify() == Category::Data
    }

    /// Returns true if this error was caused by prematurely reaching the end of
    /// the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    pub fn is_eof(&self) -> bool {
        self.classify() == Category::Eof
    }
}

/// Categorizes the cause of a `sexpr::Error`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Category {
    /// The error was caused by a failure to read or write bytes on an IO
    /// stream.
    Io,

    /// The error was caused by input that was not a syntactically valid S-expression.
    Syntax,

    /// The error was caused by input data that was semantically incorrect.
    ///
    /// For example, an S-expression containing a number is semantically incorrect when the
    /// type being deserialized into holds a String.
    Data,

    /// The error was caused by prematurely reaching the end of the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    Eof,
}

impl From<Error> for io::Error {
    /// Convert a `sexpr::Error` into an `io::Error`.
    ///
    /// Sexprs syntax and data errors are turned into `InvalidData` IO errors.
    /// EOF errors are turned into `UnexpectedEof` IO errors.
    ///
    /// ```rust,ignore
    /// use std::io;
    ///
    /// enum MyError {
    ///     Io(io::Error),
    ///     Sexp(sexpr::Error),
    /// }
    ///
    /// impl From<sexpr::Error> for MyError {
    ///     fn from(err: sexpr::Error) -> MyError {
    ///         use sexpr::error::Category;
    ///         match err.classify() {
    ///             Category::Io => {
    ///                 MyError::Io(err.into())
    ///             }
    ///             Category::Syntax | Category::Data | Category::Eof => {
    ///                 MyError::Sexp(err)
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    fn from(j: Error) -> Self {
        if let ErrorCode::Io(err) = j.err.code {
            err
        } else {
            match j.classify() {
                Category::Io => unreachable!(),
                Category::Syntax | Category::Data => io::Error::new(io::ErrorKind::InvalidData, j),
                Category::Eof => io::Error::new(io::ErrorKind::UnexpectedEof, j),
            }
        }
    }
}

#[derive(Debug)]
struct ErrorImpl {
    code: ErrorCode,
    line: usize,
    column: usize,
}

// Not public API. Should be pub(crate).
#[doc(hidden)]
#[derive(Debug)]
pub enum ErrorCode {
    /// Catchall for syntax error messages
    Message(String),

    /// Some IO error occurred while serializing or deserializing.
    Io(io::Error),

    /// EOF while parsing a list.
    EofWhileParsingList,

    /// EOF while parsing an object.
    EofWhileParsingAlist,

    /// EOF while parsing a string.
    EofWhileParsingString,

    /// EOF while parsing a S-expression value.
    EofWhileParsingValue,

    /// Expected this character to be a `'.'`.
    ExpectedPairDot,

    /// Expected this character to be either a space or `')'``.
    ExpectedListEltOrEnd,

    /// Expected this character to be either a `'.'` or a `')'`.
    ExpectedPairOrEnd,

    /// Expected this character to be `'('`
    ExpectedList,

    /// Expected to parse either a `#t`, `#f`, or a `#nil`.
    ExpectedSomeIdent,

    /// Expected this character to start an S-expression value.
    ExpectedSomeValue,

    /// Expected this character to start an S-expression string, symbol or keyword.
    ExpectedSomeString,

    /// Invalid hex escape code.
    InvalidEscape,

    /// Invalid number.
    InvalidNumber,

    /// Number is bigger than the maximum value of its type.
    NumberOutOfRange,

    /// Invalid unicode code point.
    InvalidUnicodeCodePoint,

    /// Object key is not a string.
    KeyMustBeAString,

    /// Lone leading surrogate in hex escape.
    LoneLeadingSurrogateInHexEscape,

    /// S-expression has non-whitespace trailing characters after the value.
    TrailingCharacters,

    /// Unexpected end of hex excape.
    UnexpectedEndOfHexEscape,

    /// Encountered nesting of S-expression maps and arrays more than 128 layers deep.
    RecursionLimitExceeded,
}

impl Error {
    // Not public API. Should be pub(crate).
    #[doc(hidden)]
    pub fn syntax(code: ErrorCode, line: usize, column: usize) -> Self {
        Error {
            err: Box::new(ErrorImpl { code, line, column }),
        }
    }

    // Not public API. Should be pub(crate).
    #[doc(hidden)]
    pub fn io(error: io::Error) -> Self {
        Error {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Io(error),
                line: 0,
                column: 0,
            }),
        }
    }

    // Not public API. Should be pub(crate).
    #[doc(hidden)]
    pub fn fix_position<F>(self, f: F) -> Self
    where
        F: FnOnce(ErrorCode) -> Error,
    {
        if self.err.line == 0 {
            f(self.err.code)
        } else {
            self
        }
    }
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorCode::Message(ref msg) => f.write_str(msg),
            ErrorCode::Io(ref err) => Display::fmt(err, f),
            ErrorCode::EofWhileParsingList => f.write_str("EOF while parsing a list"),
            ErrorCode::EofWhileParsingAlist => f.write_str("EOF while parsing an alist"),
            ErrorCode::EofWhileParsingString => f.write_str("EOF while parsing a string"),
            ErrorCode::EofWhileParsingValue => f.write_str("EOF while parsing a value"),
            ErrorCode::ExpectedPairDot => f.write_str("expected `.`"),
            ErrorCode::ExpectedListEltOrEnd => f.write_str("expected ` ` or `)`"),
            ErrorCode::ExpectedPairOrEnd => f.write_str("expected `.` or `)`"),
            ErrorCode::ExpectedList => f.write_str("expected `(`"),
            ErrorCode::ExpectedSomeIdent => f.write_str("expected ident"),
            ErrorCode::ExpectedSomeValue => f.write_str("expected value"),
            ErrorCode::ExpectedSomeString => f.write_str("expected string"),
            ErrorCode::InvalidEscape => f.write_str("invalid escape"),
            ErrorCode::InvalidNumber => f.write_str("invalid number"),
            ErrorCode::NumberOutOfRange => f.write_str("number out of range"),
            ErrorCode::InvalidUnicodeCodePoint => f.write_str("invalid unicode code point"),
            ErrorCode::KeyMustBeAString => f.write_str("key must be a string"),
            ErrorCode::LoneLeadingSurrogateInHexEscape => {
                f.write_str("lone leading surrogate in hex escape")
            }
            ErrorCode::TrailingCharacters => f.write_str("trailing characters"),
            ErrorCode::UnexpectedEndOfHexEscape => f.write_str("unexpected end of hex escape"),
            ErrorCode::RecursionLimitExceeded => f.write_str("recursion limit exceeded"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self.err.code {
            ErrorCode::Io(ref err) => error::Error::description(err),
            _ => {
                // If you want a better message, use Display::fmt or to_string().
                "S-expression error"
            }
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match self.err.code {
            ErrorCode::Io(ref err) => Some(err),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&*self.err, f)
    }
}

impl Display for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.line == 0 {
            Display::fmt(&self.code, f)
        } else {
            write!(
                f,
                "{} at line {} column {}",
                self.code, self.line, self.column
            )
        }
    }
}

// Remove two layers of verbosity from the debug representation. Humans often
// end up seeing this representation because it is what unwrap() shows.
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&*self.err, f)
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Error {
        Error {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Message(msg.to_string()),
                line: 0,
                column: 0,
            }),
        }
    }
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Error {
        Error {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Message(msg.to_string()),
                line: 0,
                column: 0,
            }),
        }
    }
}

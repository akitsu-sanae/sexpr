// Copyright 2017 Zephyr Pellerin
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Deserialize S-expression data to a Rust data structure.

use std::io;
use std::marker::PhantomData;
use std::{i32, u64};

use super::error::{Error, ErrorCode, Result};
use serde::de::{self, Unexpected};
use serde::forward_to_deserialize_any;

use crate::read::{self, Reference};

use crate::atom::Atom;
pub use crate::read::{IoRead, Read, SliceRead, StrRead};

//////////////////////////////////////////////////////////////////////////////

/// A structure that deserializes S-expressions into Rust values.
pub struct Deserializer<R> {
    read: R,
    str_buf: Vec<u8>,
    remaining_depth: u8,
}

impl<'de, R> Deserializer<R>
where
    R: read::Read<'de>,
{
    /// Create a S-expression deserializer from one of the possible sexpr input
    /// sources.
    ///
    /// Typically it is more convenient to use one of these methods instead:
    ///
    ///   - Deserializer::from_str
    ///   - Deserializer::from_bytes
    ///   - Deserializer::from_reader
    pub fn new(read: R) -> Self {
        Deserializer {
            read,
            str_buf: Vec::with_capacity(128),
            remaining_depth: 128,
        }
    }
}

impl<R> Deserializer<read::IoRead<R>>
where
    R: io::Read,
{
    /// Creates a S-expression deserializer from an `io::Read`.
    pub fn from_reader(reader: R) -> Self {
        Deserializer::new(read::IoRead::new(reader))
    }
}

impl<'a> Deserializer<read::SliceRead<'a>> {
    /// Creates a S-expression deserializer from a `&[u8]`.
    pub fn from_slice(bytes: &'a [u8]) -> Self {
        Deserializer::new(read::SliceRead::new(bytes))
    }
}

impl<'a> Deserializer<read::StrRead<'a>> {
    /// Creates a S-expression deserializer from a `&str`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &'a str) -> Self {
        Deserializer::new(read::StrRead::new(s))
    }
}

macro_rules! overflow {
    ($a:ident * 10 + $b:ident, $c:expr) => {
        $a >= $c / 10 && ($a > $c / 10 || $b > $c % 10)
    };
}

enum Number {
    F64(f64),
    U64(u64),
    I64(i64),
}

impl Number {
    fn visit<'de, V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            Number::F64(x) => visitor.visit_f64(x),
            Number::U64(x) => visitor.visit_u64(x),
            Number::I64(x) => visitor.visit_i64(x),
        }
    }
}

impl<'de, R: Read<'de>> Deserializer<R> {
    /// The `Deserializer::end` method should be called after a value has been fully deserialized.
    /// This allows the `Deserializer` to validate that the input stream is at the end or that it
    /// only has trailing whitespace.
    pub fn end(&mut self) -> Result<()> {
        match self.parse_whitespace()? {
            Some(_) => Err(self.peek_error(ErrorCode::TrailingCharacters)),
            None => Ok(()),
        }
    }

    /// Turn a Sexp deserializer into an iterator over values of type T.
    // TODO: Deserializer<R> cannot implement `IntoIterator`, as the
    // returned iterator is generic over `T`.
    #[allow(clippy::should_implement_trait)]
    pub fn into_iter<T>(self) -> StreamDeserializer<'de, R, T>
    where
        T: de::Deserialize<'de>,
    {
        // This cannot be an implementation of std::iter::IntoIterator because
        // we need the caller to choose what T is.
        let offset = self.read.byte_offset();
        StreamDeserializer {
            de: self,
            offset,
            output: PhantomData,
            lifetime: PhantomData,
        }
    }

    fn peek(&mut self) -> Result<Option<u8>> {
        self.read.peek().map_err(Error::io)
    }

    fn peek_or_null(&mut self) -> Result<u8> {
        Ok(self.peek()?.unwrap_or(b'\x00'))
    }

    fn eat_char(&mut self) {
        self.read.discard();
    }

    fn next_char(&mut self) -> Result<Option<u8>> {
        self.read.next().map_err(Error::io)
    }

    fn next_char_or_null(&mut self) -> Result<u8> {
        Ok(self.next_char()?.unwrap_or(b'\x00'))
    }

    /// Error caused by a byte from next_char().
    fn error(&mut self, reason: ErrorCode) -> Error {
        let pos = self.read.position();
        Error::syntax(reason, pos.line, pos.column)
    }

    /// Error caused by a byte from peek().
    fn peek_error(&mut self, reason: ErrorCode) -> Error {
        let pos = self.read.peek_position();
        Error::syntax(reason, pos.line, pos.column)
    }

    /// Returns the first non-whitespace byte without consuming it, or `None` if
    /// EOF is encountered.
    fn parse_whitespace(&mut self) -> Result<Option<u8>> {
        loop {
            match self.peek()? {
                Some(b' ') | Some(b'\n') | Some(b'\t') | Some(b'\r') => {
                    self.eat_char();
                }
                other => {
                    return Ok(other);
                }
            }
        }
    }

    fn parse_value<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let peek = match self.parse_whitespace()? {
            Some(b) => b,
            None => {
                return Err(self.peek_error(ErrorCode::EofWhileParsingValue));
            }
        };

        let value = match peek {
            b'#' => {
                self.eat_char();
                match self.next_char()? {
                    Some(b't') => visitor.visit_bool(true),
                    Some(b'f') => visitor.visit_bool(false),
                    Some(b'n') => {
                        self.parse_ident(b"il")?;
                        visitor.visit_bool(true)
                    }
                    Some(_) => Err(self.peek_error(ErrorCode::ExpectedSomeIdent)),
                    None => Err(self.peek_error(ErrorCode::EofWhileParsingValue)),
                }
            }
            b'-' => {
                self.eat_char();
                self.parse_integer(false)?.visit(visitor)
            }
            b'0'..=b'9' => self.parse_integer(true)?.visit(visitor),
            b'"' => {
                self.eat_char();
                self.str_buf.clear();
                match self.read.parse_str(&mut self.str_buf)? {
                    Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
                    Reference::Copied(s) => visitor.visit_str(s),
                }
            }
            b'(' => {
                self.remaining_depth -= 1;
                if self.remaining_depth == 0 {
                    return Err(self.peek_error(ErrorCode::RecursionLimitExceeded));
                }

                self.eat_char();
                let ret = visitor.visit_seq(SeqAccess::new(self));

                self.remaining_depth += 1;

                self.parse_whitespace()?;

                match (ret, self.end_seq()) {
                    (Ok(ret), Ok(())) => Ok(ret),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                self.str_buf.clear();
                match self.read.parse_symbol(&mut self.str_buf)? {
                    Reference::Borrowed(s) => visitor.visit_newtype_struct(Atom::from_str(s)),
                    Reference::Copied(s) => visitor.visit_newtype_struct(Atom::from_str(s)),
                }
            }
            _ => Err(self.peek_error(ErrorCode::ExpectedSomeValue)),
        };

        match value {
            Ok(value) => Ok(value),
            // The de::Error and From<de::value::Error> impls both create errors
            // with unknown line and column. Fill in the position here by
            // looking at the current index in the input. There is no way to
            // tell whether this should call `error` or `peek_error` so pick the
            // one that seems correct more often. Worst case, the position is
            // off by one character.
            Err(err) => Err(err.fix_position(|code| self.error(code))),
        }
    }

    fn parse_ident(&mut self, ident: &[u8]) -> Result<()> {
        for c in ident {
            if Some(*c) != self.next_char()? {
                return Err(self.error(ErrorCode::ExpectedSomeIdent));
            }
        }

        Ok(())
    }

    fn parse_integer(&mut self, pos: bool) -> Result<Number> {
        match self.next_char_or_null()? {
            b'0' => {
                // There can be only one leading '0'.
                match self.peek_or_null()? {
                    b'0'..=b'9' => Err(self.peek_error(ErrorCode::InvalidNumber)),
                    _ => self.parse_number(pos, 0),
                }
            }
            c @ b'1'..=b'9' => {
                let mut res = u64::from(c - b'0');

                loop {
                    match self.peek_or_null()? {
                        c @ b'0'..=b'9' => {
                            self.eat_char();
                            let digit = u64::from(c - b'0');

                            // We need to be careful with overflow. If we can, try to keep the
                            // number as a `u64` until we grow too large. At that point, switch to
                            // parsing the value as a `f64`.
                            if overflow!(res * 10 + digit, u64::MAX) {
                                return Ok(Number::F64(self.parse_long_integer(
                                    pos, res, 1, // res * 10^1
                                )?));
                            }

                            res = res * 10 + digit;
                        }
                        _ => {
                            return self.parse_number(pos, res);
                        }
                    }
                }
            }
            _ => Err(self.error(ErrorCode::InvalidNumber)),
        }
    }

    fn parse_long_integer(
        &mut self,
        pos: bool,
        significand: u64,
        mut exponent: i32,
    ) -> Result<f64> {
        loop {
            match self.peek_or_null()? {
                b'0'..=b'9' => {
                    self.eat_char();
                    // This could overflow... if your integer is gigabytes long.
                    // Ignore that possibility.
                    exponent += 1;
                }
                b'.' => {
                    return self.parse_decimal(pos, significand, exponent);
                }
                // b'e' | b'E' => {
                //     return self.parse_exponent(pos, significand, exponent);
                // }
                _ => {
                    return self.f64_from_parts(pos, significand, exponent);
                }
            }
        }
    }

    fn parse_number(&mut self, pos: bool, significand: u64) -> Result<Number> {
        Ok(match self.peek_or_null()? {
            b'.' => Number::F64(self.parse_decimal(pos, significand, 0)?),
            // b'e' | b'E' => Number::F64(try!(self.parse_exponent(pos, significand, 0))),
            _ => {
                if pos {
                    Number::U64(significand)
                } else {
                    let neg = (significand as i64).wrapping_neg();

                    // Convert into a float if we underflow.
                    if neg > 0 {
                        Number::F64(-(significand as f64))
                    } else {
                        Number::I64(neg)
                    }
                }
            }
        })
    }

    fn parse_decimal(&mut self, pos: bool, mut significand: u64, mut exponent: i32) -> Result<f64> {
        self.eat_char();

        let mut at_least_one_digit = false;
        while let c @ b'0'..=b'9' = self.peek_or_null()? {
            self.eat_char();
            let digit = u64::from(c - b'0');
            at_least_one_digit = true;

            if overflow!(significand * 10 + digit, u64::MAX) {
                // The next multiply/add would overflow, so just ignore all
                // further digits.
                while let b'0'..=b'9' = self.peek_or_null()? {
                    self.eat_char();
                }
                break;
            }

            significand = significand * 10 + digit;
            exponent -= 1;
        }

        if !at_least_one_digit {
            return Err(self.peek_error(ErrorCode::InvalidNumber));
        }

        match self.peek_or_null()? {
            // b'e' | b'E' => self.parse_exponent(pos, significand, exponent),
            _ => self.f64_from_parts(pos, significand, exponent),
        }
    }

    fn f64_from_parts(&mut self, pos: bool, significand: u64, mut exponent: i32) -> Result<f64> {
        let mut f = significand as f64;
        loop {
            match POW10.get(exponent.abs() as usize) {
                Some(&pow) => {
                    if exponent >= 0 {
                        f *= pow;
                        if f.is_infinite() {
                            return Err(self.error(ErrorCode::NumberOutOfRange));
                        }
                    } else {
                        f /= pow;
                    }
                    break;
                }
                None => {
                    if f == 0.0 {
                        break;
                    }
                    if exponent >= 0 {
                        return Err(self.error(ErrorCode::NumberOutOfRange));
                    }
                    f /= 1e308;
                    exponent += 308;
                }
            }
        }
        Ok(if pos { f } else { -f })
    }

    fn end_seq(&mut self) -> Result<()> {
        match self.parse_whitespace()? {
            Some(b')') => {
                self.eat_char();
                Ok(())
            }
            Some(_) => Err(self.peek_error(ErrorCode::TrailingCharacters)),
            None => Err(self.peek_error(ErrorCode::EofWhileParsingList)),
        }
    }
}

#[rustfmt::skip]
static POW10: [f64; 309] =
    [1e000, 1e001, 1e002, 1e003, 1e004, 1e005, 1e006, 1e007, 1e008, 1e009,
     1e010, 1e011, 1e012, 1e013, 1e014, 1e015, 1e016, 1e017, 1e018, 1e019,
     1e020, 1e021, 1e022, 1e023, 1e024, 1e025, 1e026, 1e027, 1e028, 1e029,
     1e030, 1e031, 1e032, 1e033, 1e034, 1e035, 1e036, 1e037, 1e038, 1e039,
     1e040, 1e041, 1e042, 1e043, 1e044, 1e045, 1e046, 1e047, 1e048, 1e049,
     1e050, 1e051, 1e052, 1e053, 1e054, 1e055, 1e056, 1e057, 1e058, 1e059,
     1e060, 1e061, 1e062, 1e063, 1e064, 1e065, 1e066, 1e067, 1e068, 1e069,
     1e070, 1e071, 1e072, 1e073, 1e074, 1e075, 1e076, 1e077, 1e078, 1e079,
     1e080, 1e081, 1e082, 1e083, 1e084, 1e085, 1e086, 1e087, 1e088, 1e089,
     1e090, 1e091, 1e092, 1e093, 1e094, 1e095, 1e096, 1e097, 1e098, 1e099,
     1e100, 1e101, 1e102, 1e103, 1e104, 1e105, 1e106, 1e107, 1e108, 1e109,
     1e110, 1e111, 1e112, 1e113, 1e114, 1e115, 1e116, 1e117, 1e118, 1e119,
     1e120, 1e121, 1e122, 1e123, 1e124, 1e125, 1e126, 1e127, 1e128, 1e129,
     1e130, 1e131, 1e132, 1e133, 1e134, 1e135, 1e136, 1e137, 1e138, 1e139,
     1e140, 1e141, 1e142, 1e143, 1e144, 1e145, 1e146, 1e147, 1e148, 1e149,
     1e150, 1e151, 1e152, 1e153, 1e154, 1e155, 1e156, 1e157, 1e158, 1e159,
     1e160, 1e161, 1e162, 1e163, 1e164, 1e165, 1e166, 1e167, 1e168, 1e169,
     1e170, 1e171, 1e172, 1e173, 1e174, 1e175, 1e176, 1e177, 1e178, 1e179,
     1e180, 1e181, 1e182, 1e183, 1e184, 1e185, 1e186, 1e187, 1e188, 1e189,
     1e190, 1e191, 1e192, 1e193, 1e194, 1e195, 1e196, 1e197, 1e198, 1e199,
     1e200, 1e201, 1e202, 1e203, 1e204, 1e205, 1e206, 1e207, 1e208, 1e209,
     1e210, 1e211, 1e212, 1e213, 1e214, 1e215, 1e216, 1e217, 1e218, 1e219,
     1e220, 1e221, 1e222, 1e223, 1e224, 1e225, 1e226, 1e227, 1e228, 1e229,
     1e230, 1e231, 1e232, 1e233, 1e234, 1e235, 1e236, 1e237, 1e238, 1e239,
     1e240, 1e241, 1e242, 1e243, 1e244, 1e245, 1e246, 1e247, 1e248, 1e249,
     1e250, 1e251, 1e252, 1e253, 1e254, 1e255, 1e256, 1e257, 1e258, 1e259,
     1e260, 1e261, 1e262, 1e263, 1e264, 1e265, 1e266, 1e267, 1e268, 1e269,
     1e270, 1e271, 1e272, 1e273, 1e274, 1e275, 1e276, 1e277, 1e278, 1e279,
     1e280, 1e281, 1e282, 1e283, 1e284, 1e285, 1e286, 1e287, 1e288, 1e289,
     1e290, 1e291, 1e292, 1e293, 1e294, 1e295, 1e296, 1e297, 1e298, 1e299,
     1e300, 1e301, 1e302, 1e303, 1e304, 1e305, 1e306, 1e307, 1e308];

impl<'de, 'a, R: Read<'de>> de::Deserializer<'de> for &'a mut Deserializer<R> {
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.parse_value(visitor)
    }

    /// Parses a `nil` as a None, and any other values as a `Some(...)`.
    #[inline]
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_whitespace()? {
            Some(b'n') => {
                self.eat_char();
                self.parse_ident(b"il")?;
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    /// Parses a newtype struct as the underlying value.
    #[inline]
    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    /// Parses an enum as an s-expression like `(($KEY1 $VALUE1) ($KEY2 $VALUE2))` where $VALUE
    /// is either a direct Sexp or a sequence.
    #[inline]
    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_whitespace()? {
            Some(b'(') => {
                self.remaining_depth -= 1;
                if self.remaining_depth == 0 {
                    return Err(self.peek_error(ErrorCode::RecursionLimitExceeded));
                }

                self.eat_char();
                let value = visitor.visit_enum(VariantAccess::new(self))?;

                self.remaining_depth += 1;

                match self.parse_whitespace()? {
                    Some(b')') => {
                        self.eat_char();
                        Ok(value)
                    }
                    Some(_) => Err(self.error(ErrorCode::ExpectedSomeValue)),
                    None => Err(self.error(ErrorCode::EofWhileParsingAlist)),
                }
            }
            Some(b'"') => visitor.visit_enum(UnitVariantAccess::new(self)),
            // TODO: ATOMS BROKEN
            Some(_) => Err(self.peek_error(ErrorCode::ExpectedSomeValue)),
            None => Err(self.peek_error(ErrorCode::EofWhileParsingValue)),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_whitespace()? {
            Some(b'"') => {
                self.eat_char();
                self.str_buf.clear();
                match self.read.parse_str_raw(&mut self.str_buf)? {
                    Reference::Borrowed(b) => visitor.visit_borrowed_bytes(b),
                    Reference::Copied(b) => visitor.visit_bytes(b),
                }
            }
            _ => self.deserialize_any(visitor),
        }
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let peek = match self.parse_whitespace()? {
            Some(b) => b,
            None => {
                return Err(self.peek_error(ErrorCode::EofWhileParsingValue));
            }
        };
        let value = match peek {
            b'(' => {
                self.eat_char();
                let ret = visitor.visit_map(MapAccess::new(self))?;
                self.end_seq()?;
                Ok(ret)
            }
            _ => Err(self.peek_error(ErrorCode::ExpectedList)),
        };
        match value {
            Ok(value) => Ok(value),
            Err(err) => Err(err.fix_position(|code| self.error(code))),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string unit
            unit_struct seq tuple tuple_struct map identifier ignored_any
    }
}

// POSSIBLY BROKEN --------------------------------------------------------
struct SeqAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
    first: bool,
}

impl<'a, R: 'a> SeqAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        SeqAccess { de, first: true }
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::SeqAccess<'de> for SeqAccess<'a, R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.de.peek()? {
            Some(b')') => {
                return Ok(None);
            }
            Some(b' ') => {
                self.de.eat_char();
            }
            Some(_) => {
                self.de.parse_whitespace()?;
                if self.first {
                    self.first = false;
                } else {
                    return Err(self.de.peek_error(ErrorCode::ExpectedListEltOrEnd));
                }
            }
            None => {
                return Err(self.de.peek_error(ErrorCode::EofWhileParsingList));
            }
        }

        if self.de.peek()?.unwrap() == b')' {
            Ok(None)
        } else {
            seed.deserialize(&mut *self.de).map(Some)
        }
    }
}

// END POSSIBLY BROKEN --------------------------------------------------------

/// Deserialize an association list (alist) as a map.
///
/// An alist has the a shape of `((key1 . v1) (key2 . v2) ...)`. Note
/// that the keys may be either strings or symbols. When the values
/// are themselves lists, the dot may be omitted, for example, the
/// following two expressions are identical:
///
/// ```lisp
/// ((key some values))
/// ```
///
/// ```lisp
/// ((key . (some values)))
/// ```
struct MapAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> MapAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        MapAccess { de }
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::MapAccess<'de> for MapAccess<'a, R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.de.parse_whitespace()? {
            Some(b')') => return Ok(None),
            Some(b'(') => {
                self.de.eat_char();
            }
            Some(_) => {
                return Err(self.de.peek_error(ErrorCode::ExpectedList));
            }
            None => {
                return Err(self.de.peek_error(ErrorCode::EofWhileParsingAlist));
            }
        };
        seed.deserialize(MapKey { de: &mut *self.de }).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = match self.de.parse_whitespace()? {
            Some(b'.') => {
                self.de.eat_char();
                seed.deserialize(&mut *self.de)?
            }
            Some(_) => seed.deserialize(MapSeqValue::new(self.de))?,
            None => return Err(self.de.peek_error(ErrorCode::EofWhileParsingAlist)),
        };
        match self.de.parse_whitespace()? {
            Some(b')') => {
                self.de.eat_char();
                Ok(value)
            }
            Some(_) => Err(self.de.peek_error(ErrorCode::TrailingCharacters)),
            None => Err(self.de.peek_error(ErrorCode::EofWhileParsingAlist)),
        }
    }
}

// To be used after consuming the initial open parenthesis of an
// association list item.
struct MapKey<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'de, 'a, R> de::Deserializer<'de> for MapKey<'a, R>
where
    R: Read<'de>,
{
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        // TODO: this is duplicated from `parse_value`. Find out about
        // the relationship between symbols and newtype structs.
        match self.de.parse_whitespace()? {
            Some(b) => match b {
                b'"' => {
                    self.de.eat_char();
                    self.de.str_buf.clear();
                    match self.de.read.parse_str(&mut self.de.str_buf)? {
                        Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
                        Reference::Copied(s) => visitor.visit_str(s),
                    }
                }
                b'a'..=b'z' | b'A'..=b'Z' => {
                    self.de.str_buf.clear();
                    match self.de.read.parse_symbol(&mut self.de.str_buf)? {
                        Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
                        Reference::Copied(s) => visitor.visit_str(s),
                    }
                }
                _ => Err(self.de.peek_error(ErrorCode::ExpectedSomeIdent)), // TODO: inaccurate error code
            },
            None => Err(self.de.peek_error(ErrorCode::EofWhileParsingAlist)),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string unit unit_struct seq tuple tuple_struct map
        bytes byte_buf option newtype_struct enum
        struct identifier ignored_any
    }
}

// To be used after consuming the field name (key) of an alist item
struct MapSeqValue<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> MapSeqValue<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        Self { de }
    }
}

impl<'de, 'a, R> de::Deserializer<'de> for MapSeqValue<'a, R>
where
    R: Read<'de>,
{
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self.de))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string unit unit_struct seq tuple tuple_struct map
        bytes byte_buf option newtype_struct enum
        struct identifier ignored_any
    }
}

struct VariantAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> VariantAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        VariantAccess { de }
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::EnumAccess<'de> for VariantAccess<'a, R> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, _seed: V) -> Result<(V::Value, Self)>
    where
        V: de::DeserializeSeed<'de>,
    {
        unimplemented!()
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::VariantAccess<'de> for VariantAccess<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        de::Deserialize::deserialize(self.de)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_any(self.de, visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_any(self.de, visitor)
    }
}

struct UnitVariantAccess<'a, R: 'a> {
    de: &'a mut Deserializer<R>,
}

impl<'a, R: 'a> UnitVariantAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>) -> Self {
        UnitVariantAccess { de }
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::EnumAccess<'de> for UnitVariantAccess<'a, R> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(&mut *self.de)?;
        Ok((variant, self))
    }
}

impl<'de, 'a, R: Read<'de> + 'a> de::VariantAccess<'de> for UnitVariantAccess<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"newtype variant",
        ))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"tuple variant",
        ))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"struct variant",
        ))
    }
}

//////////////////////////////////////////////////////////////////////////////

/// Iterator that deserializes a stream into multiple Sexp values.
///
/// A stream deserializer can be created from any SExp deserializer using the
/// `Deserializer::into_iter` method.
///
/// The data must consist of Sexp lists optionally separated by whitespace. A
/// null, boolean, number, or string at the top level are all
/// errors.
///
/// ```
/// use sexpr::{Deserializer, Sexp};
///
/// fn main() {
///     let data = "(a 1) () (1 2 3)";
///
///     let stream = Deserializer::from_str(data).into_iter::<Sexp>();
///
///     for value in stream {
///         println!("{}", value.unwrap());
///     }
/// }
/// ```
pub struct StreamDeserializer<'de, R, T> {
    de: Deserializer<R>,
    offset: usize,
    output: PhantomData<T>,
    lifetime: PhantomData<&'de ()>,
}

impl<'de, R, T> StreamDeserializer<'de, R, T>
where
    R: read::Read<'de>,
    T: de::Deserialize<'de>,
{
    /// Create a sexp-stream deserializer from one of the possible sexpr
    /// input sources.
    ///
    /// Typically it is more convenient to use one of these methods instead:
    ///
    ///   - Deserializer::from_str(...).into_iter()
    ///   - Deserializer::from_bytes(...).into_iter()
    ///   - Deserializer::from_reader(...).into_iter()
    pub fn new(read: R) -> Self {
        let offset = read.byte_offset();
        StreamDeserializer {
            de: Deserializer::new(read),
            offset,
            output: PhantomData,
            lifetime: PhantomData,
        }
    }

    /// Returns the number of bytes so far deserialized into a successful `T`.
    ///
    /// If a stream deserializer returns an EOF error, new data can be joined to
    /// `old_data[stream.byte_offset()..]` to try again.
    pub fn byte_offset(&self) -> usize {
        self.offset
    }
}

impl<'de, R, T> Iterator for StreamDeserializer<'de, R, T>
where
    R: Read<'de>,
    T: de::Deserialize<'de>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Result<T>> {
        // skip whitespaces, if any
        // this helps with trailing whitespaces, since whitespaces between
        // values are handled for us.
        match self.de.parse_whitespace() {
            Ok(None) => {
                self.offset = self.de.read.byte_offset();
                None
            }
            Ok(Some(b'(')) => {
                self.offset = self.de.read.byte_offset();
                let result = de::Deserialize::deserialize(&mut self.de);
                if result.is_ok() {
                    self.offset = self.de.read.byte_offset();
                }
                Some(result)
            }
            Ok(Some(_)) => Some(Err(self.de.peek_error(ErrorCode::ExpectedList))),
            Err(e) => Some(Err(e)),
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

fn from_trait<'de, R, T>(read: R) -> Result<T>
where
    R: Read<'de>,
    T: de::Deserialize<'de>,
{
    let mut de = Deserializer::new(read);
    let value = de::Deserialize::deserialize(&mut de)?;

    // Make sure the whole stream has been consumed.
    de.end()?;
    Ok(value)
}

/// Deserialize an instance of type `T` from an IO stream of S-expressions.
///
/// # Errors
///
/// This conversion can fail if the structure of the input does not match the
/// structure expected by `T`, for example if `T` is a struct type but the input
/// contains something other than a Sexp map. It can also fail if the structure
/// is correct but `T`'s implementation of `Deserialize` decides that something
/// is wrong with the data, for example required struct fields are missing from
/// the Sexp map or some number is too big to fit in the expected primitive
/// type.
///
/// ```
/// use serde_derive::Deserialize;
///
/// use std::error::Error;
/// use std::fs::File;
/// use std::path::Path;
///
/// #[derive(Deserialize, Debug)]
/// struct User {
///     fingerprint: String,
///     location: String,
/// }
///
/// fn read_user_from_file<P: AsRef<Path>>(path: P) -> Result<User, Box<Error>> {
///     // Open the file in read-only mode.
///     let file = File::open(path)?;
///
///     // Read the Sexp contents of the file as an instance of `User`.
///     let u = sexpr::from_reader(file)?;
///
///     // Return the `User`.
///     Ok(u)
/// }
///
/// fn main() {
/// # }
/// # fn fake_main() {
///     let u = read_user_from_file("test.scm").unwrap();
///     println!("{:#?}", u);
/// }
/// ```
pub fn from_reader<R, T>(rdr: R) -> Result<T>
where
    R: io::Read,
    T: de::DeserializeOwned,
{
    from_trait(read::IoRead::new(rdr))
}

/// Deserialize an instance of type `T` from bytes of an S-expression.
///
/// # Errors
///
/// This conversion can fail if the structure of the input does not match the
/// structure expected by `T`, for example if `T` is a struct type but the input
/// contains something other than a S-expression "map". It can also fail if the
/// structure is correct but `T`'s implementation of `Deserialize` decides that
/// something is wrong with the data, for example required struct fields are
/// missing from the S-expression or some number is too big to fit in the expected
/// primitive type.
///
/// ```
/// use serde_derive::Deserialize;
///
/// #[derive(Deserialize, Debug)]
/// struct User {
///     fingerprint: String,
///     location: String,
/// }
///
/// fn main() {
///     // The type of `s` is `&[u8]`
///     let s = b"(
///                 (fingerprint . \"0xF9BA143B95FF6D82\")
///                 (location . \"Menlo Park, CA\")
///               )";
///
///     let u: User = sexpr::from_slice(s).unwrap();
///     println!("{:#?}", u);
/// }
/// ```
pub fn from_slice<'a, T>(v: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    from_trait(read::SliceRead::new(v))
}

/// Deserialize an instance of type `T` from a string of S-expressions.
///
/// # Errors
///
/// This conversion can fail if the structure of the input does not match the
/// structure expected by `T`, for example if `T` is a struct type but the input
/// contains something other than a S-expression "map". It can also fail if the
/// structure is correct but `T`'s implementation of `Deserialize` decides that
/// something is wrong with the data, for example required struct fields are
/// missing from the S-expression or some number is too big to fit in the expected
/// primitive type.
///
/// ```
/// use serde_derive::Deserialize;
///
/// #[derive(Deserialize, Debug)]
/// struct User {
///     fingerprint: String,
///     location: String,
/// }
///
/// fn main() {
///     // The type of `s` is `&str`
///     let s = "(
///                (fingerprint . \"0xF9BA143B95FF6D82\")
///                (location . \"Menlo Park, CA\")
///              )";
///
///     let u: User = sexpr::from_str(s).unwrap();
///     println!("{:#?}", u);
/// }
/// ```
pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    from_trait(read::StrRead::new(s))
}

#[cfg(test)]
mod tests {
    use serde_derive::Deserialize;

    #[derive(Eq, PartialEq, Deserialize, Debug)]
    struct User {
        fingerprint: String,
        location: String,
    }

    #[test]
    fn test_struct_symbol_keys() {
        let s = "((fingerprint . \"0xF9BA143B95FF6D82\")
                  (location . \"Menlo Park, CA\"))";
        let user: User = super::from_str(s).unwrap();
        assert_eq!(
            user,
            User {
                fingerprint: "0xF9BA143B95FF6D82".into(),
                location: "Menlo Park, CA".into(),
            }
        );
    }

    #[test]
    fn test_struct_string_keys() {
        let s = "((\"fingerprint\" . \"0xF9BA143B95FF6D82\")
                  (\"location\" . \"Menlo Park, CA\"))";
        let user: User = super::from_str(s).unwrap();
        assert_eq!(
            user,
            User {
                fingerprint: "0xF9BA143B95FF6D82".into(),
                location: "Menlo Park, CA".into(),
            }
        );
    }
}

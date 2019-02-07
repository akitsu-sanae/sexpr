use crate::ast::Ast;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

#[derive(Debug)]
struct Parser {
    tokens: Vec<TokenTree>,
    index: usize,
}

#[derive(Debug)]
pub enum ParseError {
    Int(std::num::ParseIntError),
    UnexpectedToken(TokenTree),
    UnexpectedChar(char),
    UnexpectedDelimiter(Delimiter),
    UnexpectedEnd,
}

impl Parser {
    fn new(tokens: Vec<TokenTree>) -> Self {
        Parser { tokens, index: 0 }
    }

    fn next_token(&mut self) -> Option<&TokenTree> {
        if self.index == self.tokens.len() {
            return None;
        }
        let token = &self.tokens[self.index];
        self.index += 1;
        Some(token)
    }

    fn token(&mut self) -> Result<&TokenTree, ParseError> {
        self.next_token().ok_or(ParseError::UnexpectedEnd)
    }

    fn peek(&mut self) -> Option<&TokenTree> {
        if self.index == self.tokens.len() {
            return None;
        }
        Some(&self.tokens[self.index])
    }

    fn eat_token(&mut self) {
        assert!(self.index < self.tokens.len());
        self.index += 1;
    }

    fn parse(&mut self) -> Result<Ast, ParseError> {
        match self.token()? {
            TokenTree::Punct(punct) => match punct.as_char() {
                '#' => self.parse_octothorpe(),
                c => Err(ParseError::UnexpectedChar(c)),
            },
            TokenTree::Literal(literal) => {
                let s = literal.to_string();
                let b: &[u8] = s.as_ref();
                match b[0] {
                    b'"' => Ok(Ast::String(s[1..s.len() - 1].to_string())),
                    b'0'...b'9' => Ok(Ast::Int(s.parse::<u64>().map_err(ParseError::Int)?)),
                    c => Err(ParseError::UnexpectedChar(c as char)),
                }
            }
            TokenTree::Ident(ident) => Ok(Ast::Symbol(ident.to_string())),
            TokenTree::Group(group) => match group.delimiter() {
                Delimiter::Parenthesis => Self::parse_list(group.stream()),
                delim => Err(ParseError::UnexpectedDelimiter(delim)),
            },
        }
    }

    fn parse_list(tokens: TokenStream) -> Result<Ast, ParseError> {
        let mut elements = vec![];
        let mut tail = None;
        let mut parser = Parser::new(tokens.into_iter().collect());
        loop {
            if let Some(token) = parser.peek() {
                if let TokenTree::Punct(punct) = token {
                    if punct.as_char() == '.' {
                        if tail.is_some() {
                            return Err(ParseError::UnexpectedChar('.'));
                        }
                        parser.eat_token();
                        tail = Some(parser.parse()?);
                        continue;
                    }
                }
                elements.push(parser.parse()?);
            } else {
                break;
            }
        }
        match tail {
            Some(rest) => Ok(Ast::ImproperList(elements, Box::new(rest))),
            None => Ok(Ast::List(elements)),
        }
    }

    fn parse_octothorpe(&mut self) -> Result<Ast, ParseError> {
        let token = self.token()?;
        match token {
            TokenTree::Punct(punct) => match punct.as_char() {
                ':' => Ok(Ast::Keyword(self.parse_ident()?)),
                c => Err(ParseError::UnexpectedChar(c)),
            },
            TokenTree::Ident(ident) => {
                let name = ident.to_string();
                match name.as_str() {
                    "t" => Ok(Ast::Boolean(true)),
                    "f" => Ok(Ast::Boolean(false)),
                    _ => Err(ParseError::UnexpectedToken(token.clone())),
                }
            }
            t => Err(ParseError::UnexpectedToken(t.clone())),
        }
    }

    fn parse_ident(&mut self) -> Result<String, ParseError> {
        match self.token()? {
            TokenTree::Ident(ident) => Ok(ident.to_string()),
            t => Err(ParseError::UnexpectedToken(t.clone())),
        }
    }
}

pub fn parse(tokens: TokenStream) -> Result<Ast, ParseError> {
    let mut parser = Parser::new(tokens.into_iter().collect());
    parser.parse()
}

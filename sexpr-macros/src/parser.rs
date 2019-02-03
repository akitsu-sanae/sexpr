use crate::ast::Ast;

use proc_macro2::{TokenStream, TokenTree};

#[derive(Debug)]
struct Parser {
    tokens: Vec<TokenTree>,
    index: usize,
}

#[derive(Debug)]
pub struct ParseError;

impl Parser {
    fn new(tokens: Vec<TokenTree>) -> Self {
        Parser { tokens, index: 0 }
    }

    fn token(&mut self) -> Result<&TokenTree, ParseError> {
        if self.index == self.tokens.len() {
            return Err(ParseError)
        }
        let token = &self.tokens[self.index];
        self.index += 1;
        Ok(token)
    }

    fn parse(&mut self) -> Result<Ast, ParseError> {
        match self.token()? {
            TokenTree::Punct(punct) => {
                match punct.as_char() {
                    '#' => self.parse_octothorpe(),
                    _ => Err(ParseError),
                }
            }
            _ => Err(ParseError),
        }
    }

    fn parse_octothorpe(&mut self) -> Result<Ast, ParseError> {
        match self.token()? {
            TokenTree::Punct(punct) => {
                match punct.as_char() {
                    ':' => Ok(Ast::Keyword(self.parse_ident()?)),
                    _ => Err(ParseError),
                }
            }
            TokenTree::Ident(ident) => {
                let name = ident.to_string();
                match name.as_str() {
                    "t" => Ok(Ast::Boolean(true)),
                    "f" => Ok(Ast::Boolean(false)),
                    _ => Err(ParseError),
                }
            }
            _ => Err(ParseError),
        }
    }

    fn parse_ident(&mut self)  -> Result<String, ParseError> {
        match self.token()? {
            TokenTree::Ident(ident) => Ok(ident.to_string()),
            _ => Err(ParseError),
        }
    }
}

pub fn parse(tokens: TokenStream) -> Result<Ast, ParseError> {
    let mut parser = Parser::new(tokens.into_iter().collect());
    parser.parse()
}

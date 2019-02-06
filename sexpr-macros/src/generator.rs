use crate::ast::Ast;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

impl ToTokens for Ast {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use Ast::*;

        let expanded = match self {
            Boolean(value) => quote! { ::sexpr::Sexp::from(#value) },
            Int(value) => quote! { ::sexpr::Sexp::from(#value) },
            Keyword(name) => quote! { ::sexpr::Sexp::new_keyword(#name) },
            String(s) => quote! { ::sexpr::Sexp::from(#s) },
            List(elements) => quote! { ::sexpr::Sexp::List(vec![#(#elements),*]) },
            ImproperList(elements, rest) => quote! { ::sexpr::Sexp::ImproperList(vec![#(#elements),*], #rest) },
        };
        tokens.extend(expanded);
    }
}

pub fn generate(ast: Ast) -> TokenStream {
    ast.into_token_stream()
}

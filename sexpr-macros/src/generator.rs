use crate::ast::Ast;

use proc_macro2::TokenStream;
use quote::quote;

pub fn generate(ast: Ast) -> TokenStream {
    use Ast::*;

    match ast {
        Keyword(name) => quote! { ::sexpr::Sexp::new_keyword(#name) },
        Boolean(value) => quote! { ::sexpr::Sexp::from(#value) },
    }
}

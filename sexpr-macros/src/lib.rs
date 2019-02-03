extern crate proc_macro;

mod ast;
mod generator;
mod parser;

use proc_macro2::TokenStream;
use proc_macro_hack::proc_macro_hack;
use quote::quote;

#[proc_macro_hack]
pub fn sexp(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let output = match expand(TokenStream::from(input)) {
        Err(e) => {
            let msg = format!("could not parse s-expression: {:?}", e);
            quote! { compile_error!(#msg) }
        }
        Ok(output) => output,
    };

    proc_macro::TokenStream::from(output)
}

fn expand(input: TokenStream) -> Result<TokenStream, parser::ParseError> {
    let ast = parser::parse(input)?;
    Ok(generator::generate(ast))
}

use syn::{DeriveInput, parse_macro_input};

mod document;
mod key_group;
mod util;

#[proc_macro_derive(Document, attributes(document))]
pub fn derive_document(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match document::derive_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

#[proc_macro_derive(KeyGroup)]
pub fn derive_group(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match key_group::derive_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

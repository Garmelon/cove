use case::CaseExt;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput};

use crate::util;

fn decapitalize(s: &str) -> String {
    let mut chars = s.chars();
    if let Some(char) = chars.next() {
        char.to_lowercase().chain(chars).collect()
    } else {
        String::new()
    }
}

pub fn derive_impl(input: DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(data) = input.data else {
        return util::bail(input.span(), "Must be a struct");
    };

    let struct_ident = input.ident;
    let enum_ident = format_ident!("{}Event", struct_ident);

    let mut enum_variants = vec![];
    let mut match_cases = vec![];
    for field in &data.fields {
        if let Some(field_ident) = &field.ident {
            let docstring = util::docstring(field)?;
            let variant_ident = format_ident!("{}", field_ident.to_string().to_camel());

            enum_variants.push(quote! {
                #[doc = #docstring]
                #variant_ident,
            });

            let description = decapitalize(&docstring);
            let description = description.strip_suffix('.').unwrap_or(&description);
            match_cases.push(quote!{
                () if input.matches(&self.#field_ident, #description) => Some(Self::Event::#variant_ident),
            });
        }
    }

    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #enum_ident {
            #( #enum_variants )*
        }

        impl ::cove_input::KeyGroup for #struct_ident {
            type Event = #enum_ident;

            fn event(&self, input: &mut ::cove_input::Input) -> Option<Self::Event> {
                match () {
                    #( #match_cases )*
                    () => None,
                }
            }
        }
    })
}

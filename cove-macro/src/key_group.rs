use proc_macro2::TokenStream;
use quote::quote;
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
        return util::bail(input.span(), "must be a struct");
    };

    let docstring = util::docstring(&input.attrs)?;
    let description = docstring.strip_suffix('.').unwrap_or(&docstring);

    let mut bindings = vec![];
    let mut defaults = vec![];
    for field in &data.fields {
        if let Some(field_ident) = &field.ident {
            let field_name = field_ident.to_string();

            let docstring = util::docstring(&field.attrs)?;
            let description = decapitalize(&docstring);
            let description = description.strip_suffix('.').unwrap_or(&description);

            let default = util::serde_default(field)?;
            let Some(default) = default else {
                return util::bail(field_ident.span(), "must have serde default");
            };
            let default_value = default.value();

            bindings.push(quote! {
                ::cove_input::KeyBindingInfo {
                    name: #field_name,
                    binding: &self.#field_ident,
                    description: #description
                }
            });

            defaults.push(quote! {
                #field_ident: #default_value,
            });
        }
    }

    let ident = input.ident;
    Ok(quote! {
        impl ::cove_input::KeyGroup for #ident {
            const DESCRIPTION: &'static str = #description;

            fn bindings(&self) -> Vec<::cove_input::KeyBindingInfo<'_>> {
                vec![
                    #( #bindings, )*
                ]
            }
        }

        impl Default for #ident {
            fn default() -> Self {
                Self {
                    #( #defaults )*
                }
            }
        }
    })
}

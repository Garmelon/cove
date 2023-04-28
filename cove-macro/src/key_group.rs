use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput};

use crate::util::{self, bail};

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

    let mut bindings = vec![];
    let mut defaults = vec![];
    for field in &data.fields {
        if let Some(field_ident) = &field.ident {
            let docstring = util::docstring(field)?;
            let description = decapitalize(&docstring);
            let description = description.strip_suffix('.').unwrap_or(&description);

            let default = util::serde_default(field)?;
            let Some(default) = default else {
                return bail(field_ident.span(), "must have serde default");
            };
            let default_value = default.value();

            bindings.push(quote! {
                (&self.#field_ident, #description)
            });

            defaults.push(quote! {
                #field_ident: #default_value,
            });
        }
    }

    let ident = input.ident;
    Ok(quote! {
        impl ::cove_input::KeyGroup for #ident {
            fn bindings(&self) -> Vec<(&::cove_input::KeyBinding, &'static str)> {
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

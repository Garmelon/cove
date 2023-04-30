use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Field, Ident, LitStr};

use crate::util::{self, SerdeDefault};

#[derive(Default)]
struct FieldInfo {
    description: Option<String>,
    metavar: Option<LitStr>,
    default: Option<LitStr>,
    serde_default: Option<SerdeDefault>,
    no_default: bool,
}

impl FieldInfo {
    fn initialize_from_field(&mut self, field: &Field) -> syn::Result<()> {
        let docstring = util::docstring(&field.attrs)?;
        if !docstring.is_empty() {
            self.description = Some(docstring);
        }

        for arg in util::attribute_arguments(&field.attrs, "document")? {
            if arg.path.is_ident("metavar") {
                // Parse `#[document(metavar = "bla")]`
                if let Some(metavar) = arg.value.and_then(util::into_litstr) {
                    self.metavar = Some(metavar);
                } else {
                    util::bail(arg.path.span(), "must be of the form `key = \"value\"`")?;
                }
            } else if arg.path.is_ident("default") {
                // Parse `#[document(default = "bla")]`
                if let Some(value) = arg.value.and_then(util::into_litstr) {
                    self.default = Some(value);
                } else {
                    util::bail(arg.path.span(), "must be of the form `key = \"value\"`")?;
                }
            } else if arg.path.is_ident("no_default") {
                // Parse #[document(no_default)]
                if arg.value.is_some() {
                    util::bail(arg.path.span(), "must not have a value")?;
                }
                self.no_default = true;
            } else {
                util::bail(arg.path.span(), "unknown argument name")?;
            }
        }

        // Find `#[serde(default)]` or `#[serde(default = "bla")]`.
        self.serde_default = util::serde_default(field)?;

        Ok(())
    }

    fn from_field(field: &Field) -> syn::Result<Self> {
        let mut result = Self::default();
        result.initialize_from_field(field)?;
        Ok(result)
    }
}

fn from_struct(ident: Ident, data: DataStruct) -> syn::Result<TokenStream> {
    let mut fields = vec![];
    for field in data.fields {
        let Some(ident) = field.ident.as_ref() else {
            return util::bail(field.span(), "must not be a tuple struct");
        };
        let ident = ident.to_string();

        let info = FieldInfo::from_field(&field)?;

        let mut setters = vec![];
        if let Some(description) = info.description {
            setters.push(quote! {
                doc.description = Some(#description.to_string());
            });
        }
        if let Some(metavar) = info.metavar {
            setters.push(quote! {
                doc.wrap_info.metavar = Some(#metavar.to_string());
            });
        }
        if info.no_default {
        } else if let Some(default) = info.default {
            setters.push(quote! {
                doc.value_info.default = Some(#default.to_string());
            });
        } else if let Some(serde_default) = info.serde_default {
            let value = serde_default.value();
            setters.push(quote! {
                doc.value_info.default = Some(crate::doc::toml_value_as_markdown(&#value));
            });
        }

        let ty = field.ty;
        fields.push(quote! {
            fields.insert(
                #ident.to_string(),
                {
                    let mut doc = <#ty as crate::doc::Document>::doc();
                    #( #setters )*
                    ::std::boxed::Box::new(doc)
                }
            );
        });
    }

    let tokens = quote!(
        impl crate::doc::Document for #ident {
            fn doc() -> crate::doc::Doc {
                let mut fields = ::std::collections::HashMap::new();
                #( #fields )*

                let mut doc = crate::doc::Doc::default();
                doc.struct_info.fields = fields;
                doc
            }
        }
    );

    Ok(tokens)
}

fn from_enum(ident: Ident, data: DataEnum) -> syn::Result<TokenStream> {
    let mut values = vec![];
    for variant in data.variants {
        let ident = variant.ident;
        values.push(quote! {
            crate::doc::toml_value_as_markdown(&Self::#ident)
        });
    }

    let tokens = quote!(
        impl crate::doc::Document for #ident {
            fn doc() -> crate::doc::Doc {
                let mut doc = <String as crate::doc::Document>::doc();
                doc.value_info.values = Some(vec![ #( #values ),* ]);
                doc
            }
        }
    );

    Ok(tokens)
}

pub fn derive_impl(input: DeriveInput) -> syn::Result<TokenStream> {
    match input.data {
        Data::Struct(data) => from_struct(input.ident, data),
        Data::Enum(data) => from_enum(input.ident, data),
        Data::Union(_) => util::bail(input.span(), "must be an enum or a struct"),
    }
}

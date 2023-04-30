use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprLit, ExprPath, Field, Lit, LitStr, Path, Token, Type};

pub fn bail<T>(span: Span, message: &str) -> syn::Result<T> {
    Err(syn::Error::new(span, message))
}

pub fn litstr(expr: &Expr) -> Option<&LitStr> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) => Some(lit),
        _ => None,
    }
}

pub fn into_litstr(expr: Expr) -> Option<LitStr> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) => Some(lit),
        _ => None,
    }
}

/// Given a struct field, this finds all attributes like `#[doc = "bla"]`,
/// unindents, concatenates and returns them.
pub fn docstring(attributes: &[Attribute]) -> syn::Result<String> {
    let mut lines = vec![];

    for attr in attributes.iter().filter(|attr| attr.path().is_ident("doc")) {
        if let Some(lit) = litstr(&attr.meta.require_name_value()?.value) {
            let value = lit.value();
            let value = value
                .strip_prefix(' ')
                .map(|value| value.to_string())
                .unwrap_or(value);
            lines.push(value);
        }
    }

    Ok(lines.join("\n"))
}

pub struct AttributeArgument {
    pub path: Path,
    pub value: Option<Expr>,
}

impl Parse for AttributeArgument {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let path = Path::parse(input)?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(Expr::parse(input)?)
        } else {
            None
        };
        Ok(Self { path, value })
    }
}

/// Given a struct field, this finds all arguments of the form `#[path(key)]`
/// and `#[path(key = value)]`. Multiple arguments may be specified in a single
/// annotation, e.g. `#[foo(bar, baz = true)]`.
pub fn attribute_arguments(
    attributes: &[Attribute],
    path: &str,
) -> syn::Result<Vec<AttributeArgument>> {
    let mut attr_args = vec![];

    for attr in attributes.iter().filter(|attr| attr.path().is_ident(path)) {
        let args =
            attr.parse_args_with(Punctuated::<AttributeArgument, Token![,]>::parse_terminated)?;
        attr_args.extend(args);
    }

    Ok(attr_args)
}

pub enum SerdeDefault {
    Default(Type),
    Path(ExprPath),
}

impl SerdeDefault {
    pub fn value(&self) -> TokenStream {
        match self {
            Self::Default(ty) => quote! {
                <#ty as Default>::default()
            },
            Self::Path(path) => quote! {
                #path()
            },
        }
    }
}

/// Find `#[serde(default)]` or `#[serde(default = "bla")]`.
pub fn serde_default(field: &Field) -> syn::Result<Option<SerdeDefault>> {
    for arg in attribute_arguments(&field.attrs, "serde")? {
        if arg.path.is_ident("default") {
            if let Some(value) = arg.value {
                if let Some(path) = into_litstr(value) {
                    return Ok(Some(SerdeDefault::Path(path.parse()?)));
                }
            } else {
                return Ok(Some(SerdeDefault::Default(field.ty.clone())));
            }
        }
    }
    Ok(None)
}

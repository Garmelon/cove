use proc_macro2::Span;
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, Field, Lit, LitStr, Path, Token};

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
pub fn docstring(field: &Field) -> syn::Result<String> {
    let mut lines = vec![];

    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
    {
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
pub fn attribute_arguments(field: &Field, path: &str) -> syn::Result<Vec<AttributeArgument>> {
    let mut attrs = vec![];

    for attr in field.attrs.iter().filter(|attr| attr.path().is_ident(path)) {
        let args =
            attr.parse_args_with(Punctuated::<AttributeArgument, Token![,]>::parse_terminated)?;
        attrs.extend(args);
    }

    Ok(attrs)
}

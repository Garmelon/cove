use syn::{Expr, ExprLit, Field, Lit, LitStr};

pub fn strlit(expr: &Expr) -> Option<&LitStr> {
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
        if let Some(lit) = strlit(&attr.meta.require_name_value()?.value) {
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

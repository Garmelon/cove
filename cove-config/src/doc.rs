//! Auto-generate markdown documentation.

use std::{collections::HashMap, path::PathBuf};

use cove_input::KeyBinding;
pub use cove_macro::Document;
use serde::Serialize;

const MARKDOWN_INTRODUCTION: &str = r#"# Config file format

Cove's config file uses the [TOML](https://toml.io/) format.

Here is an example config that changes a few different options:

```toml
measure_widths = true
rooms_sort_order = "importance"

[euph.servers."euphoria.leet.nu".rooms]
welcome.autojoin = true
test.username = "badingle"
test.force_username = true
private.password = "foobar"

[keys]
general.abort = ["esc", "ctrl+c"]
general.exit = "ctrl+q"
tree.action.fold_tree = "f"
```

## Key bindings

Key bindings are specified as strings or lists of strings. Each string specifies
a main key and zero or more modifier keys. The modifier keys (if any) are listed
first, followed by the main key. They are separated by the `+` character and
**no** whitespace.

Examples of key bindings:
- `"ctrl+c"`
- `"X"` (not `"shift+x"`)
- `"space"` or `" "` (both space bar)
- `["g", "home"]`
- `["K", "ctrl+up"]`
- `["f1", "?"]`
- `"ctrl+alt+f3"`
- `["enter", "any+enter"]` (matches `enter` regardless of modifiers)

Available main keys:
- Any single character that can be typed
- `esc`, `enter`, `space`, `tab`, `backtab`
- `backspace`, `delete`, `insert`
- `left`, `right`, `up`, `down`
- `home`, `end`, `pageup`, `pagedown`
- `f1`, `f2`, ...

Available modifiers:
- `shift` (must not be used with single characters)
- `ctrl`
- `alt`
- `any` (matches as long as at least one modifier is pressed)

## Available options
"#;

pub fn toml_value_as_markdown<T: Serialize>(value: &T) -> String {
    let mut result = String::new();
    value
        .serialize(toml::ser::ValueSerializer::new(&mut result))
        .expect("not a valid toml value");
    format!("`{result}`")
}

#[derive(Clone, Default)]
pub struct ValueInfo {
    pub required: Option<bool>,
    pub r#type: Option<String>,
    pub values: Option<Vec<String>>,
    pub default: Option<String>,
}

impl ValueInfo {
    fn as_markdown(&self) -> String {
        let mut lines = vec![];

        if let Some(required) = self.required {
            let yesno = if required { "yes" } else { "no" };
            lines.push(format!("**Required:** {yesno}"));
        }

        if let Some(r#type) = &self.r#type {
            lines.push(format!("**Type:** {type}"));
        }

        if let Some(values) = &self.values {
            let values = values.join(", ");
            lines.push(format!("**Values:** {values}"));
        }

        if let Some(default) = &self.default {
            lines.push(format!("**Default:** {default}"));
        }

        lines.join("  \n")
    }
}

#[derive(Clone, Default)]
pub struct StructInfo {
    pub fields: HashMap<String, Box<Doc>>,
}

#[derive(Clone, Default)]
pub struct WrapInfo {
    pub inner: Option<Box<Doc>>,
    pub metavar: Option<String>,
}

#[derive(Clone, Default)]
pub struct Doc {
    pub description: Option<String>,

    pub value_info: ValueInfo,
    pub struct_info: StructInfo,
    pub wrap_info: WrapInfo,
}

struct Entry {
    path: String,
    description: String,
    value_info: ValueInfo,
}

impl Entry {
    fn new(description: String, value_info: ValueInfo) -> Self {
        Self {
            path: String::new(),
            description,
            value_info,
        }
    }

    fn with_parent(mut self, segment: String) -> Self {
        if self.path.is_empty() {
            self.path = segment;
        } else {
            self.path = format!("{segment}.{}", self.path);
        }
        self
    }
}

impl Doc {
    fn entries(&self) -> Vec<Entry> {
        let mut entries = vec![];

        if let Some(description) = &self.description {
            entries.push(Entry::new(description.clone(), self.value_info.clone()));
        }

        for (segment, field) in &self.struct_info.fields {
            entries.extend(
                field
                    .entries()
                    .into_iter()
                    .map(|entry| entry.with_parent(segment.clone())),
            );
        }

        if let Some(inner) = &self.wrap_info.inner {
            let segment = match &self.wrap_info.metavar {
                Some(metavar) => format!("<{metavar}>"),
                None => "<...>".to_string(),
            };
            entries.extend(
                inner
                    .entries()
                    .into_iter()
                    .map(|entry| entry.with_parent(segment.clone())),
            );
        }

        entries
    }

    pub fn as_markdown(&self) -> String {
        // Print entries in alphabetical order to make generated documentation
        // format more stable.
        let mut entries = self.entries();
        entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));

        let mut result = String::new();

        result.push_str(MARKDOWN_INTRODUCTION);

        for entry in entries {
            result.push_str(&format!("\n### `{}`\n", entry.path));

            let value_info = entry.value_info.as_markdown();
            if !value_info.is_empty() {
                result.push_str(&format!("\n{value_info}\n"));
            }

            if !entry.description.is_empty() {
                result.push_str(&format!("\n{}\n", entry.description));
            }
        }

        result
    }
}

pub trait Document {
    fn doc() -> Doc;
}

impl Document for String {
    fn doc() -> Doc {
        let mut doc = Doc::default();
        doc.value_info.required = Some(true);
        doc.value_info.r#type = Some("string".to_string());
        doc
    }
}

impl Document for bool {
    fn doc() -> Doc {
        let mut doc = Doc::default();
        doc.value_info.required = Some(true);
        doc.value_info.r#type = Some("boolean".to_string());
        doc
    }
}

impl Document for PathBuf {
    fn doc() -> Doc {
        let mut doc = Doc::default();
        doc.value_info.required = Some(true);
        doc.value_info.r#type = Some("path".to_string());
        doc
    }
}

impl<I: Document> Document for Option<I> {
    fn doc() -> Doc {
        let mut doc = I::doc();
        assert_eq!(doc.value_info.required, Some(true));
        doc.value_info.required = Some(false);
        doc
    }
}

impl<I: Document> Document for HashMap<String, I> {
    fn doc() -> Doc {
        let mut doc = Doc::default();
        doc.wrap_info.inner = Some(Box::new(I::doc()));
        doc
    }
}

impl Document for KeyBinding {
    fn doc() -> Doc {
        let mut doc = Doc::default();
        doc.value_info.required = Some(true);
        doc.value_info.r#type = Some("key binding".to_string());
        doc
    }
}

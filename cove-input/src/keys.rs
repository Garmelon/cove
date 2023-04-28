use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{de::Error, Deserialize, Deserializer};
use serde::{Serialize, Serializer};
use serde_either::SingleOrVec;

#[derive(Debug, thiserror::Error)]
pub enum ParseKeysError {
    #[error("no key code specified")]
    NoKeyCode,
    #[error("unknown key code: {0:?}")]
    UnknownKeyCode(String),
    #[error("invalid function key number: {0}")]
    InvalidFNumber(#[from] ParseIntError),
    #[error("unknown modifier: {0:?}")]
    UnknownModifier(String),
    #[error("modifier {0} conflicts with previous modifier")]
    ConflictingModifier(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub code: KeyCode,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub any: bool,
}

impl KeyPress {
    fn parse_key_code(code: &str) -> Result<Self, ParseKeysError> {
        let code = match code {
            "backspace" => KeyCode::Backspace,
            "enter" => KeyCode::Enter,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "delete" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            "esc" => KeyCode::Esc,
            c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().unwrap()),
            c if c.starts_with('f') => KeyCode::F(c.strip_prefix('f').unwrap().parse()?),
            "" => return Err(ParseKeysError::NoKeyCode),
            c => return Err(ParseKeysError::UnknownKeyCode(c.to_string())),
        };
        Ok(Self {
            code,
            shift: false,
            ctrl: false,
            alt: false,
            any: false,
        })
    }

    fn display_key_code(code: KeyCode) -> String {
        match code {
            KeyCode::Backspace => "backspace".to_string(),
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Left => "left".to_string(),
            KeyCode::Right => "right".to_string(),
            KeyCode::Up => "up".to_string(),
            KeyCode::Down => "down".to_string(),
            KeyCode::Home => "home".to_string(),
            KeyCode::End => "end".to_string(),
            KeyCode::PageUp => "pageup".to_string(),
            KeyCode::PageDown => "pagedown".to_string(),
            KeyCode::Tab => "tab".to_string(),
            KeyCode::BackTab => "backtab".to_string(),
            KeyCode::Delete => "delete".to_string(),
            KeyCode::Insert => "insert".to_string(),
            KeyCode::Esc => "esc".to_string(),
            KeyCode::F(n) => format!("f{n}"),
            KeyCode::Char(c) => c.to_string(),
            _ => "unknown".to_string(),
        }
    }

    fn parse_modifier(
        &mut self,
        modifier: &str,
        shift_allowed: bool,
    ) -> Result<(), ParseKeysError> {
        match modifier {
            m if self.any => return Err(ParseKeysError::ConflictingModifier(m.to_string())),
            "shift" if shift_allowed && !self.shift => self.shift = true,
            "ctrl" if !self.ctrl => self.ctrl = true,
            "alt" if !self.alt => self.alt = true,
            "any" if !self.shift && !self.ctrl && !self.alt => self.any = true,
            m @ ("shift" | "ctrl" | "alt" | "any") => {
                return Err(ParseKeysError::ConflictingModifier(m.to_string()))
            }
            m => return Err(ParseKeysError::UnknownModifier(m.to_string())),
        }
        Ok(())
    }

    pub fn matches(&self, event: KeyEvent) -> bool {
        if event.code != self.code {
            return false;
        }

        if self.any && !event.modifiers.is_empty() {
            return true;
        }

        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl;
        let alt = event.modifiers.contains(KeyModifiers::ALT) == self.alt;
        if matches!(self.code, KeyCode::Char(_)) {
            ctrl && alt
        } else {
            let shift = event.modifiers.contains(KeyModifiers::SHIFT) == self.shift;
            shift && ctrl && alt
        }
    }
}

impl FromStr for KeyPress {
    type Err = ParseKeysError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('+');
        let code = parts.next_back().ok_or(ParseKeysError::NoKeyCode)?;

        let mut keys = KeyPress::parse_key_code(code)?;
        let shift_allowed = !matches!(keys.code, KeyCode::Char(_));
        for modifier in parts {
            keys.parse_modifier(modifier, shift_allowed)?;
        }

        Ok(keys)
    }
}

impl fmt::Display for KeyPress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = Self::display_key_code(self.code);

        let mut segments = vec![];
        if self.shift {
            segments.push("shift");
        }
        if self.ctrl {
            segments.push("ctrl");
        }
        if self.alt {
            segments.push("alt");
        }
        if self.any {
            segments.push("any");
        }
        segments.push(&code);

        segments.join("+").fmt(f)
    }
}

impl Serialize for KeyPress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{self}").serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyPress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|e| D::Error::custom(format!("{e}")))
    }
}

#[derive(Debug, Clone)]
pub struct KeyBinding(Vec<KeyPress>);

impl KeyBinding {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn keys(&self) -> &[KeyPress] {
        &self.0
    }

    pub fn with_key(self, key: &str) -> Result<Self, ParseKeysError> {
        self.with_keys([key])
    }

    pub fn with_keys<'a, I>(mut self, keys: I) -> Result<Self, ParseKeysError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        for key in keys {
            self.0.push(key.parse()?);
        }
        Ok(self)
    }

    pub fn matches(&self, event: KeyEvent) -> bool {
        self.0.iter().any(|kp| kp.matches(event))
    }
}

impl Default for KeyBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for KeyBinding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.len() == 1 {
            self.0[0].serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match SingleOrVec::<KeyPress>::deserialize(deserializer)? {
            SingleOrVec::Single(key) => Self(vec![key]),
            SingleOrVec::Vec(keys) => Self(keys),
        })
    }
}

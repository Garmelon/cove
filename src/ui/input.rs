use crossterm::event::{KeyCode, KeyModifiers};

/// A key event data type that is a bit easier to pattern match on than
/// [`crossterm::event::KeyEvent`].
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: event.code,
            shift: event.modifiers.contains(KeyModifiers::SHIFT),
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
        }
    }
}

#[rustfmt::skip]
macro_rules! key {
    // key!('a')
    (        $key:literal ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: false, alt: false, } };
    ( Ctrl + $key:literal ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: true,  alt: false, } };
    (  Alt + $key:literal ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: false, alt: true,  } };

    // key!(Char(xyz))
    (        Char $key:pat ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: false, alt: false, } };
    ( Ctrl + Char $key:pat ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: true,  alt: false, } };
    (  Alt + Char $key:pat ) => { KeyEvent { code: KeyCode::Char($key), shift: _, ctrl: false, alt: true,  } };

    // key!(F(n))
    (         F $key:pat ) => { KeyEvent { code: KeyCode::F($key), shift: false, ctrl: false, alt: false, } };
    ( Shift + F $key:pat ) => { KeyEvent { code: KeyCode::F($key), shift: true,  ctrl: false, alt: false, } };
    (  Ctrl + F $key:pat ) => { KeyEvent { code: KeyCode::F($key), shift: false, ctrl: true,  alt: false, } };
    (   Alt + F $key:pat ) => { KeyEvent { code: KeyCode::F($key), shift: false, ctrl: false, alt: true,  } };

    // key!(other)
    (         $key:ident ) => { KeyEvent { code: KeyCode::$key, shift: false, ctrl: false, alt: false, } };
    ( Shift + $key:ident ) => { KeyEvent { code: KeyCode::$key, shift: true,  ctrl: false, alt: false, } };
    (  Ctrl + $key:ident ) => { KeyEvent { code: KeyCode::$key, shift: false, ctrl: true,  alt: false, } };
    (   Alt + $key:ident ) => { KeyEvent { code: KeyCode::$key, shift: false, ctrl: false, alt: true,  } };
}
pub(crate) use key;

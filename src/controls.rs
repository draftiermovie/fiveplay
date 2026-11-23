//! Rebindable keyboard controls. Mouse fire/aim, weapon keys 1-5 and Escape
//! stay fixed; everything else can be assigned any key in [`KEYS`].
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bind {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Crouch,
    Jump,
    Reload,
    Scoreboard,
}
impl Bind {
    pub const ALL: [Bind; 9] = [
        Bind::Forward,
        Bind::Back,
        Bind::Left,
        Bind::Right,
        Bind::Sprint,
        Bind::Crouch,
        Bind::Jump,
        Bind::Reload,
        Bind::Scoreboard,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Bind::Forward => "MOVE FORWARD",
            Bind::Back => "MOVE BACK",
            Bind::Left => "MOVE LEFT",
            Bind::Right => "MOVE RIGHT",
            Bind::Sprint => "SPRINT",
            Bind::Crouch => "CROUCH / SLIDE",
            Bind::Jump => "JUMP",
            Bind::Reload => "RELOAD",
            Bind::Scoreboard => "SCOREBOARD",
        }
    }
    /// Key used in the save file.
    pub fn save_key(self) -> &'static str {
        match self {
            Bind::Forward => "key_forward",
            Bind::Back => "key_back",
            Bind::Left => "key_left",
            Bind::Right => "key_right",
            Bind::Sprint => "key_sprint",
            Bind::Crouch => "key_crouch",
            Bind::Jump => "key_jump",
            Bind::Reload => "key_reload",
            Bind::Scoreboard => "key_scoreboard",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bindings([KeyCode; 9]);
impl Default for Bindings {
    fn default() -> Self {
        Self([
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::ShiftLeft,
            KeyCode::ControlLeft,
            KeyCode::Space,
            KeyCode::KeyR,
            KeyCode::Tab,
        ])
    }
}
impl Bindings {
    pub fn get(&self, bind: Bind) -> KeyCode {
        self.0[bind as usize]
    }
    /// Assigns `key`; if another action already uses it, the two swap so no
    /// action is ever left without a key.
    pub fn set(&mut self, bind: Bind, key: KeyCode) {
        let old = self.0[bind as usize];
        if let Some(other) = self.0.iter().position(|&k| k == key) {
            self.0[other] = old;
        }
        self.0[bind as usize] = key;
    }
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, bind: Bind) -> bool {
        keys.pressed(self.get(bind))
    }
    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, bind: Bind) -> bool {
        keys.just_pressed(self.get(bind))
    }
    pub fn name(&self, bind: Bind) -> &'static str {
        key_label(self.get(bind)).unwrap_or("?")
    }
}

/// Assignable keys: (key, save-file id, on-screen label).
pub const KEYS: &[(KeyCode, &str, &str)] = &[
    (KeyCode::KeyA, "KeyA", "A"),
    (KeyCode::KeyB, "KeyB", "B"),
    (KeyCode::KeyC, "KeyC", "C"),
    (KeyCode::KeyD, "KeyD", "D"),
    (KeyCode::KeyE, "KeyE", "E"),
    (KeyCode::KeyF, "KeyF", "F"),
    (KeyCode::KeyG, "KeyG", "G"),
    (KeyCode::KeyH, "KeyH", "H"),
    (KeyCode::KeyI, "KeyI", "I"),
    (KeyCode::KeyJ, "KeyJ", "J"),
    (KeyCode::KeyK, "KeyK", "K"),
    (KeyCode::KeyL, "KeyL", "L"),
    (KeyCode::KeyM, "KeyM", "M"),
    (KeyCode::KeyN, "KeyN", "N"),
    (KeyCode::KeyO, "KeyO", "O"),
    (KeyCode::KeyP, "KeyP", "P"),
    (KeyCode::KeyQ, "KeyQ", "Q"),
    (KeyCode::KeyR, "KeyR", "R"),
    (KeyCode::KeyS, "KeyS", "S"),
    (KeyCode::KeyT, "KeyT", "T"),
    (KeyCode::KeyU, "KeyU", "U"),
    (KeyCode::KeyV, "KeyV", "V"),
    (KeyCode::KeyW, "KeyW", "W"),
    (KeyCode::KeyX, "KeyX", "X"),
    (KeyCode::KeyY, "KeyY", "Y"),
    (KeyCode::KeyZ, "KeyZ", "Z"),
    (KeyCode::Digit6, "Digit6", "6"),
    (KeyCode::Digit7, "Digit7", "7"),
    (KeyCode::Digit8, "Digit8", "8"),
    (KeyCode::Digit9, "Digit9", "9"),
    (KeyCode::Digit0, "Digit0", "0"),
    (KeyCode::Space, "Space", "SPACE"),
    (KeyCode::Tab, "Tab", "TAB"),
    (KeyCode::CapsLock, "CapsLock", "CAPS LOCK"),
    (KeyCode::ShiftLeft, "ShiftLeft", "LEFT SHIFT"),
    (KeyCode::ShiftRight, "ShiftRight", "RIGHT SHIFT"),
    (KeyCode::ControlLeft, "ControlLeft", "LEFT CTRL"),
    (KeyCode::ControlRight, "ControlRight", "RIGHT CTRL"),
    (KeyCode::AltLeft, "AltLeft", "LEFT ALT"),
    (KeyCode::AltRight, "AltRight", "RIGHT ALT"),
    (KeyCode::Enter, "Enter", "ENTER"),
    (KeyCode::Backspace, "Backspace", "BACKSPACE"),
    (KeyCode::ArrowUp, "ArrowUp", "UP"),
    (KeyCode::ArrowDown, "ArrowDown", "DOWN"),
    (KeyCode::ArrowLeft, "ArrowLeft", "LEFT"),
    (KeyCode::ArrowRight, "ArrowRight", "RIGHT"),
    (KeyCode::Backquote, "Backquote", "`"),
    (KeyCode::Minus, "Minus", "-"),
    (KeyCode::Equal, "Equal", "="),
    (KeyCode::BracketLeft, "BracketLeft", "["),
    (KeyCode::BracketRight, "BracketRight", "]"),
    (KeyCode::Semicolon, "Semicolon", ";"),
    (KeyCode::Quote, "Quote", "'"),
    (KeyCode::Comma, "Comma", ","),
    (KeyCode::Period, "Period", "."),
    (KeyCode::Slash, "Slash", "/"),
    (KeyCode::Backslash, "Backslash", "\\"),
    (KeyCode::Insert, "Insert", "INSERT"),
    (KeyCode::Delete, "Delete", "DELETE"),
    (KeyCode::Home, "Home", "HOME"),
    (KeyCode::End, "End", "END"),
    (KeyCode::PageUp, "PageUp", "PAGE UP"),
    (KeyCode::PageDown, "PageDown", "PAGE DOWN"),
    (KeyCode::F1, "F1", "F1"),
    (KeyCode::F2, "F2", "F2"),
    (KeyCode::F3, "F3", "F3"),
    (KeyCode::F4, "F4", "F4"),
    (KeyCode::F5, "F5", "F5"),
    (KeyCode::F6, "F6", "F6"),
    (KeyCode::F7, "F7", "F7"),
    (KeyCode::F8, "F8", "F8"),
    (KeyCode::F9, "F9", "F9"),
    (KeyCode::F10, "F10", "F10"),
    (KeyCode::F11, "F11", "F11"),
    (KeyCode::F12, "F12", "F12"),
    (KeyCode::Numpad0, "Numpad0", "NUM 0"),
    (KeyCode::Numpad1, "Numpad1", "NUM 1"),
    (KeyCode::Numpad2, "Numpad2", "NUM 2"),
    (KeyCode::Numpad3, "Numpad3", "NUM 3"),
    (KeyCode::Numpad4, "Numpad4", "NUM 4"),
    (KeyCode::Numpad5, "Numpad5", "NUM 5"),
    (KeyCode::Numpad6, "Numpad6", "NUM 6"),
    (KeyCode::Numpad7, "Numpad7", "NUM 7"),
    (KeyCode::Numpad8, "Numpad8", "NUM 8"),
    (KeyCode::Numpad9, "Numpad9", "NUM 9"),
];
pub fn key_label(key: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|k| k.0 == key).map(|k| k.2)
}
pub fn key_id(key: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|k| k.0 == key).map(|k| k.1)
}
pub fn key_from_id(id: &str) -> Option<KeyCode> {
    KEYS.iter().find(|k| k.1 == id).map(|k| k.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebinding_to_a_used_key_swaps() {
        let mut b = Bindings::default();
        b.set(Bind::Jump, KeyCode::KeyW);
        assert_eq!(b.get(Bind::Jump), KeyCode::KeyW);
        assert_eq!(b.get(Bind::Forward), KeyCode::Space);
        b.set(Bind::Reload, KeyCode::KeyF);
        assert_eq!(b.get(Bind::Reload), KeyCode::KeyF);
        // Every action still has a distinct key.
        let mut keys: Vec<_> = Bind::ALL.iter().map(|&a| b.get(a)).collect();
        keys.dedup();
        keys.sort_by_key(|k| key_id(*k));
        keys.dedup();
        assert_eq!(keys.len(), Bind::ALL.len());
    }
    #[test]
    fn ids_round_trip() {
        for (key, id, _) in KEYS {
            assert_eq!(key_from_id(id), Some(*key));
        }
        assert_eq!(key_from_id("Escape"), None);
        assert!(
            key_label(KeyCode::Digit1).is_none(),
            "weapon keys stay reserved"
        );
    }
}

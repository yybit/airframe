//! Translation from egui's physical keys to the two coordinate systems the
//! protocols use.
//!
//! VNC addresses keys by X11 keysym, RDP and SPICE by PC/XT scancode set 1.
//! Both are produced here, from the *physical* key, so a Dvorak or AZERTY
//! layout on the controlling machine never leaks into the remote session.

use airframe_core::KeyInput;
use egui::{Key, Modifiers};

pub struct KeyCode {
    pub keysym: u32,
    pub scancode: u8,
    pub extended: bool,
}

pub const SHIFT: KeyCode = KeyCode { keysym: 0xFFE1, scancode: 0x2A, extended: false };
const CONTROL: KeyCode = KeyCode { keysym: 0xFFE3, scancode: 0x1D, extended: false };
const ALT: KeyCode = KeyCode { keysym: 0xFFE9, scancode: 0x38, extended: false };
const SUPER: KeyCode = KeyCode { keysym: 0xFFEB, scancode: 0x5B, extended: true };

/// Enter and Tab, needed when text is typed into a session rather than a key
/// pressed by hand.
const ENTER: KeyCode = KeyCode { keysym: 0xFF0D, scancode: 0x1C, extended: false };
const TAB: KeyCode = KeyCode { keysym: 0xFF09, scancode: 0x0F, extended: false };

/// A US layout: the printable characters a physical key carries, plain and with
/// shift. Letters are not listed; they are arithmetic in [`character`].
const PRINTABLE: [(u8, char, char); 22] = [
    (0x39, ' ', ' '),
    (0x0B, '0', ')'),
    (0x02, '1', '!'),
    (0x03, '2', '@'),
    (0x04, '3', '#'),
    (0x05, '4', '$'),
    (0x06, '5', '%'),
    (0x07, '6', '^'),
    (0x08, '7', '&'),
    (0x09, '8', '*'),
    (0x0A, '9', '('),
    (0x0C, '-', '_'),
    (0x0D, '=', '+'),
    (0x1A, '[', '{'),
    (0x1B, ']', '}'),
    (0x27, ';', ':'),
    (0x28, '\'', '"'),
    (0x29, '`', '~'),
    (0x2B, '\\', '|'),
    (0x33, ',', '<'),
    (0x34, '.', '>'),
    (0x35, '/', '?'),
];

/// Physical keys carrying the Latin letters, in alphabetical order.
const LETTERS: [u8; 26] = [
    0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19,
    0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C,
];

/// The modifier keys, in a stable order.
pub const MODIFIERS: [KeyCode; 4] = [SHIFT, CONTROL, ALT, SUPER];

/// Whether each modifier is currently held, in [`MODIFIERS`] order.
pub fn modifier_state(modifiers: &Modifiers) -> [bool; 4] {
    [modifiers.shift, modifiers.ctrl, modifiers.alt, modifiers.mac_cmd || modifiers.command]
}

/// The key code for an egui physical key, if the protocols can express it.
///
/// `shift` picks the character the key carries with shift held. A keysym names
/// a character rather than a key, so the shifted form has to be sent as its own
/// keysym, the way [`character`] sends the character itself: a server that maps
/// keysyms straight to characters would otherwise type `a` for `A`.
pub fn lookup(key: Key, shift: bool) -> Option<KeyCode> {
    let (keysym, scancode, extended) = match key {
        Key::Escape => (0xFF1B, 0x01, false),
        Key::Tab => (0xFF09, 0x0F, false),
        Key::Backspace => (0xFF08, 0x0E, false),
        Key::Enter => (0xFF0D, 0x1C, false),
        Key::Space => (0x0020, 0x39, false),

        Key::Insert => (0xFF63, 0x52, true),
        Key::Delete => (0xFFFF, 0x53, true),
        Key::Home => (0xFF50, 0x47, true),
        Key::End => (0xFF57, 0x4F, true),
        Key::PageUp => (0xFF55, 0x49, true),
        Key::PageDown => (0xFF56, 0x51, true),
        Key::ArrowUp => (0xFF52, 0x48, true),
        Key::ArrowDown => (0xFF54, 0x50, true),
        Key::ArrowLeft => (0xFF51, 0x4B, true),
        Key::ArrowRight => (0xFF53, 0x4D, true),

        Key::Num0 => (0x30, 0x0B, false),
        Key::Num1 => (0x31, 0x02, false),
        Key::Num2 => (0x32, 0x03, false),
        Key::Num3 => (0x33, 0x04, false),
        Key::Num4 => (0x34, 0x05, false),
        Key::Num5 => (0x35, 0x06, false),
        Key::Num6 => (0x36, 0x07, false),
        Key::Num7 => (0x37, 0x08, false),
        Key::Num8 => (0x38, 0x09, false),
        Key::Num9 => (0x39, 0x0A, false),

        Key::A => (0x61, 0x1E, false),
        Key::B => (0x62, 0x30, false),
        Key::C => (0x63, 0x2E, false),
        Key::D => (0x64, 0x20, false),
        Key::E => (0x65, 0x12, false),
        Key::F => (0x66, 0x21, false),
        Key::G => (0x67, 0x22, false),
        Key::H => (0x68, 0x23, false),
        Key::I => (0x69, 0x17, false),
        Key::J => (0x6A, 0x24, false),
        Key::K => (0x6B, 0x25, false),
        Key::L => (0x6C, 0x26, false),
        Key::M => (0x6D, 0x32, false),
        Key::N => (0x6E, 0x31, false),
        Key::O => (0x6F, 0x18, false),
        Key::P => (0x70, 0x19, false),
        Key::Q => (0x71, 0x10, false),
        Key::R => (0x72, 0x13, false),
        Key::S => (0x73, 0x1F, false),
        Key::T => (0x74, 0x14, false),
        Key::U => (0x75, 0x16, false),
        Key::V => (0x76, 0x2F, false),
        Key::W => (0x77, 0x11, false),
        Key::X => (0x78, 0x2D, false),
        Key::Y => (0x79, 0x15, false),
        Key::Z => (0x7A, 0x2C, false),

        Key::Minus => (0x2D, 0x0C, false),
        Key::Equals => (0x3D, 0x0D, false),
        Key::Plus => (0x2B, 0x0D, false),
        Key::OpenBracket => (0x5B, 0x1A, false),
        Key::CloseBracket => (0x5D, 0x1B, false),
        Key::OpenCurlyBracket => (0x7B, 0x1A, false),
        Key::CloseCurlyBracket => (0x7D, 0x1B, false),
        Key::Backslash => (0x5C, 0x2B, false),
        Key::Pipe => (0x7C, 0x2B, false),
        Key::Semicolon => (0x3B, 0x27, false),
        Key::Colon => (0x3A, 0x27, false),
        Key::Quote => (0x27, 0x28, false),
        Key::Backtick => (0x60, 0x29, false),
        Key::Comma => (0x2C, 0x33, false),
        Key::Period => (0x2E, 0x34, false),
        Key::Slash => (0x2F, 0x35, false),
        Key::Questionmark => (0x3F, 0x35, false),
        Key::Exclamationmark => (0x21, 0x02, false),

        // egui names these after the clipboard action; the remote side wants
        // the letter.
        Key::Copy => (0x63, 0x2E, false),
        Key::Cut => (0x78, 0x2D, false),
        Key::Paste => (0x76, 0x2F, false),

        Key::F1 => (0xFFBE, 0x3B, false),
        Key::F2 => (0xFFBF, 0x3C, false),
        Key::F3 => (0xFFC0, 0x3D, false),
        Key::F4 => (0xFFC1, 0x3E, false),
        Key::F5 => (0xFFC2, 0x3F, false),
        Key::F6 => (0xFFC3, 0x40, false),
        Key::F7 => (0xFFC4, 0x41, false),
        Key::F8 => (0xFFC5, 0x42, false),
        Key::F9 => (0xFFC6, 0x43, false),
        Key::F10 => (0xFFC7, 0x44, false),
        Key::F11 => (0xFFC8, 0x57, false),
        Key::F12 => (0xFFC9, 0x58, false),
        _ => return None,
    };
    let keysym = if shift { shifted(keysym) } else { keysym };
    Some(KeyCode { keysym, scancode, extended })
}

/// The keysym the same physical key produces with shift held: the uppercase of
/// a letter, or the second character of a printed pair. A key with no shifted
/// form keeps the keysym it already had.
fn shifted(keysym: u32) -> u32 {
    if (0x61..=0x7A).contains(&keysym) {
        return keysym - 0x20; // 'a'..'z' -> 'A'..'Z'
    }
    PRINTABLE
        .iter()
        .find_map(|&(_, plain, shifted)| (plain as u32 == keysym).then_some(shifted as u32))
        .unwrap_or(keysym)
}

impl KeyCode {
    pub fn input(&self, down: bool) -> KeyInput {
        KeyInput { down, keysym: self.keysym, scancode: u32::from(self.scancode), extended: self.extended }
    }
}

/// The key that types `c` on a US layout, and whether shift has to be held.
///
/// Only what a US keyboard can produce can be typed into a remote session: the
/// protocols carry key codes, not text. `None` means the character cannot be
/// sent at all — the caller decides what to do about it. Newlines and tabs are
/// keys rather than text, and map to Enter and Tab.
pub fn character(c: char) -> Option<(KeyCode, bool)> {
    let key = |scancode: u8, shifted: bool| {
        (KeyCode { keysym: c as u32, scancode, extended: false }, shifted)
    };
    match c {
        '\n' | '\r' => return Some((ENTER, false)),
        '\t' => return Some((TAB, false)),
        'a'..='z' => return Some(key(LETTERS[c as usize - 'a' as usize], false)),
        'A'..='Z' => return Some(key(LETTERS[c as usize - 'A' as usize], true)),
        _ => {}
    }
    // The keysym of a printable Latin-1 character is the character itself.
    PRINTABLE.iter().find_map(|&(scancode, plain, shifted)| {
        if plain == c {
            Some(key(scancode, false))
        } else if shifted == c {
            Some(key(scancode, true))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character a US keyboard can produce has to map, or typing text
    /// into a session would silently drop it — the space bar included.
    #[test]
    fn every_printable_ascii_character_can_be_typed() {
        for code in 0x20u8..=0x7E {
            let c = code as char;
            assert!(character(c).is_some(), "{c:?} has no key on a US layout");
        }
    }

    #[test]
    fn characters_map_to_the_keys_a_us_layout_puts_them_on() {
        let (key, shifted) = character('a').expect("a letter");
        assert_eq!((key.scancode, shifted), (0x1E, false));

        let (key, shifted) = character('A').expect("a shifted letter");
        assert_eq!((key.scancode, shifted, key.keysym), (0x1E, true, 0x41));

        let (key, shifted) = character('7').expect("a digit");
        assert_eq!((key.scancode, shifted), (0x08, false));

        // The shifted pairs sit on the same physical key as their plain form.
        for (plain, shifted) in [('7', '&'), ('=', '+'), ('/', '?'), (';', ':'), ('0', ')')] {
            let (plain_key, plain_shift) = character(plain).expect("the plain form");
            let (shifted_key, shifted_shift) = character(shifted).expect("the shifted form");
            assert_eq!(plain_key.scancode, shifted_key.scancode, "{plain} and {shifted}");
            assert!(!plain_shift && shifted_shift, "{plain} and {shifted}");
            assert_eq!(shifted_key.keysym, shifted as u32, "the keysym is the character");
        }

        assert_eq!(character('\n').expect("newline").0.scancode, 0x1C);
        assert_eq!(character('\t').expect("tab").0.scancode, 0x0F);
        assert!(character('中').is_none(), "a remote session cannot be sent a CJK character");
    }

    /// A typed key has to name the character shift makes of it, the way
    /// [`character`] does for pasted text.
    #[test]
    fn shifted_keys_send_the_character_they_type() {
        assert_eq!(lookup(Key::A, false).expect("a letter").keysym, 'a' as u32);
        assert_eq!(lookup(Key::A, true).expect("a letter").keysym, 'A' as u32);

        let bang = lookup(Key::Num1, true).expect("a digit");
        assert_eq!((bang.scancode, bang.keysym), (0x02, '!' as u32));

        // Keys with no shifted form keep the keysym they already had.
        assert_eq!(lookup(Key::Tab, true).expect("tab").keysym, 0xFF09);
        assert_eq!(lookup(Key::ArrowUp, true).expect("an arrow").keysym, 0xFF52);
    }
}

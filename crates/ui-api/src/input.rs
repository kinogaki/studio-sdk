//! [`Input`]: this frame's pointer, keys and typing, as plain data the
//! host fills in before it calls a plugin.
use crate::geom::{Pos, Vec2};

/// What the user did since the last frame, and the state of the pointer
/// and keys now. Read it through [`Ui::input`](crate::Ui::input).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Input {
    /// The pointer.
    pub pointer: Pointer,
    /// The modifier keys held now.
    pub modifiers: Modifiers,
    /// Everything that happened this frame, in order.
    pub events: Vec<Event>,
    /// How far the wheel or trackpad scrolled this frame, in points,
    /// smoothed by the host.
    pub scroll: Vec2,
    /// Seconds since the host started.
    pub time: f64,
    /// A text field or another widget has the keyboard: shortcuts should
    /// leave the keys alone.
    pub keyboard_taken: bool,
    /// A popup or menu is open: viewport clicks should not act.
    pub popup_open: bool,
}

impl Input {
    /// Whether `key` went down this frame (repeats included), with any
    /// modifiers.
    pub fn key_pressed(&self, key: Key) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, Event::Key { key: k, pressed: true, .. } if *k == key))
    }

    /// Whether `key` went down this frame with exactly `modifiers` held
    /// ([`Modifiers::matches`]).
    pub fn key_pressed_with(&self, key: Key, modifiers: Modifiers) -> bool {
        self.events.iter().any(|e| {
            matches!(e, Event::Key { key: k, pressed: true, modifiers: m, .. }
                if *k == key && m.matches(modifiers))
        })
    }

    /// Whether `key` came up this frame.
    pub fn key_released(&self, key: Key) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, Event::Key { key: k, pressed: false, .. } if *k == key))
    }

    /// The text typed this frame, in order.
    pub fn typed(&self) -> impl Iterator<Item = &str> {
        self.events.iter().filter_map(|e| match e {
            Event::Text(text) => Some(text.as_str()),
            _ => None,
        })
    }
}

/// A pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// The left button (or a trackpad click).
    Primary,
    /// The right button (or a two-finger click).
    Secondary,
    /// The middle button (or wheel press).
    Middle,
}

impl PointerButton {
    /// Its index in [`Pointer`]'s button arrays.
    pub const fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
            Self::Middle => 2,
        }
    }
}

/// The pointer: where it is and what its buttons do. Button arrays are
/// indexed by [`PointerButton::index`]. A helper named as one of egui's
/// `PointerState`'s reads the same; [`Pointer::moved`] has its own name
/// because egui's `is_moving` is smoothed over several frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pointer {
    /// Where it is, while it is over the window.
    pub hover: Option<Pos>,
    /// Where it acts: where it is, or where it last was while a button is
    /// held (a touch that lifted).
    pub interact: Option<Pos>,
    /// Where the button now held went down.
    pub press_origin: Option<Pos>,
    /// How far it moved this frame.
    pub delta: Vec2,
    /// Which buttons are held.
    pub down: [bool; 3],
    /// Which buttons went down this frame.
    pub pressed: [bool; 3],
    /// Which buttons came up this frame.
    pub released: [bool; 3],
}

impl Pointer {
    /// Where it is, while it is over the window.
    pub fn hover_pos(&self) -> Option<Pos> {
        self.hover
    }

    /// Where it acts (see [`Pointer::interact`]).
    pub fn interact_pos(&self) -> Option<Pos> {
        self.interact
    }

    /// Where the held button went down.
    pub fn press_origin(&self) -> Option<Pos> {
        self.press_origin
    }

    /// How far it moved this frame.
    pub fn delta(&self) -> Vec2 {
        self.delta
    }

    /// Whether it moved this frame.
    pub fn moved(&self) -> bool {
        self.delta != Vec2::ZERO
    }

    /// Whether it is over the window.
    pub fn has_pointer(&self) -> bool {
        self.hover.is_some()
    }

    /// Whether `button` is held.
    pub fn button_down(&self, button: PointerButton) -> bool {
        self.down[button.index()]
    }

    /// Whether `button` went down this frame.
    pub fn button_pressed(&self, button: PointerButton) -> bool {
        self.pressed[button.index()]
    }

    /// Whether `button` came up this frame.
    pub fn button_released(&self, button: PointerButton) -> bool {
        self.released[button.index()]
    }

    /// Whether any button is held.
    pub fn any_down(&self) -> bool {
        self.down.contains(&true)
    }

    /// Whether any button went down this frame.
    pub fn any_pressed(&self) -> bool {
        self.pressed.contains(&true)
    }

    /// Whether any button came up this frame.
    pub fn any_released(&self) -> bool {
        self.released.contains(&true)
    }

    /// Whether the primary button is held.
    pub fn primary_down(&self) -> bool {
        self.button_down(PointerButton::Primary)
    }

    /// Whether the primary button went down this frame.
    pub fn primary_pressed(&self) -> bool {
        self.button_pressed(PointerButton::Primary)
    }

    /// Whether the primary button came up this frame.
    pub fn primary_released(&self) -> bool {
        self.button_released(PointerButton::Primary)
    }

    /// Whether the secondary button is held.
    pub fn secondary_down(&self) -> bool {
        self.button_down(PointerButton::Secondary)
    }

    /// Whether the secondary button went down this frame.
    pub fn secondary_pressed(&self) -> bool {
        self.button_pressed(PointerButton::Secondary)
    }

    /// Whether the secondary button came up this frame.
    pub fn secondary_released(&self) -> bool {
        self.button_released(PointerButton::Secondary)
    }

    /// Whether the middle button is held.
    pub fn middle_down(&self) -> bool {
        self.button_down(PointerButton::Middle)
    }
}

/// The modifier keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Alt, or Option on a Mac.
    pub alt: bool,
    /// Control (on a Mac too).
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Command on a Mac (always false elsewhere).
    pub mac_cmd: bool,
    /// The platform's command key: Command on a Mac, Control elsewhere.
    /// Shortcuts test this one.
    pub command: bool,
}

impl Modifiers {
    /// None held.
    pub const NONE: Modifiers = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        mac_cmd: false,
        command: false,
    };
    /// Shift alone.
    pub const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..Self::NONE
    };
    /// Alt alone.
    pub const ALT: Modifiers = Modifiers {
        alt: true,
        ..Self::NONE
    };
    /// The command key alone (as a shortcut means it; see
    /// [`Modifiers::command`]).
    pub const COMMAND: Modifiers = Modifiers {
        command: true,
        ..Self::NONE
    };

    /// Whether none is held.
    pub fn is_none(&self) -> bool {
        !(self.alt || self.ctrl || self.shift || self.mac_cmd || self.command)
    }

    /// Whether any is held.
    pub fn any(&self) -> bool {
        !self.is_none()
    }

    /// Whether the held keys are a shortcut's `wanted` ones: Alt, Shift
    /// and the platform's command key ([`Modifiers::command`]) must each
    /// agree; Control and Command themselves are not compared, so one
    /// shortcut works on every platform.
    pub fn matches(&self, wanted: Modifiers) -> bool {
        self.alt == wanted.alt && self.shift == wanted.shift && self.command == wanted.command
    }
}

/// How a scroll's distance is measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollUnit {
    /// Points.
    #[default]
    Point,
    /// Lines of text.
    Line,
    /// Pages.
    Page,
}

/// One thing that happened, in [`Input::events`].
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A key went down or came up.
    Key {
        /// Which.
        key: Key,
        /// Down (`true`) or up.
        pressed: bool,
        /// A held key's repeat.
        repeat: bool,
        /// The modifiers held with it.
        modifiers: Modifiers,
    },
    /// Text was typed (one or more characters, no control characters).
    Text(String),
    /// The copy shortcut.
    Copy,
    /// The cut shortcut.
    Cut,
    /// The paste shortcut, with the clipboard's text.
    Paste(String),
    /// The pointer moved to here.
    PointerMoved(Pos),
    /// A pointer button went down or came up.
    PointerButton {
        /// Where.
        pos: Pos,
        /// Which.
        button: PointerButton,
        /// Down (`true`) or up.
        pressed: bool,
        /// The modifiers held with it.
        modifiers: Modifiers,
    },
    /// The pointer left the window.
    PointerGone,
    /// The wheel or trackpad scrolled (unsmoothed; [`Input::scroll`] is
    /// the smoothed total).
    Scroll {
        /// How far.
        delta: Vec2,
        /// In what.
        unit: ScrollUnit,
        /// The modifiers held.
        modifiers: Modifiers,
    },
    /// The window gained (`true`) or lost the keyboard.
    WindowFocused(bool),
}

/// A key, by what it means (its logical key on the user's layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs)] // The variants are the keys' names.
pub enum Key {
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Space,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    /// `-`.
    Minus,
    /// `.`.
    Period,
    /// Insert (Help on some Mac keyboards).
    Insert,
    /// Any other key, by the host's own code for it: compare it only with
    /// another `Other` from the same host.
    Other(u32),
}

impl Key {
    /// The key's name as shortcuts and key bindings spell it (`"A"`, `"7"`,
    /// `"Escape"`, `"Down"`); `None` for [`Key::Other`].
    pub const fn name(self) -> Option<&'static str> {
        Some(match self {
            Self::Escape => "Escape",
            Self::Enter => "Enter",
            Self::Tab => "Tab",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::ArrowUp => "Up",
            Self::ArrowDown => "Down",
            Self::ArrowLeft => "Left",
            Self::ArrowRight => "Right",
            Self::Home => "Home",
            Self::End => "End",
            Self::Space => "Space",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Minus => "Minus",
            Self::Period => "Period",
            Self::Insert => "Insert",
            Self::Other(_) => return None,
        })
    }
}

/// The navigation keys a focused widget keeps instead of letting them
/// move the focus ([`Ui::hold_keys`](crate::Ui::hold_keys)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyFilter {
    /// Tab and Shift+Tab.
    pub tab: bool,
    /// Left and right.
    pub horizontal_arrows: bool,
    /// Up and down.
    pub vertical_arrows: bool,
    /// Escape.
    pub escape: bool,
}

impl KeyFilter {
    /// Every navigation key.
    pub const ALL: KeyFilter = KeyFilter {
        tab: true,
        horizontal_arrows: true,
        vertical_arrows: true,
        escape: true,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn keys_and_text_read_from_the_events() {
        let input = Input {
            events: vec![
                key(Key::Z, true, Modifiers::COMMAND),
                Event::Text("ab".into()),
                key(Key::Tab, false, Modifiers::NONE),
                Event::Text("c".into()),
            ],
            ..Input::default()
        };
        assert!(input.key_pressed(Key::Z));
        assert!(input.key_pressed_with(Key::Z, Modifiers::COMMAND));
        assert!(!input.key_pressed_with(Key::Z, Modifiers::NONE));
        assert!(!input.key_pressed(Key::Tab) && input.key_released(Key::Tab));
        assert_eq!(input.typed().collect::<String>(), "abc");
    }

    #[test]
    fn pointer_buttons_by_name() {
        let mut pointer = Pointer::default();
        pointer.down[PointerButton::Secondary.index()] = true;
        pointer.pressed[0] = true;
        assert!(pointer.secondary_down() && !pointer.primary_down());
        assert!(pointer.primary_pressed() && pointer.any_pressed() && !pointer.any_released());
        assert!(!pointer.has_pointer() && !pointer.moved());
    }
}

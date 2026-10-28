//! Heads-up value fields: small boxes beside what they measure while a
//! drawing tool runs (a line's length and angle, a rectangle's width and
//! height).
//!
//! A field shows its **live** value (what the pointer gives) until typed
//! into. Typing goes to the **focused** field: the first character
//! replaces the live text, later ones append, and the field **locks**
//! (drawn filled): the tool then follows the typed value. Tab locks the
//! focused field at its live value and moves on; Shift+Tab goes back;
//! Backspace edits, and a field emptied unlocks. Enter and Escape are
//! reported for the tool to place or cancel. The tool reads the locked
//! texts, parses them (expressions, units, parameter names) and marks a
//! field it can't read with [`ValueField::error`], which the host draws in
//! the error color with the reason as its tooltip.
//!
//! [`ValueFields::type_events`] is the whole keyboard behaviour and draws
//! nothing, so tools and tests drive it without a frame.
//! [`Ui::value_fields`](crate::Ui::value_fields) has the host draw the
//! boxes where the tool places them and, while shown, hold the keyboard
//! (Tab, the arrows and Escape included), so no other widget takes keys
//! meant for them.
use crate::input::{Event, Key};

/// The longest text a field takes, in bytes.
const MOST: usize = 64;

/// One field (see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValueField {
    /// Shown before the value when two fields measure the same kind.
    pub label: Option<String>,
    /// What the pointer gives now, formatted by the tool.
    pub live: String,
    /// What was typed (shown instead of `live` while locked).
    pub text: String,
    /// Whether the field holds a typed value.
    pub locked: bool,
    /// Why the typed text can't be read (set by the tool).
    pub error: Option<String>,
}

impl ValueField {
    /// What the field shows.
    pub fn shown(&self) -> &str {
        if self.locked { &self.text } else { &self.live }
    }
}

/// What typing did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    /// No key concerned the fields.
    None,
    /// A field's text, lock or the focus changed.
    Changed,
    /// Enter: place where the fields say.
    Enter,
    /// Escape: the tool cancels (the typed values, then the shape).
    Escape,
}

/// A tool's fields, in Tab order, and the focused one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValueFields {
    /// The fields, in Tab order.
    pub fields: Vec<ValueField>,
    /// The focused field's index.
    pub focus: usize,
}

impl ValueFields {
    /// `count` fields, none typed, the first focused.
    pub fn new(count: usize) -> Self {
        ValueFields {
            fields: vec![ValueField::default(); count],
            focus: 0,
        }
    }

    /// Whether any field holds a typed value.
    pub fn any_locked(&self) -> bool {
        self.fields.iter().any(|f| f.locked)
    }

    /// The typed text of field `i`, while it is locked.
    pub fn locked(&self, i: usize) -> Option<&str> {
        self.fields
            .get(i)
            .filter(|f| f.locked)
            .map(|f| f.text.as_str())
    }

    /// Unlocks every field (the shape was placed or dropped).
    pub fn clear(&mut self) {
        for f in &mut self.fields {
            f.text.clear();
            f.locked = false;
            f.error = None;
        }
        self.focus = 0;
    }

    /// Takes the keyboard `events` (see the module docs). Keys other than
    /// text, Tab, Backspace, Enter and Escape are left alone.
    pub fn type_events(&mut self, events: &[Event]) -> Typing {
        let mut out = Typing::None;
        let n = self.fields.len();
        if n == 0 {
            return out;
        }
        self.focus = self.focus.min(n - 1);
        for event in events {
            match event {
                Event::Text(text) if !text.chars().any(char::is_control) => {
                    let f = &mut self.fields[self.focus];
                    if !f.locked {
                        f.text.clear();
                        f.locked = true;
                    }
                    if f.text.len() + text.len() <= MOST {
                        f.text.push_str(text);
                    }
                    f.error = None;
                    out = Typing::Changed;
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    Key::Tab => {
                        let f = &mut self.fields[self.focus];
                        if !modifiers.shift && !f.locked {
                            f.text = f.live.clone();
                            f.locked = true;
                        }
                        self.focus = if modifiers.shift {
                            (self.focus + n - 1) % n
                        } else {
                            (self.focus + 1) % n
                        };
                        out = Typing::Changed;
                    }
                    Key::Backspace => {
                        let f = &mut self.fields[self.focus];
                        if f.locked {
                            f.text.pop();
                            if f.text.is_empty() {
                                f.locked = false;
                            }
                            f.error = None;
                            out = Typing::Changed;
                        }
                    }
                    Key::Enter => return Typing::Enter,
                    Key::Escape => return Typing::Escape,
                    _ => {}
                },
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;

    fn key(key: Key, shift: bool) -> Event {
        Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::NONE
            },
        }
    }

    fn text(t: &str) -> Event {
        Event::Text(t.into())
    }

    #[test]
    fn typing_replaces_then_appends_and_locks() {
        let mut v = ValueFields::new(2);
        v.fields[0].live = "12.5".into();
        assert_eq!(v.fields[0].shown(), "12.5");
        assert_eq!(v.type_events(&[text("6"), text("0")]), Typing::Changed);
        assert_eq!(v.locked(0), Some("60"));
        assert_eq!(v.fields[0].shown(), "60");
        // Tab moves on; the second field is still live.
        v.fields[1].live = "33".into();
        v.type_events(&[key(Key::Tab, false)]);
        assert_eq!(v.focus, 1);
        assert_eq!(v.locked(1), None);
        v.type_events(&[text("40")]);
        assert_eq!(v.locked(1), Some("40"));
        assert_eq!(v.type_events(&[key(Key::Enter, false)]), Typing::Enter);
    }

    #[test]
    fn tab_locks_the_live_value_and_shift_tab_goes_back() {
        let mut v = ValueFields::new(3);
        v.fields[0].live = "7".into();
        v.type_events(&[key(Key::Tab, false)]);
        assert_eq!(v.locked(0), Some("7"));
        assert_eq!(v.focus, 1);
        v.type_events(&[key(Key::Tab, true)]);
        assert_eq!(v.focus, 0);
        assert_eq!(v.locked(1), None, "Shift+Tab locks nothing");
        v.type_events(&[key(Key::Tab, true)]);
        assert_eq!(v.focus, 2, "it wraps");
    }

    #[test]
    fn backspace_edits_and_an_emptied_field_unlocks() {
        let mut v = ValueFields::new(1);
        v.fields[0].live = "3".into();
        v.type_events(&[text("12")]);
        v.fields[0].error = Some("unknown".into());
        v.type_events(&[key(Key::Backspace, false)]);
        assert_eq!(v.locked(0), Some("1"));
        assert_eq!(v.fields[0].error, None, "an edit clears the error");
        v.type_events(&[key(Key::Backspace, false)]);
        assert_eq!(v.locked(0), None);
        assert_eq!(v.fields[0].shown(), "3", "live again");
        assert_eq!(v.type_events(&[key(Key::Backspace, false)]), Typing::None);
        assert_eq!(v.type_events(&[key(Key::Escape, false)]), Typing::Escape);
        v.type_events(&[text("5")]);
        v.clear();
        assert!(!v.any_locked());
    }

    #[test]
    fn control_text_and_overlong_typing_are_ignored() {
        let mut v = ValueFields::new(1);
        assert_eq!(v.type_events(&[text("\u{8}")]), Typing::None);
        v.type_events(&[text(&"9".repeat(MOST))]);
        v.type_events(&[text("1")]);
        assert_eq!(v.locked(0).map(str::len), Some(MOST));
        assert_eq!(ValueFields::new(0).type_events(&[text("1")]), Typing::None);
    }
}

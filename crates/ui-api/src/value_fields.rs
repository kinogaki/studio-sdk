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
//! [`Ui::value_fields`](crate::Ui::value_fields) has the host draw the
//! boxes where the tool places them and, while shown, hold the keyboard
//! (Tab, the arrows and Escape included), so no other widget takes keys
//! meant for them.
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
}

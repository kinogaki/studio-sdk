//! [`Edits`]: what the user did to a sheet this frame, and the
//! [`Action`]s that are not new values.

/// Something the user did that is not a new value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A [`Kind::Button`](super::Kind::Button) was pressed.
    Press,
    /// A [`Kind::Expression`](super::Kind::Expression) field's pick button.
    Pick,
    /// A button of [`Kind::Buttons`](super::Kind::Buttons) or a chip of
    /// [`Kind::Chips`](super::Kind::Chips).
    Index(usize),
    /// The field's material connection button
    /// ([`Socket`](super::Socket)).
    Socket,
    /// The trailing icon.
    Trailing,
    /// The tree path picker's Relative paths toggle, now this.
    Relative(bool),
    /// The vector's uniform lock, or the channels' link, now this.
    Lock(bool),
    /// A [`Kind::Code`](super::Kind::Code) field's grip set its height, now
    /// this many lines (`None`: sized by its text). The field's kind holds
    /// it already.
    Resize(Option<u32>),
    /// A [`Kind::Wires`](super::Kind::Wires) row was dragged: wire `from`
    /// goes to place `to`.
    Reorder {
        /// The wire's place before.
        from: usize,
        /// Its place after.
        to: usize,
    },
    /// A [`Kind::Wires`](super::Kind::Wires) row's remove button: that
    /// wire goes.
    Remove(usize),
}

/// What the user did to a schema this frame. Changed fields have their new
/// values in the schema already; the owner applies them.
///
/// One undo step spans a gesture, from `started` to `finished` (a drag, a
/// text edit, a picker opened and closed).
#[derive(Clone, Debug, PartialEq)]
pub struct Edits<K> {
    /// The fields whose values changed.
    pub changed: Vec<K>,
    /// A gesture began.
    pub started: bool,
    /// The gesture ended.
    pub finished: bool,
    /// The field whose gesture began this frame (an owner names its undo
    /// step after it).
    pub active: Option<K>,
    /// Actions taken, by field.
    pub actions: Vec<(K, Action)>,
    /// The text being typed in a field, while it has focus.
    pub typing: Option<(K, String)>,
}

impl<K> Default for Edits<K> {
    fn default() -> Self {
        Edits {
            changed: Vec::new(),
            started: false,
            finished: false,
            active: None,
            actions: Vec::new(),
            typing: None,
        }
    }
}

impl<K: PartialEq> Edits<K> {
    /// Nothing happened.
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && !self.started && !self.finished && self.actions.is_empty()
    }

    /// Whether the field at `key` changed.
    pub fn changed(&self, key: &K) -> bool {
        self.changed.contains(key)
    }

    /// The action taken on `key`, if any.
    pub fn action(&self, key: &K) -> Option<Action> {
        self.actions.iter().find(|(k, _)| k == key).map(|(_, a)| *a)
    }
}

impl<K> Edits<K> {
    /// These edits with their keys mapped by `f`; edits whose key `f` maps
    /// to `None` are dropped.
    pub fn filter_map<J>(self, f: impl Fn(K) -> Option<J>) -> Edits<J> {
        Edits {
            changed: self.changed.into_iter().filter_map(&f).collect(),
            started: self.started,
            finished: self.finished,
            active: self.active.and_then(&f),
            actions: self
                .actions
                .into_iter()
                .filter_map(|(k, a)| f(k).map(|k| (k, a)))
                .collect(),
            typing: self.typing.and_then(|(k, t)| f(k).map(|k| (k, t))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_read_and_map() {
        let edits = Edits {
            changed: vec![1, 2],
            started: true,
            active: Some(2),
            actions: vec![(3, Action::Press)],
            typing: Some((2, "1.5".into())),
            ..Edits::default()
        };
        assert!(!edits.is_empty() && edits.changed(&2) && !edits.changed(&3));
        assert_eq!(edits.action(&3), Some(Action::Press));
        let mapped = edits.filter_map(|k| (k != 1).then_some(k * 10));
        assert_eq!(mapped.changed, [20]);
        assert_eq!(mapped.active, Some(20));
        assert_eq!(mapped.actions, [(30, Action::Press)]);
        assert_eq!(mapped.typing, Some((20, "1.5".into())));
        assert!(Edits::<u8>::default().is_empty());
    }
}

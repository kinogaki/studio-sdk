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
    /// A keyable field ([`Field::keyable`](super::Field::keyable)) was
    /// Alt+clicked: its label (`None`), and the host sets or removes the
    /// keys of every component at the current time; or the field of
    /// component `Some(c)` (0 for a field of one value), and the host keys
    /// that component.
    Key(Option<usize>),
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

/// How a sheet's gesture ended: what it did stands, or goes back (Escape
/// ends typing or cancels a drag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditEnd {
    /// What the gesture did applies.
    Commit,
    /// The values the gesture began with stay: the owner puts back what it
    /// changed.
    Cancel,
}

/// A sheet's gesture open as a frame began.
#[derive(Clone, Debug, PartialEq)]
pub struct Open<K> {
    /// Its field: its changes this frame are the gesture's (`None`: the field
    /// is no longer drawn, and the gesture ends).
    pub field: Option<K>,
    /// How this frame ended it: committed (released, Enter, a click
    /// elsewhere, its field out of view or gone, another field's gesture
    /// begun), or cancelled (Escape: the owner puts back what it changed).
    pub ended: Option<EditEnd>,
}

/// What the user did to a schema this frame. Changed fields have their new
/// values in the schema already; the owner applies them.
///
/// A sheet has one gesture at a time (a drag, a burst of typing, a picker
/// open): one undo step from its beginning to its end. A frame tells, in
/// order: the gesture open as it began, its field's changes and how it ended
/// ([`Open`]); the changes of other fields, each a step of its own; a gesture
/// begun on a field ([`Edits::begun`]), and that field's changes, its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Edits<K> {
    /// The fields whose values changed.
    pub changed: Vec<K>,
    /// The gesture open as the frame began.
    pub open: Option<Open<K>>,
    /// The field a gesture began on this frame (an owner names its undo step
    /// after it); its changes this frame are the new gesture's.
    pub begun: Option<K>,
    /// Actions taken, by field.
    pub actions: Vec<(K, Action)>,
    /// The text being typed in a field, while it has focus.
    pub typing: Option<(K, String)>,
}

impl<K> Default for Edits<K> {
    fn default() -> Self {
        Edits {
            changed: Vec::new(),
            open: None,
            begun: None,
            actions: Vec::new(),
            typing: None,
        }
    }
}

impl<K: PartialEq> Edits<K> {
    /// Nothing happened.
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty()
            && self.open.as_ref().is_none_or(|o| o.ended.is_none())
            && self.begun.is_none()
            && self.actions.is_empty()
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
            open: self.open.map(|o| Open {
                field: o.field.and_then(&f),
                ended: o.ended,
            }),
            begun: self.begun.and_then(&f),
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
            open: Some(Open {
                field: Some(1),
                ended: Some(EditEnd::Commit),
            }),
            begun: Some(2),
            actions: vec![(3, Action::Press)],
            typing: Some((2, "1.5".into())),
        };
        assert!(!edits.is_empty() && edits.changed(&2) && !edits.changed(&3));
        assert_eq!(edits.action(&3), Some(Action::Press));
        let mapped = edits.filter_map(|k| (k != 1).then_some(k * 10));
        assert_eq!(mapped.changed, [20]);
        assert_eq!(mapped.begun, Some(20));
        let open = mapped.open.expect("the open gesture");
        assert_eq!((open.field, open.ended), (None, Some(EditEnd::Commit)));
        assert_eq!(mapped.actions, [(30, Action::Press)]);
        assert_eq!(mapped.typing, Some((20, "1.5".into())));
        assert!(Edits::<u8>::default().is_empty());
    }
}

/// Where a row's menu opened ([`RowMenu`](crate::RowMenu)), and whether it
/// opened this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuAt {
    /// The component whose field was clicked (0 for a field of one value, a
    /// vector's or color's component); `None` for the label or a control of
    /// the whole row.
    pub part: Option<usize>,
    /// It opened this frame: what it shows is built now and kept while it
    /// stays open.
    pub opened: bool,
}

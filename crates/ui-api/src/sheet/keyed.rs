//! [`FieldKey`]: the one key type the host's sheet takes, and [`Keyed`],
//! which turns any owner's keys into it and back.
use super::{Edits, Schema};
use std::hash::{DefaultHasher, Hash, Hasher};

/// A field's key as the host's sheet sees it: a hash of the owner's key.
///
/// The host derives a row's widget id from it, so a row keeps its state
/// (focus, an open dropdown, a drag) while rows above it come and go.
/// [`UiExt::sheet_keyed`](crate::UiExt::sheet_keyed) makes them from any
/// hashable key; build them by hand only for a `Ui` implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldKey(pub u64);

/// An owner's schema with its keys swapped for [`FieldKey`]s, and the way
/// back. Two keys that hash alike get distinct field keys (the later one
/// rehashed), so every field stays addressable.
pub struct Keyed<K> {
    /// The owner's keys, in field order, with their field keys.
    keys: Vec<(FieldKey, K)>,
}

impl<K: Clone + PartialEq + Hash> Keyed<K> {
    /// `schema` keyed by [`FieldKey`], and the map back.
    pub fn new(schema: Schema<K>) -> (Self, Schema<FieldKey>) {
        let mut keys: Vec<(FieldKey, K)> = Vec::new();
        let keyed = schema.map(|key| {
            let mut salt = 0u32;
            let mut field = hash(&key, salt);
            while keys.iter().any(|(f, _)| *f == field) {
                salt += 1;
                field = hash(&key, salt);
            }
            keys.push((field, key));
            field
        });
        (Keyed { keys }, keyed)
    }

    /// The owner's key of `field`, if it is one of the schema's.
    pub fn key(&self, field: FieldKey) -> Option<&K> {
        self.keys.iter().find(|(f, _)| *f == field).map(|(_, k)| k)
    }

    /// The schema back with the owner's keys. The host changes a sheet's
    /// values only, never its fields' keys.
    pub fn restore(&self, schema: Schema<FieldKey>) -> Schema<K> {
        schema.map(|field| {
            self.key(field)
                .cloned()
                .expect("the host keeps a sheet's field keys")
        })
    }

    /// The edits with the owner's keys; edits naming unknown fields are
    /// dropped.
    pub fn edits(&self, edits: Edits<FieldKey>) -> Edits<K> {
        edits.filter_map(|field| self.key(field).cloned())
    }
}

/// The field key of `key`, salted after a collision.
fn hash<K: Hash>(key: &K, salt: u32) -> FieldKey {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    if salt > 0 {
        salt.hash(&mut hasher);
    }
    FieldKey(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Field, Number};
    use super::*;

    fn schema(names: &[&'static str]) -> Schema<&'static str> {
        let mut s = Schema::new();
        s.section("main", "Main", true);
        for &name in names {
            s.push(Field::number(name, name, 1.0, Number::new()));
        }
        s
    }

    #[test]
    fn field_keys_follow_the_key_not_the_row() {
        let (_, a) = Keyed::new(schema(&["width", "height"]));
        let (_, b) = Keyed::new(schema(&["depth", "width", "height"]));
        let keys = |s: &Schema<FieldKey>| s.fields().map(|f| f.key).collect::<Vec<_>>();
        assert_eq!(keys(&a), keys(&b)[1..]);
    }

    #[test]
    fn schemas_and_edits_come_back_with_the_owners_keys() {
        let (keyed, mut sheet) = Keyed::new(schema(&["width", "height"]));
        let height = sheet.fields().nth(1).unwrap().key;
        for f in sheet.fields_mut() {
            if f.key == height {
                f.value = Some(2.5.into());
            }
        }
        let edits = Edits {
            changed: vec![height, FieldKey(7)],
            actions: vec![(height, Action::Press)],
            ..Edits::default()
        };
        let back = keyed.restore(sheet);
        assert_eq!(back.number(&"height"), Some(2.5));
        assert_eq!(back.number(&"width"), Some(1.0));
        let edits = keyed.edits(edits);
        assert_eq!(edits.changed, ["height"]);
        assert_eq!(edits.action(&"height"), Some(Action::Press));
    }

    #[test]
    fn equal_hashes_get_distinct_field_keys() {
        // Two fields with one key: both stay addressable.
        let (keyed, sheet) = Keyed::new(schema(&["same", "same"]));
        let keys: Vec<_> = sheet.fields().map(|f| f.key).collect();
        assert_ne!(keys[0], keys[1]);
        assert_eq!(keyed.restore(sheet).fields().count(), 2);
    }
}

//! [`Schema`]: sections, headings and fields, top to bottom.
use super::{Kind, Number, Text, Trailing};
use kinogaki_studio_api::params::Value;

/// What a property sheet shows, top to bottom. `K` is the owner's key for
/// a field (an enum naming its slots, a parameter's name).
#[derive(Clone, Debug, PartialEq)]
pub struct Schema<K> {
    /// Sections, headings and fields, in order.
    pub items: Vec<Item<K>>,
}

impl<K> Default for Schema<K> {
    fn default() -> Self {
        Schema { items: Vec::new() }
    }
}

// Fields are nearly every item: boxing them would allocate per field.
/// One row of a schema.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Item<K> {
    /// A collapsible section band; its fields follow until the next band.
    Section(Section),
    /// A band that does not collapse (a settings page's group).
    Heading(Text),
    /// A labelled property.
    Field(Field<K>),
}

/// A section or subsection band.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Remembers whether the band is open, for every schema with a section
    /// of this key.
    pub key: Text,
    /// The band's text.
    pub label: Text,
    /// Whether it starts open.
    pub open: bool,
    /// A subsection inside the current section.
    pub sub: bool,
}

/// One labelled property.
#[derive(Clone, Debug, PartialEq)]
pub struct Field<K> {
    /// The owner's key for the field; edits name fields by it.
    pub key: K,
    /// The label in the left column.
    pub label: Text,
    /// Its type and presentation.
    pub kind: Kind,
    /// The current value; the host writes edits into it. `None` for a
    /// kind that holds none (a button).
    pub value: Option<Value>,
    /// A disabled field shows its value greyed and takes no input; the
    /// reason is its tooltip.
    pub enabled: bool,
    /// Hover text (the reason, while disabled).
    pub tooltip: Option<Text>,
    /// Extra words a settings search accepts.
    pub keywords: Text,
    /// The value is invalid (an expression that does not parse).
    pub error: bool,
    /// Several selected items differ here.
    pub mixed: bool,
    /// The slot at the field's right edge.
    pub trailing: Trailing,
}

impl<K> Field<K> {
    /// A field of `kind` holding `value`, enabled.
    pub fn new(key: K, label: impl Into<Text>, kind: Kind, value: Option<Value>) -> Self {
        Field {
            key,
            label: label.into(),
            kind,
            value,
            enabled: true,
            tooltip: None,
            keywords: Text::Borrowed(""),
            error: false,
            mixed: false,
            trailing: Trailing::None,
        }
    }

    /// A number field.
    pub fn number(key: K, label: impl Into<Text>, value: f64, number: Number) -> Self {
        Self::new(key, label, Kind::Number(number), Some(Value::Number(value)))
    }

    /// A vector field with axis letters.
    pub fn vector(key: K, label: impl Into<Text>, values: &[f64], number: Number) -> Self {
        Self::new(
            key,
            label,
            Kind::Vector { number, lock: None },
            Some(Value::Numbers(values.to_vec())),
        )
    }

    /// A working-space color field (RGB or RGBA).
    pub fn color(key: K, label: impl Into<Text>, rgb: &[f64]) -> Self {
        Self::new(key, label, Kind::Color, Some(Value::Numbers(rgb.to_vec())))
    }

    /// A toggle.
    pub fn toggle(key: K, label: impl Into<Text>, on: bool) -> Self {
        Self::new(key, label, Kind::Toggle, Some(Value::Bool(on)))
    }

    /// Segmented choices, at `at`.
    pub fn choice(key: K, label: impl Into<Text>, at: usize, items: Vec<Text>) -> Self {
        Self::new(key, label, Kind::Choice(items), Some(Value::Index(at)))
    }

    /// A dropdown of `items`, at `at`.
    pub fn dropdown(key: K, label: impl Into<Text>, at: usize, items: Vec<Text>) -> Self {
        Self::new(
            key,
            label,
            Kind::Dropdown {
                items,
                available: Vec::new(),
                info: None,
            },
            Some(Value::Index(at)),
        )
    }

    /// A dropdown over `all`, each named by `name`, at `value` (the first
    /// item when `value` is not among them): the usual field for an enum.
    pub fn dropdown_of<T: Copy + PartialEq>(
        key: K,
        label: impl Into<Text>,
        all: &[T],
        value: T,
        name: impl Fn(T) -> &'static str,
    ) -> Self {
        let items = all.iter().map(|&t| name(t).into()).collect();
        let at = all.iter().position(|&t| t == value).unwrap_or(0);
        Self::dropdown(key, label, at, items)
    }

    /// A text field, committed when the edit ends.
    pub fn text(key: K, label: impl Into<Text>, text: impl Into<String>) -> Self {
        Self::new(key, label, Kind::Text, Some(Value::Text(text.into())))
    }

    /// A code field holding `text` (see [`Kind::Code`]).
    pub fn code(key: K, label: impl Into<Text>, text: impl Into<String>) -> Self {
        Self::new(
            key,
            label,
            Kind::Code {
                diagnostics: Vec::new(),
                fill: false,
                lines: None,
            },
            Some(Value::Text(text.into())),
        )
    }

    /// Read-only text, right-aligned.
    pub fn info(key: K, label: impl Into<Text>, text: impl Into<String>) -> Self {
        Self::new(key, label, Kind::Info, Some(Value::Text(text.into())))
    }

    /// Read-only text, left-aligned.
    pub fn value(key: K, label: impl Into<Text>, text: impl Into<String>) -> Self {
        Self::new(key, label, Kind::Value, Some(Value::Text(text.into())))
    }

    /// A button with `text`.
    pub fn button(key: K, label: impl Into<Text>, text: impl Into<Text>) -> Self {
        Self::new(
            key,
            label,
            Kind::Button {
                text: text.into(),
                icon: None,
            },
            None,
        )
    }

    /// Enabled or not, with `reason` as its tooltip while disabled.
    pub fn enabled(mut self, enabled: bool, reason: Option<impl Into<Text>>) -> Self {
        self.enabled = enabled;
        if !enabled && let Some(reason) = reason {
            self.tooltip = Some(reason.into());
        }
        self
    }

    /// With hover text `tooltip`.
    pub fn tooltip(mut self, tooltip: impl Into<Text>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// With extra words a settings search accepts.
    pub fn keywords(mut self, keywords: impl Into<Text>) -> Self {
        self.keywords = keywords.into();
        self
    }

    /// Marked invalid.
    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }

    /// Marked as differing between selected items.
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    /// With `trailing` at its right edge.
    pub fn trailing(mut self, trailing: Trailing) -> Self {
        self.trailing = trailing;
        self
    }

    /// The field with its key mapped by `f`.
    pub fn map<J>(self, f: impl FnOnce(K) -> J) -> Field<J> {
        Field {
            key: f(self.key),
            label: self.label,
            kind: self.kind,
            value: self.value,
            enabled: self.enabled,
            tooltip: self.tooltip,
            keywords: self.keywords,
            error: self.error,
            mixed: self.mixed,
            trailing: self.trailing,
        }
    }
}

impl<K> Schema<K> {
    /// An empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a collapsible section; `key` remembers whether it is open.
    pub fn section(&mut self, key: impl Into<Text>, label: impl Into<Text>, open: bool) {
        self.items.push(Item::Section(Section {
            key: key.into(),
            label: label.into(),
            open,
            sub: false,
        }));
    }

    /// Starts a collapsible subsection inside the current section.
    pub fn subsection(&mut self, key: impl Into<Text>, label: impl Into<Text>, open: bool) {
        self.items.push(Item::Section(Section {
            key: key.into(),
            label: label.into(),
            open,
            sub: true,
        }));
    }

    /// Starts a band that does not collapse.
    pub fn heading(&mut self, label: impl Into<Text>) {
        self.items.push(Item::Heading(label.into()));
    }

    /// Appends `field`.
    pub fn push(&mut self, field: Field<K>) {
        self.items.push(Item::Field(field));
    }

    /// Every field, in order.
    pub fn fields(&self) -> impl Iterator<Item = &Field<K>> {
        self.items.iter().filter_map(|i| match i {
            Item::Field(f) => Some(f),
            _ => None,
        })
    }

    /// Every field, in order, to change.
    pub fn fields_mut(&mut self) -> impl Iterator<Item = &mut Field<K>> {
        self.items.iter_mut().filter_map(|i| match i {
            Item::Field(f) => Some(f),
            _ => None,
        })
    }

    /// Appends `other`'s items with keys mapped by `f`.
    pub fn extend<J>(&mut self, other: Schema<J>, f: impl Fn(J) -> K) {
        self.items
            .extend(other.items.into_iter().map(|item| match item {
                Item::Section(s) => Item::Section(s),
                Item::Heading(h) => Item::Heading(h),
                Item::Field(field) => Item::Field(field.map(&f)),
            }));
    }

    /// The schema with every field's key mapped by `f`, in order.
    pub fn map<J>(self, mut f: impl FnMut(K) -> J) -> Schema<J> {
        Schema {
            items: self
                .items
                .into_iter()
                .map(|item| match item {
                    Item::Section(s) => Item::Section(s),
                    Item::Heading(h) => Item::Heading(h),
                    Item::Field(field) => Item::Field(field.map(&mut f)),
                })
                .collect(),
        }
    }
}

impl<K: Clone> Schema<K> {
    /// The fields whose keys `f` maps (and every band), with mapped keys:
    /// the part of a combined schema one owner applies.
    pub fn filter_map<J>(&self, f: impl Fn(&K) -> Option<J>) -> Schema<J> {
        Schema {
            items: self
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Section(s) => Some(Item::Section(s.clone())),
                    Item::Heading(h) => Some(Item::Heading(h.clone())),
                    Item::Field(field) => {
                        let key = f(&field.key)?;
                        Some(Item::Field(field.clone().map(|_| key)))
                    }
                })
                .collect(),
        }
    }
}

impl<K: PartialEq> Schema<K> {
    /// The field at `key`.
    pub fn field(&self, key: &K) -> Option<&Field<K>> {
        self.fields().find(|f| f.key == *key)
    }

    /// The value at `key`.
    pub fn value(&self, key: &K) -> Option<&Value> {
        self.field(key).and_then(|f| f.value.as_ref())
    }

    /// The number at `key` (a number, slider or angle field).
    pub fn number(&self, key: &K) -> Option<f64> {
        self.value(key).and_then(Value::number)
    }

    /// The numbers at `key` (a vector or color field).
    pub fn numbers(&self, key: &K) -> Option<Vec<f64>> {
        self.value(key)
            .and_then(Value::numbers)
            .map(<[f64]>::to_vec)
    }

    /// The chosen item at `key` (a dropdown or segmented field).
    pub fn index(&self, key: &K) -> Option<usize> {
        self.value(key).and_then(Value::index)
    }

    /// Whether the toggle at `key` is on.
    pub fn on(&self, key: &K) -> Option<bool> {
        self.value(key).and_then(Value::bool)
    }

    /// The text at `key`.
    pub fn text(&self, key: &K) -> Option<String> {
        self.value(key).and_then(Value::text).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Key {
        Width,
        Cap,
        Mode,
        Go,
    }

    fn schema() -> Schema<Key> {
        let mut s = Schema::new();
        s.section("shape", "Shape", true);
        s.push(Field::number(Key::Width, "Width", 2.0, Number::length()));
        s.push(Field::toggle(Key::Cap, "Cap", true).enabled(false, Some("Closed shapes only")));
        s.subsection("more", "More", false);
        s.push(Field::dropdown_of(
            Key::Mode,
            "Mode",
            &[1, 2, 3],
            3,
            |n| match n {
                1 => "One",
                2 => "Two",
                _ => "Three",
            },
        ));
        s.push(Field::button(Key::Go, "", "Go"));
        s
    }

    #[test]
    fn values_read_by_key() {
        let s = schema();
        assert_eq!(s.number(&Key::Width), Some(2.0));
        assert_eq!(s.on(&Key::Cap), Some(true));
        assert_eq!(s.index(&Key::Mode), Some(2));
        assert_eq!(s.value(&Key::Go), None);
        let cap = s.field(&Key::Cap).unwrap();
        assert!(!cap.enabled);
        assert_eq!(cap.tooltip.as_deref(), Some("Closed shapes only"));
        assert_eq!(s.fields().count(), 4);
    }

    #[test]
    fn keys_map_and_filter() {
        let s = schema();
        let only_width = s.filter_map(|k| (*k == Key::Width).then_some("w"));
        assert_eq!(only_width.fields().count(), 1);
        assert_eq!(only_width.items.len(), 3, "bands stay");
        let mut combined: Schema<String> = Schema::new();
        combined.extend(s.clone(), |k| format!("{k:?}"));
        assert_eq!(combined.number(&"Width".to_owned()), Some(2.0));
        let indexed = s.map(|k| k as u8);
        assert_eq!(indexed.index(&2), Some(2));
    }
}

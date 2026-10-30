//! [`Schema`]: how parameters are presented, as data. The host turns it
//! into its inspector's rows; edits come back as new [`Value`]s.
use super::{Parameters, Value};

/// What a parameter dialog or inspector shows, top to bottom: sections and
/// parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    /// Sections and parameters, in order. A parameter belongs to the
    /// section before it; parameters before the first section have none.
    pub items: Vec<SchemaItem>,
}

impl Schema {
    /// With a section band `label` (remembered open or closed by `key`)
    /// next.
    pub fn section(mut self, key: impl Into<String>, label: impl Into<String>, open: bool) -> Self {
        self.items.push(SchemaItem::Section(Section {
            key: key.into(),
            label: label.into(),
            open,
        }));
        self
    }

    /// With `parameter` next.
    pub fn parameter(mut self, parameter: Parameter) -> Self {
        self.items.push(SchemaItem::Parameter(parameter));
        self
    }

    /// The parameters, in order.
    pub fn parameters(&self) -> impl Iterator<Item = &Parameter> {
        self.items.iter().filter_map(|item| match item {
            SchemaItem::Parameter(p) => Some(p),
            SchemaItem::Section(_) => None,
        })
    }

    /// Every parameter's value by name.
    pub fn values(&self) -> Parameters {
        self.parameters()
            .map(|p| (p.name.clone(), p.value.clone()))
            .collect()
    }
}

/// One row of a [`Schema`].
#[derive(Clone, Debug, PartialEq)]
pub enum SchemaItem {
    /// A collapsible section band; the parameters after it, up to the next
    /// band, are in it.
    Section(Section),
    /// A parameter.
    Parameter(Parameter),
}

/// A collapsible section band.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Remembers whether the band is open, across schemas.
    pub key: String,
    /// The band's text.
    pub label: String,
    /// Whether it starts open.
    pub open: bool,
}

/// One parameter: its name, how it shows, and its current value.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// Its stored name (`distance`, `merge_shells`), unique in its schema.
    pub name: String,
    /// What its row says.
    pub label: String,
    /// What it holds and how it shows.
    pub kind: ParameterKind,
    /// Its current value (the default, in a description).
    pub value: Value,
    /// Its tooltip; empty for none.
    pub help: String,
    /// More words a search finds it by ("orientation" for an up axis);
    /// empty for none.
    pub keywords: String,
    /// Whether it takes input; a disabled row shows its value greyed and
    /// its help says why.
    pub enabled: bool,
}

impl Parameter {
    /// Parameter `name` labelled `label`, of `kind`, at `value`: enabled,
    /// with no help.
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        kind: ParameterKind,
        value: impl Into<Value>,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind,
            value: value.into(),
            help: String::new(),
            keywords: String::new(),
            enabled: true,
        }
    }

    /// With `help` as its tooltip.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Found by search under `keywords` too.
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = keywords.into();
        self
    }

    /// Disabled (or enabled again).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// What a parameter holds and how it shows.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterKind {
    /// [`Value::Number`]: a real number with a unit and limits.
    Number(Number),
    /// [`Value::Number`] holding a whole number in `min..=max`, with step
    /// arrows.
    Integer {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
    },
    /// [`Value::Bool`].
    Toggle,
    /// [`Value::Index`] into these entries, shown as segments or a menu.
    Choice(Vec<String>),
    /// [`Value::Text`], one line, committed when the edit ends.
    Text,
    /// [`Value::Text`]: code, many lines in a code editor.
    Code,
    /// [`Value::Text`]: a file's path, with a browse button.
    File,
    /// [`Value::Text`]: a free path to any node or object of the scene,
    /// with a picker of the scene's tree.
    Path,
    /// [`Value::Text`]: a path to what `RefFilter` says fits (objects of
    /// some kinds, results of some data types, Materials); the host offers
    /// what fits and resolves the path from the node holding it.
    Reference(super::RefFilter),
    /// [`Value::Text`]: attribute names (a name or a pattern) on the
    /// node's input, with a menu of the input's attributes.
    Attribute,
    /// [`Value::Text`]: a group, an expression over the input's components
    /// in the node's own syntax, which a pick in the viewport can write.
    /// The node resolves it when it cooks.
    Group,
    /// [`Value::Numbers`]: three channels of the scene's working-space RGB,
    /// shown as a swatch.
    Color,
    /// [`Value::Numbers`]: `components` numbers (2 to 4) lettered X, Y,
    /// Z, W, each shown and moved as `number`.
    Vector {
        /// How many components, 2 to 4.
        components: u8,
        /// How each component shows and moves.
        number: Number,
    },
    /// [`Value::Numbers`]: a float or color ramp from 0 to 1, as
    /// [`ramp::encode`](super::ramp::encode) writes it; its length fixes how
    /// many keys it holds.
    Ramp {
        /// A color ramp (r, g, b a key) rather than a float ramp.
        color: bool,
    },
    /// [`Value::Numbers`]: tone curves, one a channel, as
    /// [`tone_curve::encode`](super::tone_curve::encode) writes them with
    /// `capacity` keys a curve.
    ToneCurves {
        /// The channels, in curve order.
        channels: Vec<CurveChannel>,
        /// The most keys a curve holds.
        capacity: usize,
    },
    /// [`Value::Numbers`]: a palette, r, g, b a color (display code values,
    /// 0–1), at most `capacity` colors.
    Palette {
        /// The most colors it holds.
        capacity: usize,
    },
    /// [`Value::Numbers`] (4): R, G, B and A, each shown and moved as
    /// `number`. The host may show them as one value while they are
    /// linked (sheet state it keeps, not a value).
    Channels(Number),
    /// [`Value::Texts`]: names the node keeps (the outputs it writes),
    /// listed, not edited.
    Texts,
}

/// One channel of [`ParameterKind::ToneCurves`]: its name and its curve's
/// tint.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveChannel {
    /// What its tab says ("Red").
    pub label: String,
    /// The curve's tint, linear RGB; `None` draws it in the text color.
    pub tint: Option<[f64; 3]>,
}

impl ParameterKind {
    /// Whether `value` is of the variant this kind holds (and, for vectors
    /// and colors, has the right count).
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Number(_) | Self::Integer { .. }, Value::Number(_)) => true,
            (Self::Toggle, Value::Bool(_)) => true,
            (Self::Choice(items), Value::Index(i)) => *i < items.len(),
            (
                Self::Text
                | Self::Code
                | Self::File
                | Self::Path
                | Self::Reference(_)
                | Self::Attribute
                | Self::Group,
                Value::Text(_),
            ) => true,
            (Self::Color, Value::Numbers(v)) => v.len() == 3,
            (Self::Vector { components, .. }, Value::Numbers(v)) => v.len() == *components as usize,
            (Self::Ramp { color }, Value::Numbers(v)) => {
                let stride = super::ramp::stride(*color);
                v.len() > stride && (v.len() - 1) % stride == 0
            }
            (Self::ToneCurves { channels, capacity }, Value::Numbers(v)) => {
                v.len() == channels.len() * super::tone_curve::stride(*capacity)
            }
            (Self::Palette { capacity }, Value::Numbers(v)) => {
                v.len().is_multiple_of(3) && v.len() / 3 <= *capacity
            }
            (Self::Channels(_), Value::Numbers(v)) => v.len() == 4,
            (Self::Texts, Value::Texts(_)) => true,
            _ => false,
        }
    }
}

/// How a number shows: its unit, limits and look.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    /// What it measures.
    pub unit: Unit,
    /// The smallest value; typed and dragged values clamp to it.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// The span dragging and the slider cover, when narrower than the
    /// limits; typing may leave it.
    pub soft: Option<(f64, f64)>,
    /// Shown as a slider filling the soft span (or the limits).
    pub slider: bool,
    /// Most decimal places shown; `None` leaves it to the host.
    pub decimals: Option<u8>,
    /// How much it changes per point dragged; `None` leaves it to the host.
    pub speed: Option<f64>,
    /// A length whose default is a real size in millimetres (a
    /// primitive's dimensions): the host gives a new node, and a reset to
    /// the default, that size in the document's unit.
    pub real_size: bool,
}

impl Number {
    /// An unlimited number of `unit`.
    pub fn new(unit: Unit) -> Self {
        Self {
            unit,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            soft: None,
            slider: false,
            decimals: None,
            speed: None,
            real_size: false,
        }
    }

    /// A plain unlimited number.
    pub fn plain() -> Self {
        Self::new(Unit::Plain)
    }

    /// An unlimited length.
    pub fn length() -> Self {
        Self::new(Unit::Length)
    }

    /// An unlimited angle.
    pub fn angle() -> Self {
        Self::new(Unit::Angle)
    }

    /// With the limits `min..=max`.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        (self.min, self.max) = (min, max);
        self
    }

    /// With the soft span `min..=max`.
    pub fn soft(mut self, min: f64, max: f64) -> Self {
        self.soft = Some((min, max));
        self
    }

    /// Shown as a slider.
    pub fn slider(mut self) -> Self {
        self.slider = true;
        self
    }

    /// Changing by `speed` per point dragged.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// A length whose default is a real size in millimetres
    /// ([`Number::real_size`](Number#structfield.real_size)).
    pub fn real_size(mut self) -> Self {
        self.unit = Unit::Length;
        self.real_size = true;
        self
    }

    /// Showing at most `places` decimals.
    pub fn decimals(mut self, places: u8) -> Self {
        self.decimals = Some(places);
        self
    }
}

/// What a number measures. The host shows and parses it in the user's
/// units (a length in the project's millimetres or inches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    /// A plain number.
    #[default]
    Plain,
    /// A length, stored in scene units.
    Length,
    /// A length stored and always shown in millimetres, whatever the
    /// project's unit (a machine's settings).
    Millimeters,
    /// An angle, stored in degrees.
    Angle,
    /// A percentage, stored as shown (0–100).
    Percent,
    /// A plain number shown and typed with this suffix after it (`" px"`,
    /// `" EV"`, `" K"`).
    Suffix(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_schema_lists_its_parameters_and_values() {
        let schema = Schema::default()
            .section("shape", "Shape", true)
            .parameter(Parameter::new(
                "distance",
                "Distance",
                ParameterKind::Number(Number::length().range(0.0, f64::INFINITY)),
                1.5,
            ))
            .parameter(
                Parameter::new("cap", "Cap", ParameterKind::Toggle, true).help("Close the ends"),
            );
        let names: Vec<&str> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["distance", "cap"]);
        assert_eq!(schema.values()["distance"], Value::Number(1.5));
        assert!(schema.parameters().all(|p| p.kind.accepts(&p.value)));
    }

    #[test]
    fn kinds_accept_their_values_only() {
        let choice = ParameterKind::Choice(vec!["A".into(), "B".into()]);
        assert!(choice.accepts(&Value::Index(1)) && !choice.accepts(&Value::Index(2)));
        assert!(ParameterKind::Color.accepts(&Value::Numbers(vec![0.0; 3])));
        assert!(!ParameterKind::Color.accepts(&Value::Numbers(vec![0.0; 4])));
        let vector = ParameterKind::Vector {
            components: 2,
            number: Number::plain(),
        };
        assert!(vector.accepts(&Value::Numbers(vec![0.0; 2])));
        assert!(!ParameterKind::Toggle.accepts(&Value::Number(1.0)));
    }
}

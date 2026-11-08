//! [`Schema`]: how parameters are presented, as data: sections, and rows
//! (parameters and buttons) placed in them by their `group`. It is the one
//! shape of every set of parameters a host draws: a node type's parameters
//! and spares, a format's options, an engine's settings, a prim type's
//! values and a material node's inputs. Each holder takes its schema in its
//! builder and gives it out by a reader; the host turns it into rows, and
//! edits come back as new [`Value`](crate::Value)s.
//!
//! **Row order.** An inspector shows the rows in no section first, in the
//! order they were added; then each section, in the order sections were
//! added, with its rows in the order they were added ([`Schema::placed`]).
//! A row's section is the one its `group` names; a row whose group names no
//! section of its schema is in none (the registry refuses such a row in
//! what a package declares). A section shows its band only over a row it
//! shows.
use crate::condition::Condition;
use crate::parameter::Parameter;
use crate::value::Parameters;
use kinogaki_geometry::Error;
use std::collections::BTreeSet;

/// Sections, and the rows placed in them by their group (the module docs
/// say in what order a host shows them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    sections: Vec<Section>,
    rows: Vec<SchemaRow>,
}

impl Schema {
    /// With `section` after the sections it has; one named as a section it
    /// has is left out.
    pub fn section(mut self, section: Section) -> Self {
        if !self.sections.iter().any(|s| s.name == section.name) {
            self.sections.push(section);
        }
        self
    }

    /// With `parameter` after the rows it has, in the section its group
    /// names.
    pub fn parameter(mut self, parameter: Parameter) -> Self {
        self.rows.push(SchemaRow::Parameter(parameter));
        self
    }

    /// With `button` after the rows it has, in the section its group names.
    pub fn button(mut self, button: Button) -> Self {
        self.rows.push(SchemaRow::Button(button));
        self
    }

    /// With `other`'s sections and rows after its own, each but those named
    /// as one it has (a node's spares after its type's rows, an engine's
    /// section on a standard prim type, the types several engines declare
    /// as one).
    pub fn merged(mut self, other: Schema) -> Self {
        for section in other.sections {
            self = self.section(section);
        }
        for row in other.rows {
            if !self.rows.iter().any(|r| r.name() == row.name()) {
                self.rows.push(row);
            }
        }
        self
    }

    /// Its sections, in the order they were added.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Its rows, in the order they were added.
    pub fn rows(&self) -> &[SchemaRow] {
        &self.rows
    }

    /// Its rows in the order a host shows them, each with its section (the
    /// module docs give the rule).
    pub fn placed(&self) -> impl Iterator<Item = (Option<&Section>, &SchemaRow)> {
        placed(&self.sections, &self.rows, SchemaRow::group)
    }

    /// Its parameters, in the order they were added.
    pub fn parameters(&self) -> impl Iterator<Item = &Parameter> {
        self.rows.iter().filter_map(SchemaRow::parameter)
    }

    /// Its parameters, to change (a node type filling a menu from its
    /// input).
    pub fn parameters_mut(&mut self) -> impl Iterator<Item = &mut Parameter> {
        self.rows.iter_mut().filter_map(|row| match row {
            SchemaRow::Parameter(p) => Some(p),
            SchemaRow::Button(_) => None,
        })
    }

    /// Only the rows `keep` keeps.
    pub fn retain(&mut self, keep: impl FnMut(&SchemaRow) -> bool) {
        self.rows.retain(keep);
    }

    /// Every parameter's value by name: its default, in a description.
    pub fn values(&self) -> Parameters {
        (self.parameters())
            .map(|p| (p.name.clone(), p.value.clone()))
            .collect()
    }

    /// `values` made to fit its parameters: each parameter's value from
    /// `values` when it takes it, else its own. Values no parameter names
    /// are dropped.
    pub fn fit(&self, values: &Parameters) -> Parameters {
        (self.parameters())
            .map(|p| {
                let v = (values.get(&p.name))
                    .filter(|v| p.accepts(v))
                    .unwrap_or(&p.value);
                (p.name.clone(), v.clone())
            })
            .collect()
    }

    /// Whether every value of `values` names a parameter that takes it.
    ///
    /// # Errors
    /// One names none, or its parameter refuses it.
    pub fn validate(&self, values: &Parameters) -> Result<(), Error> {
        for (name, value) in values {
            let p = (self.parameters())
                .find(|p| p.name == *name)
                .ok_or_else(|| Error::invalid(format_args!("unknown parameter {name:?}")))?;
            p.check(value)?;
        }
        Ok(())
    }

    /// Whether it is whole, as the registry admits what a package declares:
    /// each row's name once, each row's group naming one of its sections
    /// (or empty), and each parameter's value one it takes.
    ///
    /// # Errors
    /// What isn't, naming the row.
    pub fn check(&self) -> Result<(), Error> {
        let mut names = BTreeSet::new();
        for row in &self.rows {
            let name = row.name();
            if !names.insert(name) {
                return Err(Error::invalid(format_args!("{name} is declared twice")));
            }
            let group = row.group();
            if !group.is_empty() && !self.sections.iter().any(|s| s.name == group) {
                return Err(Error::invalid(format_args!(
                    "{name} sits in section {group}, which isn't declared"
                )));
            }
            if let SchemaRow::Parameter(p) = row
                && let Err(why) = p.check(&p.value)
            {
                return Err(Error::invalid(format_args!(
                    "{name} defaults to a value it refuses: {}",
                    why.message()
                )));
            }
        }
        Ok(())
    }
}

/// `rows` in the order a host shows rows (the module docs give the rule),
/// each with the section of `sections` its `group` names: a [`Schema`]'s,
/// and the rows a host lays out of its own in sections (a subnet's
/// interface).
pub fn placed<'a, R>(
    sections: &'a [Section],
    rows: &'a [R],
    group: impl Fn(&R) -> &str + Copy + 'a,
) -> impl Iterator<Item = (Option<&'a Section>, &'a R)> {
    let section = move |row: &R| sections.iter().find(|s| s.name == group(row));
    let loose = (rows.iter())
        .filter(move |row| section(row).is_none())
        .map(|row| (None, row));
    let held = sections.iter().flat_map(move |s| {
        (rows.iter())
            .filter(move |row| group(row) == s.name)
            .map(move |row| (Some(s), row))
    });
    loose.chain(held)
}

/// One row of a [`Schema`].
// Rows are few and short-lived (a sheet's, while it shows): a parameter
// kept inline is simpler to match than a boxed one.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum SchemaRow {
    /// A parameter.
    Parameter(Parameter),
    /// A button: it holds no value; pressing it asks the node's type
    /// (`NodeType::pressed`).
    Button(Button),
}

impl SchemaRow {
    /// Its name: the parameter's, or what the button tells its node's type.
    pub fn name(&self) -> &str {
        match self {
            SchemaRow::Parameter(p) => &p.name,
            SchemaRow::Button(b) => &b.name,
        }
    }

    /// The section it sits in, by name; empty for none.
    pub fn group(&self) -> &str {
        match self {
            SchemaRow::Parameter(p) => &p.group,
            SchemaRow::Button(b) => &b.group,
        }
    }

    /// The parameter, when it is one.
    pub fn parameter(&self) -> Option<&Parameter> {
        match self {
            SchemaRow::Parameter(p) => Some(p),
            SchemaRow::Button(_) => None,
        }
    }
}

/// A collapsible section band rows sit in.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Its name: what a row's group names, and what remembers whether the
    /// band is open.
    pub name: String,
    /// The band's text.
    pub label: String,
    /// Whether it starts open.
    pub open: bool,
    /// Open whatever `open` says while this holds (a gobo's section while
    /// its toggle is on).
    open_when: Option<Condition>,
}

impl Section {
    /// Section `name` titled `label`, open when `open`.
    pub fn new(name: impl Into<String>, label: impl Into<String>, open: bool) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            open,
            open_when: None,
        }
    }

    /// Open, whatever [`open`](Self::open) says, while `when` holds.
    pub fn open_when(mut self, when: Condition) -> Self {
        self.open_when = Some(when);
        self
    }

    /// Whether it starts open, its parameters holding `values`.
    pub fn is_open(&self, values: &Parameters) -> bool {
        self.open || self.open_when.as_ref().is_some_and(|c| c.met(values))
    }
}

/// A button row of a [`Schema`].
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// What the node's type is told was pressed, unique in its schema.
    pub name: String,
    /// What the button says.
    pub label: String,
    /// Its tooltip; empty for none.
    pub help: String,
    /// Whether it can be pressed now; a disabled button's help says why.
    pub enabled: bool,
    /// The section it sits in, by name; empty for none.
    pub group: String,
}

impl Button {
    /// Button `name` saying `label`: enabled, in no section, with no help.
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            help: String::new(),
            enabled: true,
            group: String::new(),
        }
    }

    /// With `help` as its tooltip.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Disabled (or enabled again).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// In section `group`.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::{Number, ValueKind};
    use crate::value::Value;

    fn number(name: &str) -> Parameter {
        Parameter::number(name, name, Number::plain().range(0.0, 10.0), 1.0)
    }

    /// The rows in no section come first, then each section's in the
    /// sections' order, each row in the order it was added, whatever order
    /// the rows and sections were added in.
    #[test]
    fn rows_show_loose_first_then_by_section_in_the_order_added() {
        let schema = Schema::default()
            .parameter(number("b1").group("b"))
            .section(Section::new("a", "A", true))
            .parameter(number("loose1"))
            .parameter(number("a1").group("a"))
            .section(Section::new("b", "B", false))
            .button(Button::new("press", "Press").group("a"))
            .parameter(number("stray").group("nowhere"))
            .parameter(number("b2").group("b"));
        let placed: Vec<(Option<&str>, &str)> = (schema.placed())
            .map(|(s, row)| (s.map(|s| s.name.as_str()), row.name()))
            .collect();
        assert_eq!(
            placed,
            [
                (None, "loose1"),
                (None, "stray"),
                (Some("a"), "a1"),
                (Some("a"), "press"),
                (Some("b"), "b1"),
                (Some("b"), "b2"),
            ]
        );
        let names: Vec<&str> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["b1", "loose1", "a1", "stray", "b2"]);
        assert_eq!(schema.values()["a1"], Value::Number(1.0));
    }

    /// A merge adds what isn't held, by name, after what is: the first
    /// section and row of a name stay.
    #[test]
    fn a_merge_adds_the_sections_and_rows_not_held_after_its_own() {
        let mine = Schema::default()
            .section(Section::new("light", "Light", true))
            .parameter(number("intensity").group("light"));
        let theirs = Schema::default()
            .section(Section::new("light", "Lamp", false))
            .section(Section::new("gobo", "Gobo", false))
            .parameter(Parameter::toggle("intensity", "Other", true))
            .parameter(Parameter::toggle("gobo", "Gobo", false).group("gobo"));
        let merged = mine.merged(theirs);
        let sections: Vec<&str> = merged.sections().iter().map(|s| s.label.as_str()).collect();
        assert_eq!(sections, ["Light", "Gobo"]);
        let rows: Vec<&str> = merged.parameters().map(|p| p.label.as_str()).collect();
        assert_eq!(rows, ["intensity", "Gobo"]);
        assert_eq!(
            merged.clone().section(Section::new("gobo", "Again", true)),
            merged
        );
    }

    /// Values fit and validate against the parameters by name.
    #[test]
    fn values_fit_and_validate_against_its_parameters() {
        let schema = Schema::default()
            .parameter(number("size"))
            .button(Button::new("reset", "Reset"));
        let values: Parameters = [
            ("size".to_owned(), Value::Number(20.0)),
            ("gone".to_owned(), Value::Number(2.0)),
        ]
        .into();
        assert_eq!(
            schema.values(),
            [("size".to_owned(), Value::Number(1.0))].into()
        );
        assert_eq!(schema.fit(&values), schema.values());
        assert!(schema.validate(&schema.values()).is_ok());
        let unknown = schema.validate(&values).unwrap_err();
        assert!(unknown.message().contains("gone") || unknown.message().contains("size"));
        let wide: Parameters = [("size".to_owned(), Value::Number(20.0))].into();
        assert!(schema.validate(&wide).is_err());
    }

    /// The registry's check: a name twice, a section not declared and a
    /// default the kind refuses are each refused, naming the row.
    #[test]
    fn a_check_refuses_a_name_twice_an_undeclared_section_and_a_refused_default() {
        let fine = Schema::default()
            .section(Section::new("a", "A", true))
            .parameter(number("x").group("a"))
            .button(Button::new("go", "Go"));
        assert!(fine.check().is_ok());
        let twice = fine.clone().parameter(Parameter::toggle("go", "Go", true));
        assert!(
            twice
                .check()
                .unwrap_err()
                .message()
                .contains("go is declared twice")
        );
        let stray = fine.clone().parameter(number("y").group("b"));
        assert!(stray.check().unwrap_err().message().contains("section b"));
        let mut wide = number("z");
        wide.value = Value::Number(20.0);
        let refused = fine.parameter(wide).check().unwrap_err();
        assert!(refused.message().contains("z defaults"));
        assert!(!ValueKind::Toggle.accepts(&Value::Number(1.0)));
    }

    /// Rows change in place and go by `retain`.
    #[test]
    fn rows_change_in_place_and_go_by_retain() {
        let mut schema = Schema::default()
            .parameter(number("a"))
            .parameter(number("b"))
            .button(Button::new("c", "C"));
        for p in schema.parameters_mut() {
            p.label = p.name.to_uppercase();
        }
        schema.retain(|row| row.name() != "b");
        let rows: Vec<&str> = schema.rows().iter().map(SchemaRow::name).collect();
        assert_eq!(rows, ["a", "c"]);
        assert_eq!(schema.parameters().next().unwrap().label, "A");
    }
}

//! A value's persistent names as one table: every distinct name once, a
//! name after the names it is made of, and entities, history and datums
//! naming them by their index in it.
//!
//! Each name is a tag in `name_tags` with its numbers in `name_words`
//! (whole numbers, read in order) and its texts in `name_texts`:
//! - Created (0): the operation, the role's tag, the role's fields;
//! - Part (1): the name split, the piece's index;
//! - Merged (2): how many, then each name.
//!
//! A role's fields, after its tag: Local (0) and Primitive (1) an index;
//! Imported (2) body and index; ImportedBody (3) a body; Cap (4) a text
//! and its end (0 start, 1 end); Side (5) a text, curve and segment;
//! Along (6), At (7) and Offset (8) a name; Copy (9) a name and index;
//! Between (10) how many faces, each face, and an index.
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_geometry::brep::{End, Name, NameNode, OperationId, Role};
use std::collections::HashMap;
use std::sync::Arc;

/// The table being written.
#[derive(Default)]
pub(super) struct Writer {
    index: HashMap<Name, u32>,
    tags: Vec<u8>,
    words: Vec<i64>,
    texts: Vec<String>,
}

impl Writer {
    /// `name`'s index, adding it (after what it is made of) when new.
    pub(super) fn name(&mut self, name: &Name) -> Result<u32, Error> {
        if let Some(&i) = self.index.get(name) {
            return Ok(i);
        }
        let (tag, words) = match name.node() {
            NameNode::Created { by, role } => {
                let mut words = vec![by.0 as i64];
                self.role(role, &mut words)?;
                (0, words)
            }
            NameNode::Part { of, index } => (1, vec![i64::from(self.name(of)?), i64::from(index)]),
            NameNode::Merged { of } => {
                let mut words = vec![of.len() as i64];
                for n in of {
                    words.push(i64::from(self.name(n)?));
                }
                (2, words)
            }
        };
        let i = self.tags.len() as u32;
        self.tags.push(tag);
        self.words.extend(words);
        self.index.insert(name.clone(), i);
        Ok(i)
    }

    /// Each of `names`' indices.
    pub(super) fn names<'n>(
        &mut self,
        names: impl IntoIterator<Item = &'n Name>,
    ) -> Result<Vec<u32>, Error> {
        names.into_iter().map(|n| self.name(n)).collect()
    }

    fn role(&mut self, role: &Role, w: &mut Vec<i64>) -> Result<(), Error> {
        let int = |v: u32| i64::from(v);
        match role {
            Role::Local { index } => w.extend([0, int(*index)]),
            Role::Primitive { index } => w.extend([1, int(*index)]),
            Role::Imported { body, index } => w.extend([2, int(*body), int(*index)]),
            Role::ImportedBody { body } => w.extend([3, int(*body)]),
            Role::Cap { profile, end } => {
                self.texts.push(profile.to_string());
                w.extend([4, i64::from(*end == End::End)]);
            }
            Role::Side {
                profile,
                curve,
                segment,
            } => {
                self.texts.push(profile.to_string());
                w.extend([5, int(*curve), int(*segment)]);
            }
            Role::Along(n) => w.extend([6, int(self.name(n)?)]),
            Role::At(n) => w.extend([7, int(self.name(n)?)]),
            Role::Offset(n) => w.extend([8, int(self.name(n)?)]),
            Role::Copy { of, index } => w.extend([9, int(self.name(of)?), int(*index)]),
            Role::Between { faces, index } => {
                w.extend([10, faces.len() as i64]);
                for f in faces.iter() {
                    w.push(int(self.name(f)?));
                }
                w.push(int(*index));
            }
            _ => return Err(Error::invalid("a name of a role this version can't save")),
        }
        Ok(())
    }

    pub(super) fn finish(self, out: &mut dyn ElementWriter) -> Result<(), Error> {
        out.set("name_tags", Field::U8s(self.tags))?;
        out.set("name_words", Field::I64s(self.words))?;
        out.set("name_texts", Field::Texts(self.texts))
    }
}

/// The table read, every name checked to be made of earlier ones.
pub(super) struct Table {
    at: String,
    names: Vec<Name>,
}

impl Table {
    pub(super) fn read(element: &dyn ElementReader) -> Result<Self, Error> {
        use super::super::attributes::{i64s, u8s};
        let at = element.location();
        let mut words = Cursor {
            at: &at,
            words: i64s(element, "name_words")?,
        };
        let mut texts = element.texts("name_texts")?.iter();
        let mut names: Vec<Name> = Vec::new();
        for &tag in u8s(element, "name_tags")? {
            let name = |words: &mut Cursor<'_>| -> Result<Name, Error> {
                let i = words.u32()? as usize;
                names
                    .get(i)
                    .cloned()
                    .ok_or_else(|| Error::invalid(format_args!("{at}: a name made of a later one")))
            };
            let made = match tag {
                0 => {
                    let by = OperationId(words.next()? as u64);
                    let role = match words.next()? {
                        0 => Role::Local {
                            index: words.u32()?,
                        },
                        1 => Role::Primitive {
                            index: words.u32()?,
                        },
                        2 => Role::Imported {
                            body: words.u32()?,
                            index: words.u32()?,
                        },
                        3 => Role::ImportedBody { body: words.u32()? },
                        4 => Role::Cap {
                            profile: text(&at, texts.next())?,
                            end: if words.u32()? == 1 {
                                End::End
                            } else {
                                End::Start
                            },
                        },
                        5 => Role::Side {
                            profile: text(&at, texts.next())?,
                            curve: words.u32()?,
                            segment: words.u32()?,
                        },
                        6 => Role::Along(name(&mut words)?),
                        7 => Role::At(name(&mut words)?),
                        8 => Role::Offset(name(&mut words)?),
                        9 => Role::Copy {
                            of: name(&mut words)?,
                            index: words.u32()?,
                        },
                        10 => {
                            let n = words.u32()? as usize;
                            let faces: Result<Arc<[Name]>, Error> =
                                (0..n).map(|_| name(&mut words)).collect();
                            Role::Between {
                                faces: faces?,
                                index: words.u32()?,
                            }
                        }
                        r => return Err(Error::invalid(format_args!("{at}: no role kind {r}"))),
                    };
                    Name::created(by, role)
                }
                1 => {
                    let of = name(&mut words)?;
                    Name::part(&of, words.u32()?)
                }
                2 => {
                    let n = words.u32()? as usize;
                    let of: Result<Vec<Name>, Error> = (0..n).map(|_| name(&mut words)).collect();
                    Name::merged(&of?)
                }
                t => return Err(Error::invalid(format_args!("{at}: no name kind {t}"))),
            };
            names.push(made);
        }
        if !words.words.is_empty() || texts.next().is_some() {
            return Err(Error::invalid(format_args!("{at}: the name table runs on")));
        }
        Ok(Self { at, names })
    }

    /// Name `i`.
    pub(super) fn get(&self, i: u32) -> Result<Name, Error> {
        self.names
            .get(i as usize)
            .cloned()
            .ok_or_else(|| Error::invalid(format_args!("{}: no name {i}", self.at)))
    }

    /// Each of `indices`' names.
    pub(super) fn all(&self, indices: &[u32]) -> Result<Vec<Name>, Error> {
        indices.iter().map(|&i| self.get(i)).collect()
    }
}

fn text(at: &str, t: Option<&String>) -> Result<Arc<str>, Error> {
    t.map(|t| Arc::from(t.as_str()))
        .ok_or_else(|| Error::invalid(format_args!("{at}: the name texts end early")))
}

/// Whole numbers read in order.
struct Cursor<'a> {
    at: &'a str,
    words: &'a [i64],
}

impl Cursor<'_> {
    fn next(&mut self) -> Result<i64, Error> {
        let (&w, rest) = self
            .words
            .split_first()
            .ok_or_else(|| Error::invalid(format_args!("{}: the name words end early", self.at)))?;
        self.words = rest;
        Ok(w)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let w = self.next()?;
        u32::try_from(w)
            .map_err(|_| Error::invalid(format_args!("{}: a name word out of range", self.at)))
    }
}

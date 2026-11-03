//! Stored values: package-owned values the host saves in the project or
//! the user's settings, headless.
//!
//! What a project keeps for a package outside its objects: CAD's
//! tolerances (one value for the project), CAD's
//! rows of each view (one value per view); and what a user keeps across
//! projects that cooks read (CAM's printers and filaments, one value for
//! the user). A package declares each as a
//! value type implementing [`Stored`] and registers it
//! ([`StoredValue::of`], [`Package::with_stored`]). The host keeps every
//! registered value in a [`StoredValues`] table (the project's, and one
//! per view), saves each into the project and reads it back.
//!
//! **In the project.** A [`Scope::Project`] value is the element of its
//! [`Stored::id`] among the project's settings; a [`Scope::View`]
//! value is that element in each view's. The element is typed with the id,
//! which names its package, so a project holding a value of a package that
//! isn't installed keeps it as read. Reading is strict: a field the value's
//! codec doesn't read refuses the project, as one missing does.
//!
//! **For the user.** A [`Scope::User`] value is that element in the
//! user's settings folder, kept beside the user's settings, never in a
//! project: a user without one starts from its defaults. Cooks read it
//! with the project's ([`Project::user`](crate::cook::Project::user)), and
//! a change recooks the nodes that read the project.
//!
//! Values are shared and copy-on-write, as session slots are: a clone of
//! the table shares each value.
//!
//! [`Package::with_stored`]: crate::Package::with_stored
use crate::element::{ElementReader, ElementWriter};
use crate::error::Error;
use crate::id::StoredId;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// Where a stored value belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One for the project (its settings).
    Project,
    /// One for each view of the project's viewport.
    View,
    /// One for the user, across projects.
    User,
}

/// A stored value's type, naming its element.
///
/// The host calls these on its UI thread or a project I/O worker; none
/// may block.
pub trait Stored: Any + Clone + PartialEq + fmt::Debug + Send + Sync {
    /// Its id (`kinogaki.cad.tolerances`), under its package's vendor and
    /// unique among every installed package's stored values: the
    /// registry refuses a package declaring one already declared.
    fn id() -> StoredId;
    /// Where it belongs.
    fn scope() -> Scope;
    /// The value a new project (or a new view) starts with.
    fn defaults() -> Self;
    /// Writes the value's fields into its element.
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error>;
    /// Reads a value from its element; fails on anything missing or
    /// unknown.
    fn read(from: &dyn ElementReader) -> Result<Self, Error>;
}

/// A shared stored value.
type Value = Arc<dyn Any + Send + Sync>;

/// A registered stored value: its id and scope, and what the host does
/// with a value of its type without knowing the type. Made from the value
/// type with [`StoredValue::of`]; cheap to copy.
#[derive(Clone, Copy)]
pub struct StoredValue {
    id: StoredId,
    scope: Scope,
    type_name: &'static str,
    defaults: fn() -> Value,
    copy: fn(&dyn Any) -> Value,
    eq: fn(&dyn Any, &dyn Any) -> bool,
    debug: fn(&dyn Any, &mut fmt::Formatter<'_>) -> fmt::Result,
    write: fn(&dyn Any, &mut dyn ElementWriter) -> Result<(), Error>,
    read: fn(&dyn ElementReader) -> Result<Value, Error>,
}

fn typed<T: Stored>(v: &dyn Any) -> &T {
    v.downcast_ref::<T>()
        .expect("a stored value of its own type")
}

impl StoredValue {
    /// The stored value `T` is.
    pub fn of<T: Stored>() -> Self {
        StoredValue {
            id: T::id(),
            scope: T::scope(),
            type_name: std::any::type_name::<T>(),
            defaults: || Arc::new(T::defaults()),
            copy: |v| Arc::new(typed::<T>(v).clone()),
            eq: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
            debug: |v, f| fmt::Debug::fmt(typed::<T>(v), f),
            write: |v, to| typed::<T>(v).write(to),
            read: |from| T::read(from).map(|v| Arc::new(v) as Value),
        }
    }

    /// Its id.
    pub fn id(&self) -> StoredId {
        self.id
    }

    /// Where it belongs.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Its value for a new project or view.
    pub fn defaults(&self) -> Arc<dyn Any + Send + Sync> {
        (self.defaults)()
    }

    /// Writes `value` into its element.
    pub fn write(&self, value: &dyn Any, to: &mut dyn ElementWriter) -> Result<(), Error> {
        (self.write)(value, to)
    }

    /// Reads a value from its element.
    pub fn read(&self, from: &dyn ElementReader) -> Result<Arc<dyn Any + Send + Sync>, Error> {
        (self.read)(from)
    }
}

impl fmt::Debug for StoredValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredValue")
            .field("id", &self.id)
            .field("scope", &self.scope)
            .field("type", &self.type_name)
            .finish()
    }
}

/// One stored value with its registration.
#[derive(Clone)]
struct Entry {
    stored: StoredValue,
    value: Value,
}

/// Stored values of one scope by id: the project's, or one view's.
/// Cloning shares every value; two tables are equal when they hold the
/// same values.
#[derive(Clone, Default)]
pub struct StoredValues(BTreeMap<StoredId, Entry>);

impl StoredValues {
    /// Each of `stored` at its defaults; an error naming the id when two
    /// of them share one (the registry admits no such packages, so a host
    /// table made from what it admitted never fails).
    pub fn new(stored: impl IntoIterator<Item = StoredValue>) -> Result<Self, Error> {
        let mut table = BTreeMap::new();
        for s in stored {
            let entry = Entry {
                stored: s,
                value: (s.defaults)(),
            };
            if table.insert(s.id, entry).is_some() {
                return Err(Error::invalid(format_args!(
                    "stored value {} is registered twice",
                    s.id
                )));
            }
        }
        Ok(Self(table))
    }

    /// The registrations held, in id order.
    pub fn stored(&self) -> impl Iterator<Item = StoredValue> + '_ {
        self.0.values().map(|e| e.stored)
    }

    /// Value `T`, when it is one of these.
    pub fn get<T: Stored>(&self) -> Option<&T> {
        self.0.get(&T::id())?.value.downcast_ref()
    }

    /// Value `T` to change, copied first while something else shares it;
    /// `None` when it isn't one of these.
    pub fn get_mut<T: Stored>(&mut self) -> Option<&mut T> {
        let entry = self.0.get_mut(&T::id())?;
        if Arc::get_mut(&mut entry.value).is_none() {
            entry.value = (entry.stored.copy)(&*entry.value);
        }
        Arc::get_mut(&mut entry.value)?.downcast_mut()
    }

    /// Value `id`, untyped.
    pub fn get_any(&self, id: StoredId) -> Option<&(dyn Any + Send + Sync)> {
        self.0.get(&id).map(|e| &*e.value)
    }

    /// Replaces value `id` with `value`; `false` (and nothing stored) when
    /// it isn't one of these or `value` is not of its type.
    pub fn set_any(&mut self, id: StoredId, value: Arc<dyn Any + Send + Sync>) -> bool {
        match self.0.get_mut(&id) {
            Some(entry) if (*entry.value).type_id() == (*value).type_id() => {
                entry.value = value;
                true
            }
            _ => false,
        }
    }

    /// Replaces value `T`; `false` when it isn't one of these.
    pub fn set<T: Stored>(&mut self, value: T) -> bool {
        self.set_any(T::id(), Arc::new(value))
    }
}

impl PartialEq for StoredValues {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(&other.0).all(|((ia, a), (ib, b))| {
                ia == ib && (Arc::ptr_eq(&a.value, &b.value) || (a.stored.eq)(&*a.value, &*b.value))
            })
    }
}

impl fmt::Debug for StoredValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Shown<'a>(&'a Entry);
        impl fmt::Debug for Shown<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                (self.0.stored.debug)(&*self.0.value, f)
            }
        }
        f.debug_map()
            .entries(self.0.iter().map(|(id, e)| (id, Shown(e))))
            .finish()
    }
}

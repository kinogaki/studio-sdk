//! A package's pages of the Settings window, over its stored values.
//!
//! A package keeps what it lets the user set as [`Stored`] values
//! (registered with [`Package::with_stored`]): the user's
//! ([`Scope::User`], saved with the user's settings) or the project's
//! ([`Scope::Project`], saved in the `.kino`). A [`SettingsPage`] shows
//! rows over the values of one scope and writes the edits it is told of
//! back into them ([`PageRows`]); it is registered with
//! [`UiPackage::settings`](super::UiPackage::settings).
//!
//! The host draws every page over a draft of that scope's values, lent the
//! package's UI state and the call's context as parts are, so a page can
//! show what is live (a device's status) and start what it shows. A user
//! page is a page of its own in the window, named by its label; a project
//! page is a section of Settings ▸ Project, headed by its label, shown
//! while a document is open. OK or Apply puts the drafts in place: the
//! user's values saved with the user's settings (never undone), the
//! project's as one edit of the document (one undo step); Cancel drops
//! them; Reset Page puts the page's package's values of its scope back
//! at their defaults. A value set outside the window takes the same path
//! ([`HostDoc::set_stored_any`](crate::host::HostDoc::set_stored_any)).
//!
//! [`Stored`]: kinogaki_studio_api::Stored
//! [`Scope::User`]: kinogaki_studio_api::Scope::User
//! [`Scope::Project`]: kinogaki_studio_api::Scope::Project
//! [`Package::with_stored`]: kinogaki_studio_api::Package::with_stored
use super::parts::part;
use crate::host::Cx;
use crate::sheet::{Edits, Schema};
use kinogaki_studio_api::StoredValues;
use std::fmt;

/// Whose values a settings page edits, and so where it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageScope {
    /// The user's ([`Scope::User`](kinogaki_studio_api::Scope::User)):
    /// a page of the window of its own.
    User,
    /// The open project's
    /// ([`Scope::Project`](kinogaki_studio_api::Scope::Project)): a section
    /// of Settings ▸ Project.
    Project,
}

/// A page of the Settings window: its scope, its label and its rows.
pub struct SettingsPage<S: ?Sized> {
    /// Whose values it edits.
    pub scope: PageScope,
    /// The page's name in the window (a user page) or its section's heading
    /// on Settings ▸ Project ("Color").
    pub label: String,
    /// Its rows over the draft of the scope's values.
    pub rows: Box<dyn PageRows<S>>,
}

impl<S: ?Sized> SettingsPage<S> {
    /// A page of `scope` labelled `label`, drawn by `rows`.
    pub fn new(
        scope: PageScope,
        label: impl Into<String>,
        rows: impl PageRows<S> + 'static,
    ) -> Self {
        Self {
            scope,
            label: label.into(),
            rows: Box::new(rows),
        }
    }
}

impl<S: ?Sized> fmt::Debug for SettingsPage<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsPage")
            .field("scope", &self.scope)
            .field("label", &self.label)
            .finish()
    }
}

part! {
    /// A settings page's rows over the draft of its scope's stored values
    /// (`values`: the user's or the project's, every installed package's).
    pub trait PageRows<S> {
        /// The rows, drawn by the host while the page shows.
        fn rows(&self, s: &mut S, values: &StoredValues, cx: &mut dyn Cx) -> Schema<u32>;
        /// The edits the host reported on `rows`, written into `values`.
        fn apply(
            &self,
            s: &mut S,
            values: &mut StoredValues,
            rows: &Schema<u32>,
            edits: &Edits<u32>,
            cx: &mut dyn Cx,
        );
    }
}

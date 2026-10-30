//! [`HostNetwork`]: objects' networks, which core owns, as a package's UI
//! reads and edits them.
use super::data::ObjectRef;
use kinogaki_studio_api::Error;
use kinogaki_studio_api::network::{NetworkEdit, NetworkRead};

/// The networks objects hold (an object of a kind naming a context holds
/// one of it). A package's tools change a network only through
/// [`edit_network`](Self::edit_network): the host makes each edit one undo
/// step (or part of the open gesture's), checks the network when the edit
/// is committed and cooks it again.
pub trait HostNetwork {
    /// Object `object`'s network; `None` when it holds none (or is gone).
    fn network(&self, object: ObjectRef) -> Option<&dyn NetworkRead>;

    /// Edits `object`'s network with `edit` as one undo step named `label`
    /// (inside the open gesture, its part of that step). When `edit` fails,
    /// or the edited network doesn't check, nothing changes and the error
    /// says why; when it changes nothing, no step is recorded.
    fn edit_network(
        &mut self,
        object: ObjectRef,
        label: &str,
        edit: &mut dyn FnMut(&mut dyn NetworkEdit) -> Result<(), Error>,
    ) -> Result<(), Error>;
}

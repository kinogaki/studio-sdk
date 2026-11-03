//! [`assert_refused`]: a package's admission refused, saying why.
use kinogaki_studio_api::{Error, ErrorKind};

/// Asserts that `result` is a package's refusal
/// ([`ErrorKind::Refused`]) whose message holds every one of `fragments`
/// (the package, the id, the reason: "acme.more declares node creation
/// acme.forest.oak, which acme.forest declared").
///
/// # Panics
/// When `result` is not a refusal, or its message lacks a fragment.
#[track_caller]
pub fn assert_refused(result: Result<(), Error>, fragments: &[&str]) {
    let error = match result {
        Err(error) if error.kind == ErrorKind::Refused => error,
        other => panic!("not refused: {other:?}"),
    };
    for fragment in fragments {
        assert!(
            error.message().contains(fragment),
            "{error} doesn't say {fragment:?}"
        );
    }
}

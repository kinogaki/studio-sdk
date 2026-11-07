//! [`Named`]: the files a file being read names, found and read by one
//! rule.
use crate::format::budget::ReadBudget;
use crate::format::options::EMBED;
use crate::format::{FormatOptions, Input, Reading};
use crate::{Diagnostic, Error};
use kinogaki_geometry::EmbeddedFile;
use kinogaki_geometry::image::ImageFile;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// How many names a warning lists before it counts the rest.
const LISTED: usize = 8;

/// The files a file names: an OBJ's material libraries and maps, a glTF's
/// buffers and images, a USD layer's layers and textures, a MaterialX
/// document's includes and images. Every format finds and reads them here,
/// by one rule:
///
/// - **Where.** Relative to the folder of the file being read (`.` and `..`
///   taken as written), or wherever an absolute name says: a file may name
///   files anywhere. A file held in memory has no folder: the files it
///   names are left unresolved, its pictures' names kept as written.
/// - **What.** Regular files alone: a device, a pipe or a folder is never
///   read, whoever names it.
/// - **How much.** A file is read whole, charged to the read's budget
///   ([`ReadBudget::take`]).
///
/// One `Named` serves a read and the workers it shares out to.
///
/// ```
/// use kinogaki_studio_api::format::files::Named;
/// use kinogaki_studio_api::format::{Input, Reading, Purpose};
/// use kinogaki_studio_api::{FormatOptions, Units};
/// # use kinogaki_studio_api::{color::ColorService, Error};
/// # #[derive(Debug)]
/// # struct Plain;
/// # impl ColorService for Plain {
/// #     fn working(&self) -> &str { "lin_rec709" }
/// #     fn convert(&self, _: &str, _: &str, _: &mut [[f64; 3]]) -> Result<(), Error> { Ok(()) }
/// #     fn revision(&self) -> u64 { 0 }
/// # }
/// let folder = std::env::temp_dir().join(format!("named-doc-{}", std::process::id()));
/// std::fs::create_dir_all(folder.join("maps")).unwrap();
/// std::fs::write(folder.join("maps/wood.png"), b"png").unwrap();
/// let scene = folder.join("scene.obj");
/// let reading = Reading::new(Purpose::Objects, Units::default(), &Plain);
/// let named = Named::new(&Input::file(&scene), &FormatOptions::new(), &reading);
/// let wood = named.file("maps/wood.png", None).unwrap();
/// assert_eq!(named.read(&wood).unwrap(), b"png");
/// assert!(named.file("/dev/zero", None).is_none(), "a device");
/// assert!(named.file("/dev/null", None).is_none());
/// assert_eq!(named.warnings().len(), 1, "one warning lists both");
/// # std::fs::remove_dir_all(&folder).unwrap();
/// ```
#[derive(Debug)]
pub struct Named {
    /// What messages call the file being read.
    file: String,
    /// Its folder, absolute and without `.` or `..`; `None` held in memory.
    folder: Option<PathBuf>,
    /// Embed Textures.
    embed: bool,
    budget: ReadBudget,
    /// What was left unresolved, by why.
    left: Mutex<Left>,
}

#[derive(Debug, Default)]
struct Left {
    special: BTreeSet<String>,
    held: BTreeSet<String>,
    missing: BTreeSet<String>,
    failed: BTreeSet<String>,
}

impl Named {
    /// The files `input` names, its pictures embedded as `chosen` says (its
    /// [`EMBED`] option, off where it has none), read within `reading`'s
    /// budget.
    pub fn new(input: &Input<'_>, chosen: &FormatOptions, reading: &Reading<'_>) -> Named {
        let on = |name| matches!(chosen.get(name), Some(crate::Value::Bool(true)));
        let folder = (input.path())
            .and_then(|p| std::path::absolute(p).ok())
            .and_then(|p| p.parent().map(normal));
        Named {
            file: input.to_string(),
            folder,
            embed: on(EMBED),
            budget: reading.budget.clone(),
            left: Mutex::default(),
        }
    }

    /// The folder of the file being read, which the names it gives are
    /// relative to; `None` for a file held in memory.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// The regular file `name` names, relative to `from` (the folder of
    /// the file giving the name; `None`: the file being read), by its
    /// canonical path (links followed), which is the file's identity: two
    /// names of one file give one path. `None` for a file out of reach, not
    /// regular, or not there, listed in [`warnings`](Self::warnings).
    pub fn file(&self, name: &str, from: Option<&Path>) -> Option<PathBuf> {
        self.first([name], from)
    }

    /// The first of `names` that [`file`](Self::file) finds: a name a file
    /// may mean in more than one way (an MTL library's `mtllib` value whole,
    /// then each of its words). Only when none is found is the first listed.
    pub fn first<'n>(
        &self,
        names: impl IntoIterator<Item = &'n str>,
        from: Option<&Path>,
    ) -> Option<PathBuf> {
        let mut left = None;
        for name in names.into_iter().filter(|n| !n.is_empty()) {
            match self.locate(name, from) {
                Located::Found(path) => return Some(path),
                Located::Absent(_) => _ = left.get_or_insert((name, Why::Missing)),
                Located::Left(why) => _ = left.get_or_insert((name, why)),
            }
        }
        if let Some((name, why)) = left {
            self.note(why, name);
        }
        None
    }

    /// The bytes of `path`, a file [`file`](Self::file) found: a regular
    /// file, read whole and charged to the read's budget.
    ///
    /// # Errors
    /// It can't be read, isn't a regular file, or is more than the budget
    /// has left; the message names it.
    pub fn read(&self, path: &Path) -> Result<Vec<u8>, Error> {
        let (file, len) = super::regular(path)?;
        (self.budget.take(len))
            .map_err(|e| Error::invalid(format_args!("{}: {e}", path.display())))?;
        super::whole(file, len, path)
    }

    /// The picture `name` names relative to `from` (as
    /// [`file`](Self::file)), as an Image node reads it: its path, or its
    /// bytes when Embed Textures is on. A picture that isn't there stays a
    /// path, which its Image node says is missing; a name in a file held in
    /// memory stays as written (a relative one is the project folder's to
    /// find). `None` for what isn't a regular file, listed.
    pub fn image(&self, name: &str, from: Option<&Path>) -> Option<ImageFile> {
        if name.is_empty() {
            return None;
        }
        if from.is_none() && self.folder.is_none() {
            let written = normal(Path::new(name)).display().to_string();
            return Some(ImageFile::Path(Arc::from(written)));
        }
        let path = match self.locate(name, from) {
            Located::Found(found) if self.embed => match self.read(&found) {
                Ok(bytes) => {
                    let file =
                        (found.file_name()).map_or_else(|| name.into(), |n| n.to_string_lossy());
                    return Some(ImageFile::Embedded(EmbeddedFile::new(&file, bytes)));
                }
                Err(e) => {
                    self.note(Why::Failed, &e.to_string());
                    found
                }
            },
            Located::Found(found) => found,
            Located::Absent(absent) => {
                if self.embed {
                    self.note(Why::Missing, name);
                }
                absent
            }
            Located::Left(why) => {
                self.note(why, name);
                return None;
            }
        };
        Some(ImageFile::Path(Arc::from(path.display().to_string())))
    }

    /// What was left unresolved or unread, one warning a reason.
    pub fn warnings(&self) -> Vec<Diagnostic> {
        let left = self.left.lock().unwrap_or_else(|e| e.into_inner());
        let file = &self.file;
        [
            (&left.special, "not regular files, never read".to_owned()),
            (
                &left.held,
                format!("{file} is held in memory, so the files it names are left unresolved"),
            ),
            (&left.missing, "no such files".to_owned()),
            (&left.failed, "files that couldn't be read".to_owned()),
        ]
        .into_iter()
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, why)| Diagnostic::warning(format!("{why}: {}", listed(names))))
        .collect()
    }

    /// Where `name`, relative to `from` (else the read file's folder),
    /// leads.
    fn locate(&self, name: &str, from: Option<&Path>) -> Located {
        let Some(base) = from.or(self.folder.as_deref()) else {
            return Located::Left(Why::Held);
        };
        let path = normal(&base.join(name));
        match std::fs::metadata(&path) {
            Ok(meta) if !meta.is_file() => Located::Left(Why::Special),
            Ok(_) => match path.canonicalize() {
                Ok(real) => Located::Found(real),
                Err(_) => Located::Absent(path),
            },
            Err(_) => Located::Absent(path),
        }
    }

    /// Lists `name` as left unresolved for `why`.
    fn note(&self, why: Why, name: &str) {
        let mut left = self.left.lock().unwrap_or_else(|e| e.into_inner());
        let list = match why {
            Why::Special => &mut left.special,
            Why::Held => &mut left.held,
            Why::Missing => &mut left.missing,
            Why::Failed => &mut left.failed,
        };
        list.insert(name.to_owned());
    }
}

/// Where a name leads ([`Named::locate`]).
enum Located {
    /// A regular file, by its canonical path.
    Found(PathBuf),
    /// A path where nothing is.
    Absent(PathBuf),
    /// Nothing it may be read as, for this reason.
    Left(Why),
}

/// Why a name was left unresolved, or a file unread.
#[derive(Clone, Copy)]
enum Why {
    Special,
    Held,
    Missing,
    Failed,
}

/// `path` without `.` and with each `..` taken back, as text: no file is
/// looked at. A `..` above the root stays at the root.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                let climbs = matches!(
                    out.components().next_back(),
                    None | Some(Component::ParentDir)
                );
                match climbs && !out.has_root() {
                    true => out.push(".."),
                    false => _ = out.pop(),
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// `names`, the first few, then how many more.
fn listed(names: &BTreeSet<String>) -> String {
    let shown: Vec<&str> = names.iter().take(LISTED).map(String::as_str).collect();
    match names.len() > LISTED {
        true => format!("{} and {} more", shown.join(", "), names.len() - LISTED),
        false => shown.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Purpose, budget::ReadLimits};
    use crate::{Units, Value};
    use kinogaki_render_api::RENDER_SPACE;

    #[derive(Debug)]
    struct Plain;
    impl crate::color::ColorService for Plain {
        fn working(&self) -> &str {
            RENDER_SPACE
        }
        fn convert(&self, _: &str, _: &str, _: &mut [[f64; 3]]) -> Result<(), Error> {
            Ok(())
        }
        fn revision(&self) -> u64 {
            0
        }
    }

    /// A scratch folder holding `scene.obj` beside `maps/wood.png`, and a
    /// `secret` beside the folder.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kinogaki-named-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scene/maps")).unwrap();
        std::fs::write(dir.join("scene/scene.obj"), b"v 0 0 0").unwrap();
        std::fs::write(dir.join("scene/maps/wood.png"), b"wood").unwrap();
        std::fs::write(dir.join("secret"), b"key").unwrap();
        dir
    }

    fn named(scene: &Path, options: &[(&str, bool)], limits: ReadLimits) -> Named {
        let chosen: FormatOptions = (options.iter())
            .map(|(k, v)| ((*k).to_owned(), Value::Bool(*v)))
            .collect();
        let reading = Reading {
            budget: ReadBudget::new(limits),
            ..Reading::new(Purpose::Objects, Units::default(), &Plain)
        };
        Named::new(&Input::file(scene), &chosen, &reading)
    }

    #[test]
    fn names_reach_files_anywhere_and_regular_ones_alone() {
        let dir = scratch("reach");
        let scene = dir.join("scene/scene.obj");
        let named = named(&scene, &[], ReadLimits::default());
        let wood = dir.join("scene/maps/wood.png");
        let real = Some(wood.canonicalize().unwrap());
        assert_eq!(named.file("maps/wood.png", None), real);
        assert_eq!(named.file("./maps/../maps/wood.png", None), real);
        let maps = dir.join("scene/maps");
        assert_eq!(named.file("wood.png", Some(&maps)), real);
        let canonical = maps.canonicalize().unwrap();
        assert_eq!(
            named.file("wood.png", Some(&canonical)),
            real,
            "from a found file's folder"
        );
        assert_eq!(named.file(&wood.display().to_string(), None), real);
        // Outside the folder: read, as a parent folder or an absolute path.
        let secret = dir.join("secret").canonicalize().ok();
        assert_eq!(named.file("../secret", None), secret);
        assert_eq!(named.file("maps/../../secret", None), secret);
        assert_eq!(named.file("../../secret", Some(&maps)), secret);
        let absolute = dir.join("secret").display().to_string();
        assert_eq!(named.file(&absolute, None), secret);
        assert_eq!(named.read(secret.as_ref().unwrap()).unwrap(), b"key");
        // Never a device, a folder or what isn't there.
        assert_eq!(named.file("/dev/zero", None), None);
        assert_eq!(named.file("maps", None), None);
        assert_eq!(named.file("nothing.mtl", None), None);
        assert_eq!(named.file("//server/share/a.png", None), None);
        let warnings: Vec<String> = named.warnings().into_iter().map(|w| w.message).collect();
        assert_eq!(
            warnings,
            [
                "not regular files, never read: /dev/zero, maps",
                "no such files: //server/share/a.png, nothing.mtl"
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn of_several_spellings_the_first_found_is_taken_and_only_a_miss_said() {
        let dir = scratch("first");
        let scene = dir.join("scene/scene.obj");
        let named = named(&scene, &[], ReadLimits::default());
        let found = named.first(["maps wood.png", "maps", "maps/wood.png"], None);
        assert_eq!(found, dir.join("scene/maps/wood.png").canonicalize().ok());
        assert!(named.warnings().is_empty(), "{:?}", named.warnings());
        assert_eq!(named.first(["a b.mtl", "a", "b.mtl"], None), None);
        assert_eq!(named.first(["", ""], None), None);
        let said: Vec<String> = named.warnings().into_iter().map(|w| w.message).collect();
        assert_eq!(said, ["no such files: a b.mtl"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_link_is_followed_to_a_regular_file_alone() {
        let dir = scratch("link");
        let scene = dir.join("scene/scene.obj");
        let link = dir.join("scene/maps/key.png");
        std::os::unix::fs::symlink(dir.join("secret"), &link).unwrap();
        std::os::unix::fs::symlink("/dev/zero", dir.join("scene/zero.png")).unwrap();
        let named = named(&scene, &[(EMBED, true)], ReadLimits::default());
        let secret = dir.join("secret").canonicalize().ok();
        assert_eq!(named.file("maps/key.png", None), secret, "by its identity");
        let Some(ImageFile::Embedded(key)) = named.image("maps/key.png", None) else {
            panic!("embedded");
        };
        assert_eq!((key.name(), key.bytes()), ("secret", &b"key"[..]));
        assert_eq!(
            named.image("zero.png", None),
            None,
            "a device behind a link"
        );
        let messages: Vec<String> = named.warnings().into_iter().map(|w| w.message).collect();
        assert_eq!(messages, ["not regular files, never read: zero.png"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pictures_embed_within_the_budget_and_stay_paths_otherwise() {
        let dir = scratch("image");
        let scene = dir.join("scene/scene.obj");
        let embeds = named(&scene, &[(EMBED, true)], ReadLimits::default());
        let Some(ImageFile::Embedded(wood)) = embeds.image("maps/wood.png", None) else {
            panic!("embedded");
        };
        assert_eq!((wood.name(), wood.bytes()), ("wood.png", &b"wood"[..]));
        let gone = embeds.image("maps/gone.png", None);
        assert!(matches!(gone, Some(ImageFile::Path(p)) if p.ends_with("scene/maps/gone.png")));
        assert!(
            embeds.warnings()[0]
                .message
                .ends_with("no such files: maps/gone.png")
        );
        // Past the budget's bytes, a picture stays a path, said.
        let small = ReadLimits {
            bytes: 3,
            ..ReadLimits::default()
        };
        let tight = named(&scene, &[(EMBED, true)], small);
        assert!(matches!(
            tight.image("maps/wood.png", None),
            Some(ImageFile::Path(_))
        ));
        assert!(
            tight.warnings()[0]
                .message
                .contains("over the read's budget of 3 B")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_held_in_memory_names_no_file_on_disk() {
        let reading = Reading::new(Purpose::Value, Units::default(), &Plain);
        let named = Named::new(
            &Input::memory("a.gltf", b"{}"),
            &FormatOptions::new(),
            &reading,
        );
        assert_eq!(named.folder(), None);
        assert_eq!(named.file("a.bin", None), None);
        // Its pictures' names are kept as written, for the project to find.
        for (name, kept) in [
            ("maps/./wood.png", "maps/wood.png"),
            ("../wood.png", "../wood.png"),
            ("/maps/wood.png", "/maps/wood.png"),
        ] {
            let image = named.image(name, None);
            assert!(
                matches!(image, Some(ImageFile::Path(p)) if &*p == kept),
                "{name}"
            );
        }
        let warning = &named.warnings()[0].message;
        assert!(warning.starts_with("a.gltf is held in memory"), "{warning}");
        assert!(warning.ends_with(": a.bin"), "{warning}");
    }

    #[test]
    fn names_are_normalized_as_text_and_long_lists_counted() {
        assert_eq!(normal(Path::new("/a/./b/../c")), Path::new("/a/c"));
        assert_eq!(normal(Path::new("/../a")), Path::new("/a"));
        assert_eq!(normal(Path::new("a/../../b")), Path::new("../b"));
        let names: BTreeSet<String> = (0..10).map(|k| format!("f{k}")).collect();
        assert_eq!(listed(&names), "f0, f1, f2, f3, f4, f5, f6, f7 and 2 more");
    }
}

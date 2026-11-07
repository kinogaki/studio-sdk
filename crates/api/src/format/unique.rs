//! [`Unique`]: names a read or a write gives out once each.
use std::collections::{HashMap, HashSet};

/// Names given out once each, as a format makes them (the keys of a file's
/// materials, the prims a layer writes, the image files a document names):
/// a name taken again gets a copy number, `name_1`, `name_2`… (a file name
/// before its extension: `wood_1.png`), each base name counting its own
/// copies from the last one given, so naming n alike costs n steps, never
/// n² probes.
///
/// ```
/// use kinogaki_studio_api::format::Unique;
///
/// let mut names = Unique::names();
/// assert_eq!(names.take("Red"), "Red");
/// assert_eq!(names.take("Red"), "Red_1");
/// assert_eq!(names.take("Red_1"), "Red_1_1");
/// let mut files = Unique::files();
/// assert_eq!(files.take("wood.png"), "wood.png");
/// assert_eq!(files.take("Wood.PNG"), "Wood_1.PNG");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Unique {
    /// What was given out, as compared ([`key`](Self::key)).
    taken: HashSet<String>,
    /// The next copy number to try, by base name as compared.
    next: HashMap<String, usize>,
    /// Whether these are file names: copies numbered before the extension,
    /// and names compared ignoring case.
    files: bool,
}

impl Unique {
    /// Names compared as they are spelt (a prim's, a material's key).
    pub fn names() -> Unique {
        Unique::default()
    }

    /// File names: a copy numbered before its extension, and two names
    /// differing in case alone the same name, as the file systems that
    /// ignore case take them.
    pub fn files() -> Unique {
        Unique {
            files: true,
            ..Unique::default()
        }
    }

    /// `name`, or the first of its copies not given out yet.
    pub fn take(&mut self, name: &str) -> String {
        if self.taken.insert(self.key(name)) {
            return name.to_owned();
        }
        let (stem, extension) = match self.files {
            true => match name.rfind('.') {
                Some(dot) if dot > 0 => name.split_at(dot),
                _ => (name, ""),
            },
            false => (name, ""),
        };
        let next = self.next.entry(self.key(name)).or_insert(1);
        loop {
            let copy = format!("{stem}_{next}{extension}");
            *next += 1;
            let key = match self.files {
                true => copy.to_lowercase(),
                false => copy.clone(),
            };
            if self.taken.insert(key) {
                return copy;
            }
        }
    }

    /// Whether `name` was given out.
    pub fn taken(&self, name: &str) -> bool {
        self.taken.contains(&self.key(name))
    }

    /// `name` as names are compared.
    fn key(&self, name: &str) -> String {
        match self.files {
            true => name.to_lowercase(),
            false => name.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_are_numbered_per_base_name() {
        let mut names = Unique::names();
        let taken: Vec<String> = ["a", "a", "b", "a", "a_1", "b"]
            .iter()
            .map(|n| names.take(n))
            .collect();
        assert_eq!(taken, ["a", "a_1", "b", "a_2", "a_1_1", "b_1"]);
        assert!(names.taken("a_2") && !names.taken("a_3"));
        // Case counts for names.
        assert_eq!(names.take("A"), "A");
    }

    #[test]
    fn file_names_number_before_their_extension_and_ignore_case() {
        let mut files = Unique::files();
        assert_eq!(files.take("wood.png"), "wood.png");
        assert_eq!(files.take("WOOD.png"), "WOOD_1.png");
        assert_eq!(files.take("wood_1.png"), "wood_1_1.png");
        assert_eq!(files.take("wood.png"), "wood_2.png");
        assert_eq!(files.take(".hidden"), ".hidden");
        assert_eq!(files.take(".hidden"), ".hidden_1");
        assert_eq!(files.take("plain"), "plain");
        assert_eq!(files.take("plain"), "plain_1");
    }

    #[test]
    fn many_alike_take_linear_time() {
        let mut names = Unique::names();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            names.take("Leaf");
        }
        assert!(names.taken("Leaf_99999"));
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }
}

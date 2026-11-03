//! `Elements`: the rules, and set algebra against a `BTreeSet` model over
//! seeded random sets in every stored form.
use kinogaki_geometry::{Elements, ErrorKind};
use std::collections::BTreeSet;

#[test]
fn the_rules_are_checked() {
    let g = Elements::from_indices(130, [129, 0, 64, 0]).unwrap();
    assert_eq!((g.count(), g.len(), g.words().len()), (130, 3, 3));
    assert_eq!(g.iter().collect::<Vec<_>>(), [0, 64, 129]);
    assert!(g.contains(129) && !g.contains(130) && !g.contains(1000));
    assert_eq!(Elements::from_bits(130, g.words()).unwrap(), g);
    let few = Elements::from_bits(130, vec![0, 0].into()).unwrap_err();
    assert_eq!(few.message(), "a set of 130 elements has 2 words, not 3");
    let past = Elements::from_bits(130, vec![0, 0, 1 << 2].into()).unwrap_err();
    assert_eq!(
        past.message(),
        "a set of 130 elements holds bits past its last element"
    );
    let full = Elements::from_bits(128, vec![u64::MAX, u64::MAX].into()).unwrap();
    assert!(full.is_all() && full == Elements::all(128));
    let out = Elements::from_indices(4, [4]).unwrap_err();
    assert_eq!(
        (out.kind, out.diagnostic.element, out.message()),
        (
            ErrorKind::Invalid,
            Some(4),
            "element 4 is past the last of 4"
        )
    );
    assert_eq!(Elements::none(0).words().len(), 0);
    assert!(!Elements::all(0).is_all() && Elements::all(0) == Elements::none(0));
    let refused = Elements::none(3).union(&Elements::none(4)).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Invalid);
}

/// A seeded generator (SplitMix64): the same sets every run.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A set of `count` elements with about `density` of them members.
    fn set(&mut self, count: usize, density: f64) -> BTreeSet<u32> {
        (0..count as u32)
            .filter(|_| (self.next() % 1_000_000) as f64 / 1e6 < density)
            .collect()
    }
}

#[test]
fn set_algebra_matches_a_btree_model_in_every_form() {
    let mut rng = Seeded(7);
    // Counts on and around word boundaries; densities that keep the list
    // form (sparse), the bit form (dense) and every element (all).
    for count in [0usize, 1, 63, 64, 65, 130, 1_000, 4_097] {
        for _ in 0..40 {
            let densities = [0.0, 0.01, 0.5, 0.97, 1.0];
            let pick = |rng: &mut Seeded| densities[(rng.next() % 5) as usize];
            let (da, db) = (pick(&mut rng), pick(&mut rng));
            let (ma, mb) = (rng.set(count, da), rng.set(count, db));
            let a = Elements::from_indices(count, ma.iter().copied()).unwrap();
            let b = Elements::from_bits(
                count,
                Elements::from_indices(count, mb.iter().copied())
                    .unwrap()
                    .words(),
            )
            .unwrap();
            let model = |e: &Elements| e.iter().collect::<BTreeSet<_>>();
            assert_eq!((model(&a), a.len()), (ma.clone(), ma.len()));
            assert_eq!((model(&b), b.count()), (mb.clone(), count));
            let check = |got: Elements, want: BTreeSet<u32>| {
                assert_eq!(got.count(), count);
                assert_eq!(model(&got), want);
                assert_eq!(got.len(), want.len());
                assert_eq!(got.is_all(), count > 0 && want.len() == count);
                // Every form of the same members is equal.
                let listed = Elements::from_indices(count, want.iter().copied()).unwrap();
                assert_eq!(got, listed);
                assert_eq!(Elements::from_bits(count, got.words()).unwrap(), listed);
                assert_eq!(
                    got.indices().to_vec(),
                    want.iter().copied().collect::<Vec<_>>()
                );
                for probe in [0u32, 1, 63, 64, count as u32, count as u32 + 5] {
                    assert_eq!(got.contains(probe), want.contains(&probe));
                }
            };
            check(a.union(&b).unwrap(), &ma | &mb);
            check(a.intersection(&b).unwrap(), &ma & &mb);
            check(a.difference(&b).unwrap(), &ma - &mb);
            check(a.symmetric_difference(&b).unwrap(), &ma ^ &mb);
            check(a.clone(), ma.clone());
        }
    }
}

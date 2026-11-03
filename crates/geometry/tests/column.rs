//! Columns: adopted without a copy, shared by clones, sliced without a
//! copy, and their cache slots built once and dropped with them.
use kinogaki_geometry::{Column, ColumnWriter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

#[test]
fn a_vec_is_adopted_without_reallocating() {
    let mut values = Vec::with_capacity(1000);
    values.extend((0..1000).map(|i| i as f64));
    let address = values.as_ptr();
    let column = Column::new(values);
    assert_eq!(column.as_slice().as_ptr(), address);
    assert_eq!(Column::from(vec![1u32, 2]).len(), 2);
}

#[test]
fn clones_share_the_column_and_its_key() {
    let a = Column::new(vec![1u32, 2, 3]);
    let b = a.clone();
    assert!(a.ptr_eq(&b) && a.key() == b.key());
    let c = Column::new(vec![1u32, 2, 3]);
    assert!(!a.ptr_eq(&c) && a.key() != c.key());
    assert_eq!(a, c, "equal values compare equal");
}

#[test]
fn a_slice_shares_the_buffer_but_not_the_slots() {
    let a = Column::new((0..10u32).collect::<Vec<_>>());
    let s = a.slice(2..5);
    assert_eq!(s.as_slice(), &[2, 3, 4]);
    assert!(!s.ptr_eq(&a));
    assert_eq!(s.as_slice().as_ptr(), a.as_slice()[2..].as_ptr());
    let sum = |c: &Column<u32>| *c.derived(|v| v.iter().sum::<u32>());
    assert_eq!((sum(&a), sum(&s)), (45, 9));
    assert_eq!(s.slice(1..3).as_slice(), &[3, 4]);
    assert!(Column::new(vec![0u8]).slice(1..1).is_empty());
}

#[test]
#[should_panic(expected = "range 3..12 of a column of 10")]
fn a_slice_past_the_end_panics() {
    let _ = Column::new(vec![0u8; 10]).slice(3..12);
}

#[test]
fn a_cache_is_built_once_for_every_clone() {
    let builds = AtomicUsize::new(0);
    let a = Column::new(vec![3.0, 1.0, 2.0]);
    let max = |c: &Column<f64>| {
        c.derived(|v| {
            builds.fetch_add(1, Ordering::Relaxed);
            v.iter().cloned().fold(f64::MIN, f64::max)
        })
    };
    let first = max(&a);
    let again = max(&a.clone());
    assert!(Arc::ptr_eq(&first, &again) && *first == 3.0);
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    // Two cache types over one column are two slots.
    let len = a.derived(|v| v.len());
    assert_eq!(*len, 3);
}

#[test]
fn a_dropped_column_drops_its_caches() {
    struct Cache;
    let column = Column::new(vec![0u8; 4]);
    let weak: Weak<Cache> = Arc::downgrade(&column.derived(|_| Cache));
    assert!(weak.upgrade().is_some());
    drop(column);
    assert!(weak.upgrade().is_none());
}

#[test]
fn caches_built_on_many_threads_keep_one_value() {
    let column = Column::new(vec![1u64; 1000]);
    let kept: Vec<Arc<u64>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| s.spawn(|| column.derived(|v| v.iter().sum::<u64>())))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(kept.iter().all(|k| Arc::ptr_eq(k, &kept[0])));
}

#[test]
fn threads_asking_at_once_build_a_cache_once() {
    struct Sum(f64);
    let column: Column<f64> = (0..1000).map(f64::from).collect();
    let (builds, threads) = (AtomicUsize::new(0), 8);
    let start = std::sync::Barrier::new(threads);
    let sums: Vec<Arc<Sum>> = std::thread::scope(|scope| {
        let asks: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    column.derived(|values| {
                        builds.fetch_add(1, Ordering::SeqCst);
                        // A slow build: the others ask while it runs.
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        Sum(values.iter().sum())
                    })
                })
            })
            .collect();
        asks.into_iter().map(|a| a.join().unwrap()).collect()
    });
    assert_eq!(
        builds.load(Ordering::SeqCst),
        1,
        "one build for every thread"
    );
    assert!(
        sums.iter()
            .all(|s| Arc::ptr_eq(s, &sums[0]) && s.0 == 499_500.0)
    );
}

/// A column of `n` values and the same values in a `Vec`.
fn counted(n: u32) -> (Column<u32>, Vec<u32>) {
    let values: Vec<u32> = (0..n).collect();
    (Column::new(values.clone()), values)
}

/// A column reads the same however it is held.
fn reads_as(column: &Column<u32>, values: &[u32]) {
    assert_eq!(column.len(), values.len());
    assert_eq!(column.as_slice(), values);
    assert_eq!(column.to_vec(), values);
    assert!(column.iter().eq(values.iter()));
    assert!(column.iter().rev().eq(values.iter().rev()));
    assert_eq!(column.iter().len(), values.len());
    assert_eq!(column.runs().flatten().count(), values.len());
    for at in (0..values.len()).step_by(values.len() / 97 + 1) {
        assert_eq!(column[at], values[at]);
    }
    assert_eq!(column.first(), values.first());
    assert_eq!(column.last(), values.last());
    assert_eq!(column.get(values.len()), None);
    let found: Vec<u32> = column.iter().copied().collect();
    assert_eq!(found, values);
}

#[test]
fn an_edit_copies_only_the_cells_it_writes() {
    let (column, mut values) = counted(1 << 20);
    let mut edit = column.edit();
    edit[70_000] = 7;
    values[70_000] = 7;
    let edited = edit.finish();
    reads_as(&edited, &values);
    assert_eq!(column[70_000], 70_000, "the source stays");
    let differing: Vec<_> = edited.unshared(&column).collect();
    assert_eq!(differing.len(), 1, "one cell");
    assert!(differing[0].contains(&70_000) && differing[0].len() <= 1 << 14);
    assert!(edited != column);
    let mut back = edited.edit();
    back[70_000] = 70_000;
    assert_eq!(back.finish(), column, "equal by values");
}

#[test]
fn a_run_kept_after_a_removal_shares_its_source() {
    let (column, mut values) = counted(1 << 20);
    let mut edit = ColumnWriter::new();
    edit.keep(&column, 0..500);
    edit.keep(&column, 501..1 << 20);
    values.remove(500);
    let kept = edit.finish();
    reads_as(&kept, &values);
    // Past the first cell it is one run of the source's buffer, a value on.
    let far = 1 << 19;
    let run = kept.run(far..far + 40_000).expect("one buffer's run");
    assert_eq!(run.as_ptr(), column.run(far + 1..far + 2).unwrap().as_ptr());
    // A flat view of a column in pieces is one copy, kept.
    assert_eq!(kept.as_slice().as_ptr(), kept.as_slice().as_ptr());
}

#[test]
fn a_range_across_a_written_cell_is_not_one_run() {
    let (column, values) = counted(1 << 16);
    let mut edit = column.edit();
    edit[1 << 14] = values[1 << 14];
    let edited = edit.finish();
    let boundary = 1 << 14;
    assert!(edited.run(boundary - 2..boundary + 2).is_none());
    assert!(edited.run(boundary..boundary + 2).is_some());
    assert_eq!(edited, column);
    assert!(column.run(boundary - 2..boundary + 2).is_some());
}

#[test]
fn appending_reads_on_and_truncating_cuts() {
    let (column, mut values) = counted(40_000);
    let mut edit = column.edit();
    for v in 0..50_000 {
        edit.push(v);
        values.push(v);
    }
    edit.extend_from_slice(&[1, 2, 3]);
    values.extend_from_slice(&[1, 2, 3]);
    let long = edit.finish();
    reads_as(&long, &values);
    let mut cut = long.edit();
    cut.truncate(30_001);
    values.truncate(30_001);
    cut.push(9);
    values.push(9);
    reads_as(&cut.finish(), &values);
    reads_as(&long.slice(10..35_000), &long.to_vec()[10..35_000]);
    let made = Column::from_ranges(50_000, |range, cell| cell.extend(range.map(|i| i as u32)));
    reads_as(&made, &(0..50_000).collect::<Vec<u32>>());
}

/// Any mix of kept runs, writes, appends and cuts reads as a `Vec` doing the
/// same, on every kind of source the mix leaves.
#[test]
fn any_mix_of_edits_reads_as_a_vec() {
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = |below: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % below.max(1) as u64) as usize
    };
    let (mut column, mut values) = counted(100_000);
    for _ in 0..200 {
        match next(5) {
            0 => {
                let mut edit = column.edit();
                for _ in 0..next(40) {
                    let (at, v) = (next(values.len()), next(1000) as u32);
                    edit[at] = v;
                    values[at] = v;
                }
                column = edit.finish();
            }
            1 => {
                let (from, to) = (next(values.len()), next(values.len()));
                let (from, to) = (from.min(to), from.max(to));
                let mut edit = ColumnWriter::new();
                edit.keep(&column, 0..from);
                edit.keep(&column, to..values.len());
                values.drain(from..to);
                column = edit.finish();
            }
            2 => {
                let mut edit = column.edit();
                let count = next(30_000);
                (0..count).for_each(|_| edit.push(5));
                values.resize(values.len() + count, 5);
                column = edit.finish();
            }
            3 => {
                let at = next(values.len());
                let mut edit = ColumnWriter::new();
                edit.keep(&column, 0..at);
                edit.extend_from_slice(&[8; 300]);
                edit.keep(&column, at..values.len());
                values.splice(at..at, [8; 300]);
                column = edit.finish();
            }
            _ => {
                let len = next(values.len());
                if len > 10_000 {
                    let mut edit = column.edit();
                    edit.truncate(len);
                    values.truncate(len);
                    column = edit.finish();
                }
            }
        }
        reads_as(&column, &values);
    }
}

//! Columns: adopted without a copy, shared by clones, sliced without a
//! copy, and their cache slots built once and dropped with them.
use kinogaki_geometry::Column;
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
    assert_eq!(&*s, &[2, 3, 4]);
    assert!(s.shares_buffer(&a) && !s.ptr_eq(&a));
    assert_eq!(s.as_ptr(), a[2..].as_ptr());
    let sum = |c: &Column<u32>| *c.derived(|v| v.iter().sum::<u32>());
    assert_eq!((sum(&a), sum(&s)), (45, 9));
    assert_eq!(&*s.slice(1..3), &[3, 4]);
    assert!(Column::new(vec![0u8]).slice(1..1).is_empty());
}

#[test]
#[should_panic(expected = "slice 3..12 of a column of 10")]
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

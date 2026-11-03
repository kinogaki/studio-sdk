//! [`Layout`]: where the elements of one domain of an edited value come
//! from, the one plan every column and group of that domain follows.
use std::ops::Range;

/// A blend of elements of the base value: `(element, weight)`.
pub(super) type Source = (u32, f64);

/// Sources kept end to end, so a large edit allocates no list per element.
#[derive(Clone, Debug, Default)]
pub(super) struct Blends(Vec<Source>);

/// One element's sources in [`Blends`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Span {
    start: u32,
    len: u32,
}

impl Blends {
    /// `from` kept, each element numbered past `base` (one the same edit
    /// added) standing for its own sources in `added`: so sources always
    /// name the base value's elements.
    pub(super) fn add(&mut self, from: &[Source], base: u32, added: &[Span]) -> Option<Span> {
        let start = self.0.len();
        for &(element, weight) in from {
            if !weight.is_finite() || weight < 0.0 {
                return None;
            }
            match element.checked_sub(base) {
                None => self.0.push((element, weight)),
                Some(k) => {
                    let of = *added.get(k as usize)?;
                    for i in of.range() {
                        let (source, part) = self.0[i];
                        self.0.push((source, part * weight));
                    }
                }
            }
        }
        Some(Span {
            start: start as u32,
            len: (self.0.len() - start) as u32,
        })
    }

    /// `a` and `b` mixed, `a` by `1 - t` and `b` by `t`.
    pub(super) fn mixed(&mut self, a: Span, b: Span, t: f64) -> Span {
        let start = self.0.len();
        for (span, by) in [(a, 1.0 - t), (b, t)] {
            for i in span.range() {
                let (source, part) = self.0[i];
                self.0.push((source, part * by));
            }
        }
        Span {
            start: start as u32,
            len: (self.0.len() - start) as u32,
        }
    }

    /// `span`'s sources.
    pub(super) fn of(&self, span: Span) -> &[Source] {
        &self.0[span.range()]
    }
}

impl Span {
    fn range(self) -> Range<usize> {
        self.start as usize..(self.start + self.len) as usize
    }
}

/// The source of `sources` with the largest weight, the first among equals:
/// what integers, text, arrays and groups take.
pub(super) fn strongest(sources: &[Source]) -> Option<u32> {
    let mut best: Option<Source> = None;
    for &(element, weight) in sources {
        if best.is_none_or(|(_, w)| weight > w) {
            best = Some((element, weight));
        }
    }
    best.map(|(element, _)| element)
}

/// A run of a [`Layout`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Run {
    /// These elements of the base value, as they are.
    Kept(Range<u32>),
    /// One new element.
    New(Span),
}

/// The elements of one domain of the edited value, in order: runs of the
/// base's elements and new elements with their sources. Cost per column is
/// a copy per run and a blend per new element, whatever the column's size.
#[derive(Clone, Debug, Default)]
pub(super) struct Layout {
    runs: Vec<Run>,
    len: u32,
}

impl Layout {
    /// These base elements next, kept.
    pub(super) fn keep(&mut self, range: Range<u32>) {
        if range.is_empty() {
            return;
        }
        self.len += range.len() as u32;
        if let Some(Run::Kept(last)) = self.runs.last_mut()
            && last.end == range.start
        {
            last.end = range.end;
            return;
        }
        self.runs.push(Run::Kept(range));
    }

    /// One new element next.
    pub(super) fn add(&mut self, from: Span) {
        self.len += 1;
        self.runs.push(Run::New(from));
    }

    /// How many elements.
    pub(super) fn len(&self) -> usize {
        self.len as usize
    }

    /// The runs, in order.
    pub(super) fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// How many new elements.
    pub(super) fn added(&self) -> usize {
        (self.runs.iter())
            .filter(|run| matches!(run, Run::New(_)))
            .count()
    }

    /// Whether it is exactly the `base` elements of the base value: its
    /// columns are shared.
    pub(super) fn is_whole(&self, base: usize) -> bool {
        match &self.runs[..] {
            [] => base == 0,
            [Run::Kept(range)] => range.start == 0 && range.end as usize == base,
            _ => false,
        }
    }
}

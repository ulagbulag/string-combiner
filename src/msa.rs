use std::{
    borrow::{Borrow, Cow},
    cmp::min,
    fmt,
    marker::PhantomData,
};

pub use bio::alignment::pairwise::Scoring;
use bio::alignment::{pairwise::Aligner, Alignment, AlignmentMode, AlignmentOperation};
#[cfg(feature = "rayon")]
use rayon::current_num_threads;
#[cfg(feature = "rayon")]
use rayon_cond::CondIterator;
use serde::{Deserialize, Serialize};

pub trait MultipleSequenceAlignment<I, T, V>
where
    V: Clone + AlignmentVisitor<T>,
{
    type Output;

    fn reduce_all<F, Iter>(
        &self,
        scoring: Scoring<F, T>,
        visitor: V,
        inputs: Iter,
    ) -> Option<Self::Output>
    where
        F: Sync + Clone + Fn(&T, &T) -> i32,
        I: AsRef<[T]>,
        Iter: IntoIterator,
        Iter::Item: AsRef<I>,
        T: Clone + Eq;
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinearMultipleSequenceAligner;

impl<I, T, V> MultipleSequenceAlignment<I, T, V> for LinearMultipleSequenceAligner
where
    [T]: ToOwned<Owned = V::Output>,
    V: Clone + AlignmentVisitor<T>,
    V::Output: Borrow<[T]>,
{
    type Output = V::Output;

    fn reduce_all<F, Iter>(
        &self,
        scoring: Scoring<F, T>,
        visitor: V,
        inputs: Iter,
    ) -> Option<Self::Output>
    where
        F: Clone + Fn(&T, &T) -> i32,
        I: AsRef<[T]>,
        Iter: IntoIterator,
        Iter::Item: AsRef<I>,
        T: Clone + Eq,
    {
        let mut inputs = inputs.into_iter();
        let first = inputs.next()?;
        let mut x = Cow::<[T]>::Borrowed(first.as_ref().as_ref());

        for y in inputs {
            let y = y.as_ref().as_ref();
            let mut aligner = Aligner::with_capacity_and_scoring(x.len(), y.len(), scoring.clone());
            let alignment = aligner.semiglobal(&x, y);
            x = Cow::Owned(alignment.reduce(visitor.clone(), &x, y))
        }
        Some(x.into_owned())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GreedyMultipleSequenceAligner<Fm, Fs, I, S>
where
    Fm: Sync + Fn(&I, &I) -> SequenceMatch<I>,
    Fs: Sync + Fn(&I) -> Option<S>,
    S: Ord,
{
    pub _item: PhantomData<(I, S)>,
    pub match_fn: Fm,
    pub score_fn: Fs,
}

impl<Fm, Fs, I, S> GreedyMultipleSequenceAligner<Fm, Fs, I, S>
where
    Fm: Sync + Fn(&I, &I) -> SequenceMatch<I>,
    Fs: Sync + Fn(&I) -> Option<S>,
    S: Ord,
{
    #[inline]
    pub const fn new(match_fn: Fm, score_fn: Fs) -> Self {
        Self {
            _item: PhantomData,
            match_fn,
            score_fn,
        }
    }
}

impl<Fm, Fs, I, S, T, V> MultipleSequenceAlignment<I, AlignedToken<T>, V>
    for GreedyMultipleSequenceAligner<Fm, Fs, I, S>
where
    Fm: Sync + Fn(&I, &I) -> SequenceMatch<I>,
    Fs: Sync + Fn(&I) -> Option<S>,
    I: Send + Sync + Clone + GreedyMultipleSequenceAlignerItem<T>,
    S: Send + Sync + Copy + Ord,
    T: Send + Sync,
    V: Send + Sync + Clone + AlignmentVisitor<AlignedToken<T>>,
    V::Output: Into<AlignedSequence<T>>,
{
    type Output = I;

    fn reduce_all<Fscore, Iter>(
        &self,
        scoring: Scoring<Fscore, AlignedToken<T>>,
        visitor: V,
        inputs: Iter,
    ) -> Option<Self::Output>
    where
        AlignedToken<T>: Clone + Eq,
        Fscore: Sync + Clone + Fn(&AlignedToken<T>, &AlignedToken<T>) -> i32,
        I: AsRef<[AlignedToken<T>]>,
        Iter: IntoIterator,
        Iter::Item: AsRef<I>,
    {
        struct State<I, S, T> {
            _item: PhantomData<T>,
            score: Option<S>,
            seq: I,
        }

        impl<I, S, T> AsRef<[T]> for State<I, S, T>
        where
            I: AsRef<[T]>,
        {
            #[inline]
            fn as_ref(&self) -> &[T] {
                self.seq.as_ref()
            }
        }

        impl<I, S, T> State<I, S, T> {
            #[inline]
            fn new(score: S, seq: I) -> Self {
                Self {
                    _item: PhantomData,
                    score: Some(score),
                    seq,
                }
            }

            #[inline]
            fn genesis(score: Option<S>, seq: &I) -> Self
            where
                I: Clone,
            {
                Self {
                    _item: PhantomData,
                    score,
                    seq: seq.clone(),
                }
            }
        }

        let calculate_seq = |x: &State<I, S, T>, y: &I| -> Option<I> {
            match (self.match_fn)(&x.seq, y) {
                SequenceMatch::Matched => {
                    let mut aligner = Aligner::with_capacity_and_scoring(
                        x.seq.as_ref().len(),
                        y.as_ref().len(),
                        scoring.clone(),
                    );
                    let alignment = aligner.local(x.seq.as_ref(), y.as_ref());
                    let seq = alignment.reduce(visitor.clone(), x.seq.as_ref(), y.as_ref());
                    Some(I::build(&x.seq, y, seq.into()))
                }
                SequenceMatch::Unmatched => None,
                SequenceMatch::Custom(seq) => Some(seq),
            }
        };
        #[cfg(feature = "rayon")]
        let parallel = |len| len >= 5 * current_num_threads();

        // Fill the table and find the maximum score
        let inputs = inputs.into_iter();
        let mut table: Vec<State<I, S, T>> = match inputs.size_hint().1 {
            Some(len) => Vec::with_capacity(len),
            None => Default::default(),
        };
        for y in inputs {
            let y = y.as_ref();
            let mut best_state: State<I, S, T> = State::genesis((self.score_fn)(y), y);

            #[cfg(feature = "rayon")]
            let iter = CondIterator::new(&table, parallel(table.len()));

            #[cfg(not(feature = "rayon"))]
            let iter = table.iter();

            if let Some((seq, score)) = iter
                .filter_map(|x| calculate_seq(x, y))
                .filter_map(|seq| {
                    let score = (self.score_fn)(&seq)?;
                    Some((seq, score))
                })
                .max_by_key(|(_, score)| *score)
            {
                if best_state
                    .score
                    .map(|best_score| score > best_score)
                    .unwrap_or_default()
                {
                    best_state = State::new(score, seq)
                }
            }
            table.push(best_state)
        }

        // Pick up the state that was finally selected
        // If the scores are the same, we choose the latter.
        table
            .into_iter()
            .max_by_key(|state| state.score)
            .map(|state| state.seq)
    }
}

pub trait GreedyMultipleSequenceAlignerItem<T> {
    fn build(x: &Self, y: &Self, seq: AlignedSequence<T>) -> Self
    where
        Self: Sized;
}

pub trait SequenceAlignment<T> {
    fn reduce<V>(&self, visitor: V, x: &[T], y: &[T]) -> V::Output
    where
        V: AlignmentVisitor<T>;
}

impl<T> SequenceAlignment<T> for Alignment {
    fn reduce<V>(&self, mut visitor: V, x: &[T], y: &[T]) -> V::Output
    where
        V: AlignmentVisitor<T>,
    {
        if !self.operations.is_empty() {
            let mut x_i: usize;
            let mut y_i: usize;

            // If the alignment mode is one of the standard ones, the prefix clipping is
            // implicit so we need to process it here
            match self.mode {
                AlignmentMode::Custom => {
                    x_i = 0;
                    y_i = 0;
                }
                _ => {
                    x_i = self.xstart;
                    y_i = self.ystart;
                    visitor.visit_prefix_x(&x[..self.xstart.min(x.len())]);
                    visitor.visit_prefix_y(&y[..self.ystart.min(x.len())]);
                }
            }

            // Process the alignment.
            for i in 0..self.operations.len() {
                match self.operations[i] {
                    AlignmentOperation::Match => {
                        visitor.visit_match(&x[x_i], &y[y_i]);
                        x_i += 1;
                        y_i += 1;
                    }
                    AlignmentOperation::Subst => {
                        visitor.visit_subst(&x[x_i], &y[y_i]);
                        x_i += 1;
                        y_i += 1;
                    }
                    AlignmentOperation::Del => {
                        visitor.visit_del(&y[y_i]);
                        y_i += 1;
                    }
                    AlignmentOperation::Ins => {
                        visitor.visit_ins(&x[x_i]);
                        x_i += 1;
                    }
                    AlignmentOperation::Xclip(len) => {
                        let len = len.min(y.len());
                        visitor.visit_xclip(&x[..len]);
                        x_i += len;
                    }
                    AlignmentOperation::Yclip(len) => {
                        let len = len.min(y.len());
                        visitor.visit_yclip(&y[..len]);
                        y_i += len;
                    }
                }
            }

            // If the alignment mode is one of the standard ones, the suffix clipping is
            // implicit so we need to process it here
            match self.mode {
                AlignmentMode::Custom => {}
                _ => {
                    visitor.visit_suffix_x(&x[x_i.min(x.len())..self.xlen.min(x.len())]);
                    visitor.visit_suffix_y(&y[y_i.min(y.len())..self.ylen.min(y.len())]);
                }
            }
        }

        visitor.finish()
    }
}

pub trait AlignmentVisitor<T> {
    type Output;

    fn visit_prefix_x(&mut self, x: &[T]);

    fn visit_prefix_y(&mut self, y: &[T]);

    fn visit_match(&mut self, x: &T, y: &T);

    fn visit_subst(&mut self, x: &T, y: &T);

    fn visit_del(&mut self, y: &T);

    fn visit_ins(&mut self, x: &T);

    #[inline]
    fn visit_xclip(&mut self, x: &[T]) {
        self.visit_prefix_x(x)
    }

    #[inline]
    fn visit_yclip(&mut self, y: &[T]) {
        self.visit_prefix_y(y)
    }

    #[inline]
    fn visit_suffix_x(&mut self, x: &[T]) {
        self.visit_prefix_x(x)
    }

    #[inline]
    fn visit_suffix_y(&mut self, y: &[T]) {
        self.visit_prefix_y(y)
    }

    fn finish(self) -> Self::Output;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlignmentPrettyVisitor {
    ncol: usize,
    x_pretty: Vec<u8>,
    y_pretty: Vec<u8>,
    inb_pretty: Vec<u8>,
}

impl AlignmentPrettyVisitor {
    pub fn new(ncol: usize) -> Self {
        Self {
            ncol,
            x_pretty: Default::default(),
            y_pretty: Default::default(),
            inb_pretty: Default::default(),
        }
    }
}

impl AlignmentVisitor<u8> for AlignmentPrettyVisitor {
    type Output = String;

    fn visit_prefix_x(&mut self, x: &[u8]) {
        for k in x {
            self.x_pretty.push(*k);
            self.inb_pretty.push(b' ');
            self.y_pretty.push(b' ')
        }
    }

    fn visit_prefix_y(&mut self, y: &[u8]) {
        for k in y {
            self.x_pretty.push(b' ');
            self.inb_pretty.push(b' ');
            self.y_pretty.push(*k)
        }
    }

    fn visit_match(&mut self, x: &u8, y: &u8) {
        self.x_pretty.push(*x);
        self.inb_pretty.push(b'|');
        self.y_pretty.push(*y);
    }

    fn visit_subst(&mut self, x: &u8, y: &u8) {
        self.x_pretty.push(*x);
        self.inb_pretty.push(b'\\');
        self.y_pretty.push(*y);
    }

    fn visit_del(&mut self, y: &u8) {
        self.x_pretty.push(b'-');
        self.inb_pretty.push(b'x');
        self.y_pretty.push(*y);
    }

    fn visit_ins(&mut self, x: &u8) {
        self.x_pretty.push(*x);
        self.inb_pretty.push(b'+');
        self.y_pretty.push(b'-');
    }

    fn finish(self) -> Self::Output {
        let Self {
            ncol,
            x_pretty,
            y_pretty,
            inb_pretty,
        } = self;

        let mut s = String::default();
        let mut idx = 0;

        assert_eq!(x_pretty.len(), inb_pretty.len());
        assert_eq!(y_pretty.len(), inb_pretty.len());

        let ml = x_pretty.len();

        while idx < ml {
            let rng = idx..min(idx + ncol, ml);
            s.push_str(&String::from_utf8_lossy(&x_pretty[rng.clone()]));
            s.push('\n');

            s.push_str(&String::from_utf8_lossy(&inb_pretty[rng.clone()]));
            s.push('\n');

            s.push_str(&String::from_utf8_lossy(&y_pretty[rng]));
            s.push('\n');

            s.push_str("\n\n");
            idx += ncol;
        }

        s
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlignmentMergeVisitor<T> {
    buf: Vec<T>,
}

impl<T> AlignmentVisitor<T> for AlignmentMergeVisitor<T>
where
    T: Clone,
{
    type Output = Vec<T>;

    #[inline]
    fn visit_prefix_x(&mut self, x: &[T]) {
        self.buf.extend_from_slice(x)
    }

    #[inline]
    fn visit_prefix_y(&mut self, y: &[T]) {
        self.buf.extend_from_slice(y)
    }

    #[inline]
    fn visit_match(&mut self, x: &T, _y: &T) {
        self.buf.push(x.clone())
    }

    #[inline]
    fn visit_subst(&mut self, _x: &T, y: &T) {
        self.buf.push(y.clone())
    }

    #[inline]
    fn visit_del(&mut self, _y: &T) {}

    #[inline]
    fn visit_ins(&mut self, x: &T) {
        self.buf.push(x.clone())
    }

    #[inline]
    fn finish(self) -> Self::Output {
        self.buf
    }
}

#[derive(Clone, Debug)]
pub enum SequenceMatch<T> {
    Matched,
    Unmatched,
    Custom(T),
}

#[derive(Clone, Debug)]
pub struct AlignedSequence<T> {
    pub num_deleted_x: usize,
    pub num_deleted_y: usize,
    pub value: Vec<AlignedToken<T>>,
}

impl<T> FromIterator<T> for AlignedSequence<T> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Self {
            num_deleted_x: 0,
            num_deleted_y: 0,
            value: iter.into_iter().map(AlignedToken::new).collect(),
        }
    }
}

impl<T> AsRef<Self> for AlignedSequence<T> {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T> AsRef<[AlignedToken<T>]> for AlignedSequence<T> {
    #[inline]
    fn as_ref(&self) -> &[AlignedToken<T>] {
        &self.value
    }
}

impl fmt::Display for AlignedSequence<u8> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes: Vec<_> = self.value.iter().map(|token| token.data).collect();
        String::from_utf8_lossy(&bytes).fmt(f)
    }
}

impl fmt::Display for AlignedSequence<char> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.iter().try_for_each(|s| s.data.fmt(f))
    }
}

impl<T> GreedyMultipleSequenceAlignerItem<T> for AlignedSequence<T> {
    #[inline]
    fn build(_: &Self, _: &Self, seq: AlignedSequence<T>) -> Self
    where
        Self: Sized,
    {
        seq
    }
}

impl<T> AlignedSequence<T> {
    pub fn total_matched(&self) -> usize {
        self.value.iter().map(|token| token.count).sum()
    }

    pub fn join(&self, other: &Self, sep: Option<&[T]>) -> Self
    where
        T: Clone,
    {
        Self {
            num_deleted_x: self.num_deleted_x + other.num_deleted_x,
            num_deleted_y: self.num_deleted_y + other.num_deleted_y,
            value: {
                let mut buf = Vec::with_capacity(self.value.len() + other.value.len());
                buf.extend_from_slice(&self.value);
                if let Some(sep) = sep {
                    buf.extend(sep.iter().cloned().map(AlignedToken::new));
                }
                buf.extend_from_slice(&other.value);
                buf
            },
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AlignedToken<T> {
    pub count: usize,
    pub data: T,
}

impl<T> AlignedToken<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self { count: 1, data }
    }
}

impl<T> PartialEq for AlignedToken<T>
where
    T: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T> Eq for AlignedToken<T> where T: Eq {}

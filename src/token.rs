use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::msa::{AlignedSequence, AlignedToken, AlignmentVisitor};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub id: i32,
    pub t0: Duration,
    pub t1: Duration,
}

impl PartialEq for TokenData {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TokenData {}

#[derive(Clone, Debug)]
pub struct AlignmentTokenMergeVisitor<T> {
    buf: Vec<AlignmentToken<T>>,
    num_deleted_x: usize,
    num_deleted_y: usize,
}

impl<T> Default for AlignmentTokenMergeVisitor<T> {
    #[inline]
    fn default() -> Self {
        Self {
            buf: Default::default(),
            num_deleted_x: 0,
            num_deleted_y: 0,
        }
    }
}

impl<T> AlignmentVisitor<AlignedToken<T>> for AlignmentTokenMergeVisitor<AlignedToken<T>>
where
    T: Clone,
{
    type Output = AlignedSequence<T>;

    #[inline]
    fn visit_prefix_x(&mut self, x: &[AlignedToken<T>]) {
        self.buf.extend(
            x.iter()
                .cloned()
                .map(|AlignedToken { count, data }| AlignmentToken {
                    data: AlignedToken { count, data },
                    op: AlignmentTokenOp::Prefix,
                }),
        )
    }

    #[inline]
    fn visit_prefix_y(&mut self, y: &[AlignedToken<T>]) {
        self.num_deleted_y += y.len()
    }

    #[inline]
    fn visit_match(&mut self, x: &AlignedToken<T>, y: &AlignedToken<T>) {
        self.buf.push(AlignmentToken {
            data: AlignedToken {
                count: x.count.max(y.count) + 1,
                data: x.data.clone(),
            },
            op: AlignmentTokenOp::Match,
        })
    }

    #[inline]
    fn visit_subst(&mut self, x: &AlignedToken<T>, y: &AlignedToken<T>) {
        let (data, other) = if x.count >= y.count {
            self.num_deleted_y += 1;
            (x, y)
        } else {
            self.num_deleted_x += 1;
            (y, x)
        };
        self.buf.push(AlignmentToken {
            data: data.clone(),
            op: AlignmentTokenOp::Subst {
                other: other.clone(),
            },
        })
    }

    #[inline]
    fn visit_del(&mut self, y: &AlignedToken<T>) {
        self.num_deleted_x += 1;
        self.buf.push(AlignmentToken {
            data: y.clone(),
            op: AlignmentTokenOp::Del,
        })
    }

    #[inline]
    fn visit_ins(&mut self, x: &AlignedToken<T>) {
        self.buf.push(AlignmentToken {
            data: x.clone(),
            op: AlignmentTokenOp::Ins,
        })
    }

    #[inline]
    fn visit_suffix_x(&mut self, x: &[AlignedToken<T>]) {
        self.num_deleted_x += x.len()
    }

    #[inline]
    fn visit_suffix_y(&mut self, y: &[AlignedToken<T>]) {
        self.buf
            .extend(y.iter().cloned().map(|data| AlignmentToken {
                data,
                op: AlignmentTokenOp::Ins,
            }))
    }

    #[inline]
    fn finish(self) -> Self::Output {
        let Self {
            buf,
            num_deleted_x,
            num_deleted_y,
        } = self;

        AlignedSequence {
            value: buf
                .into_iter()
                // we don't want to have deleted data
                .filter(|AlignmentToken { op, .. }| !matches!(op, AlignmentTokenOp::Del))
                .map(|AlignmentToken { data, .. }| data)
                .collect(),
            num_deleted_x,
            num_deleted_y,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct AlignmentToken<T> {
    data: T,
    op: AlignmentTokenOp<T>,
}

impl<T> PartialEq for AlignmentToken<T>
where
    T: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T> Eq for AlignmentToken<T> where T: Eq {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AlignmentTokenOp<T> {
    Prefix,
    Match,
    Subst { other: T },
    Del,
    Ins,
}

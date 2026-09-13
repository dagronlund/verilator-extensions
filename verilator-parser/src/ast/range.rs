use serde::{Deserialize, Serialize};

/// A range of indices, inclusive of both ends. The left index may be greater
/// than the right index (in which case the range is in descending order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// The first index of the range, inclusive.
    pub left: isize,
    /// The last index of the range, inclusive.
    pub right: isize,
}

impl Range {
    pub fn swap(self) -> Self {
        Self {
            left: self.right,
            right: self.left,
        }
    }

    pub fn len(self) -> usize {
        self.left.abs_diff(self.right).checked_add(1).unwrap()
    }

    pub fn is_empty(self) -> bool {
        false
    }
}

pub struct RangeIterator {
    next: isize,
    step: isize,
    remaining: usize,
}

impl Iterator for RangeIterator {
    type Item = isize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let value = self.next;
        self.next = self.next.saturating_add(self.step);
        self.remaining -= 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for RangeIterator {}

impl IntoIterator for Range {
    type Item = isize;
    type IntoIter = RangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        RangeIterator {
            next: self.left,
            step: if self.left <= self.right { 1 } else { -1 },
            remaining: self.len(),
        }
    }
}

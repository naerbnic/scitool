use std::ops::{Bound, RangeBounds};

use num::NumCast;

#[derive(Clone, Copy, Debug)]
pub struct OffsetSize<T> {
    offset: T,
    size: T,
}

impl<T> OffsetSize<T>
where
    T: Copy,
{
    pub fn offset(&self) -> T {
        self.offset
    }

    pub fn size(&self) -> T {
        self.size
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Range<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    start: T,
    end: Option<T>,
}

impl<T> Range<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    pub fn cast_to<T2>(&self) -> Range<T2>
    where
        T2: num::PrimInt + num::Unsigned + 'static,
    {
        Range {
            start: NumCast::from(self.start).expect("Failed to cast range start"),
            end: self
                .end
                .as_ref()
                .map(|v| NumCast::from(*v).expect("Failed to cast range end")),
        }
    }

    pub fn coerce_to<T2>(&self) -> Range<T2>
    where
        T: Into<T2>,
        T2: num::PrimInt + num::Unsigned + 'static,
    {
        Range {
            start: self.start.into(),
            end: self.end.as_ref().map(|v| (*v).into()),
        }
    }

    pub fn is_full_range(&self) -> bool {
        self.start == T::zero() && self.end.is_none()
    }

    pub fn as_range_bounds(&self) -> impl RangeBounds<T> + 'static {
        (
            Bound::Included(self.start),
            match &self.end {
                Some(v) => Bound::Excluded(*v),
                None => Bound::Unbounded,
            },
        )
    }

    pub fn start(&self) -> T {
        self.start
    }

    pub fn end(&self) -> Option<T> {
        self.end
    }

    pub fn size(&self) -> Option<T> {
        self.end.map(|end| end - self.start)
    }

    /// Create a range that is relative to this range.
    #[must_use]
    pub fn new_relative(&self, inner: Range<T>) -> Range<T> {
        let start = self.start + inner.start;

        let end = match inner.end {
            Some(v) => {
                let new_end = self.start + v;
                if let Some(end) = self.end {
                    assert!(new_end <= end, "Relative range end out of bounds");
                }
                Some(new_end)
            }
            None => None,
        };

        Range { start, end }
    }
}

impl<T> Range<T>
where
    T: num::PrimInt + num::Unsigned,
{
    pub fn from_range<R>(range: R) -> Self
    where
        R: RangeBounds<T>,
    {
        let start = match range.start_bound() {
            Bound::Included(v) => *v,
            Bound::Excluded(v) => {
                assert!(*v != T::max_value(), "Start bound overflow");
                *v + T::one()
            }
            Bound::Unbounded => T::zero(),
        };

        let end = match range.end_bound() {
            Bound::Included(v) => {
                assert!(*v != T::max_value(), "End bound overflow");
                Some(*v + T::one())
            }
            Bound::Excluded(v) => Some(*v),
            Bound::Unbounded => None,
        };
        Self { start, end }
    }
}

impl<T> RangeBounds<T> for Range<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }

    fn end_bound(&self) -> Bound<&T> {
        match &self.end {
            Some(v) => Bound::Excluded(v),
            None => Bound::Unbounded,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoundedRange<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    start: T,
    end: T,
}

impl<T> BoundedRange<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    pub fn from_size(size: T) -> Self {
        Self {
            start: T::zero(),
            end: size,
        }
    }

    pub fn start(&self) -> T {
        self.start
    }

    pub fn end(&self) -> T {
        self.end
    }

    pub fn size(&self) -> T {
        self.end - self.start
    }

    pub fn as_range(&self) -> Range<T> {
        Range {
            start: self.start,
            end: Some(self.end),
        }
    }

    /// Shifts this range down by the given offset. If the offset is larger
    /// than the start of the range, the new start will be zero, and the end
    /// will be adjusted accordingly. Shifting past the entire range ends up
    /// with a zero-sized range at zero.
    #[must_use]
    pub fn shift_down_by(&self, offset: T) -> BoundedRange<T> {
        BoundedRange {
            start: self.start.saturating_sub(offset),
            end: self.end.saturating_sub(offset),
        }
    }

    /// Returns the intersection of this range with another range. Returns
    /// None if the intersection is empty.
    pub fn intersect<R>(&self, other: R) -> Option<BoundedRange<T>>
    where
        R: RangeBounds<T>,
    {
        let other = Range::from_range(other);
        let start = std::cmp::max(self.start, other.start);
        let end = match other.end() {
            Some(v) => std::cmp::min(self.end, v),
            None => self.end,
        };
        if end <= start {
            None
        } else {
            Some(BoundedRange { start, end })
        }
    }

    pub fn contains(&self, other: BoundedRange<T>) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    #[must_use]
    pub fn new_relative<R>(&self, inner: R) -> BoundedRange<T>
    where
        R: RangeBounds<T>,
    {
        let inner = Range::from_range(inner);
        let start = self.start + inner.start;

        let end = match inner.end {
            Some(v) => self.start + v,
            None => self.end,
        };

        assert!(end <= self.end, "Relative range end out of bounds");

        BoundedRange { start, end }
    }

    pub fn cast_to<T2>(&self) -> BoundedRange<T2>
    where
        T2: num::PrimInt + num::Unsigned + 'static,
    {
        BoundedRange {
            start: NumCast::from(self.start).expect("Failed to cast range start"),
            end: NumCast::from(self.end).expect("Failed to cast range end"),
        }
    }
}

impl<T> RangeBounds<T> for BoundedRange<T>
where
    T: num::PrimInt + num::Unsigned + 'static,
{
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }

    fn end_bound(&self) -> Bound<&T> {
        Bound::Excluded(&self.end)
    }
}

use std::ops::RangeBounds;

use str_indices::lines_lf;

use super::gap_buffer::ChunkSummary;
use super::utils::*;
use crate::range_bounds_to_start_end;
use crate::tree::Summarize;

/// A slice of a [`GapBuffer`](super::gap_buffer::GapBuffer).
///
/// TODO: docs
#[derive(Copy, Clone, Default)]
pub(super) struct GapSlice<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) len_first_segment: u16,
    pub(super) len_second_segment: u16,
}

impl std::fmt::Debug for GapSlice<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("\"")?;
        debug_no_quotes(self.first_segment(), f)?;
        write!(f, "{:~^1$}", "", self.len_gap())?;
        debug_no_quotes(self.second_segment(), f)?;
        f.write_str("\"")?;
        Ok(())
    }
}

// impl<'a> From<&'a str> for GapSlice<'a> {
//     #[inline]
//     fn from(s: &str) -> Self {
//         Self {
//             bytes: s.as_bytes(),
//             len_first_segment: s.len() as u16,
//             len_gap: 0,
//             len_second_segment: 0,
//         }
//     }
// }

impl<'a> GapSlice<'a> {
    /// Returns the byte at the given index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds, i.e. greater than or equal to
    /// [`len()`](Self::len()).
    #[inline]
    pub(super) fn byte(&self, byte_index: usize) -> u8 {
        debug_assert!(byte_index < self.len());

        if byte_index < self.len_first_segment() {
            self.first_segment().as_bytes()[byte_index]
        } else {
            self.second_segment().as_bytes()
                [byte_index - self.len_first_segment()]
        }
    }

    #[inline]
    pub(super) fn byte_slice<R>(&self, byte_range: R) -> GapSlice<'a>
    where
        R: RangeBounds<usize>,
    {
        let (start, end) =
            range_bounds_to_start_end(byte_range, 0, self.len());

        debug_assert!(start <= end);
        debug_assert!(end <= self.len());

        match (
            start <= self.len_first_segment(),
            end <= self.len_first_segment(),
        ) {
            (true, true) => Self {
                bytes: &self.bytes[start..end],
                len_first_segment: (end - start) as u16,
                len_second_segment: 0,
            },

            (true, false) => Self {
                bytes: &self.bytes[start..end + self.len_gap()],
                len_first_segment: self.len_first_segment - (start as u16),
                len_second_segment: (end as u16) - self.len_first_segment,
            },

            (false, false) => Self {
                bytes: &self.bytes
                    [start + self.len_gap()..end + self.len_gap()],
                len_first_segment: 0,
                len_second_segment: (end - start) as u16,
            },

            (false, true) => unreachable!(),
        }
    }

    /// Returns the byte offset of the start of the given line.
    #[inline]
    pub(super) fn byte_of_line(&self, line_index: usize) -> usize {
        let offset_first_segment =
            lines_lf::to_byte_idx(self.first_segment(), line_index);

        if offset_first_segment < self.len_first_segment() {
            offset_first_segment
        } else {
            let line_breaks_in_first_segment =
                lines_lf::count_breaks(self.first_segment());

            self.len_first_segment()
                + lines_lf::to_byte_idx(
                    self.second_segment(),
                    line_index - line_breaks_in_first_segment,
                )
        }
    }

    #[inline]
    pub(super) fn empty() -> Self {
        Self::default()
    }

    #[inline]
    pub(super) fn first_segment(&self) -> &'a str {
        // SAFETY: this `GapSlice` was obtained by slicing a `GapBuffer` whose
        // first `len_first_segment` bytes were valid UTF-8.
        unsafe {
            std::str::from_utf8_unchecked(
                &self.bytes[..self.len_first_segment()],
            )
        }
    }

    /// Returns `true` if it ends with a newline (either LF or CRLF).
    #[inline]
    pub(super) fn has_trailing_newline(&self) -> bool {
        last_byte_is_newline(self.last_segment())
    }

    #[inline]
    pub(super) fn is_char_boundary(&self, byte_offset: usize) -> bool {
        debug_assert!(byte_offset <= self.len());

        if byte_offset <= self.len_first_segment() {
            self.first_segment().is_char_boundary(byte_offset)
        } else {
            self.second_segment()
                .is_char_boundary(byte_offset - self.len_first_segment())
        }
    }

    /// The second segment if it's not empty, or the first one otherwise.
    #[inline]
    pub(super) fn last_segment(&self) -> &'a str {
        if !self.second_segment().is_empty() {
            self.second_segment()
        } else {
            self.first_segment()
        }
    }

    #[inline]
    pub(super) fn len(&self) -> usize {
        self.len_first_segment() + self.len_second_segment()
    }

    #[inline]
    pub(super) fn len_first_segment(&self) -> usize {
        self.len_first_segment as _
    }

    #[inline]
    fn len_gap(&self) -> usize {
        self.bytes.len() - self.len()
    }

    #[inline]
    pub(super) fn len_second_segment(&self) -> usize {
        self.len_second_segment as _
    }

    // #[inline]
    // fn len_trailing_gap(&self) -> usize {
    //     self.bytes.len() - self.len() - self.len_middle_gap()
    // }

    #[inline]
    pub(super) fn second_segment(&self) -> &'a str {
        // SAFETY: this `GapSlice` was obtained by slicing a `GapBuffer` whose
        // last `len_second_segment` bytes were valid UTF-8.
        unsafe {
            std::str::from_utf8_unchecked(
                &self.bytes[self.bytes.len() - self.len_second_segment()..],
            )
        }
    }

    #[inline]
    pub(super) fn split_at_offset(&self, byte_offset: usize) -> (Self, Self) {
        (self.byte_slice(..byte_offset), self.byte_slice(byte_offset..))
    }

    // /// Return the segment containing the given byte index.
    // #[inline]
    // pub(super) fn segment_at_index(&self, byte_index: usize) -> &'a str {
    //     debug_assert!(byte_index < self.len());

    //     if byte_index < self.len_first_segment() {
    //         self.first_segment()
    //     } else {
    //         self.second_segment()
    //     }
    // }
}

impl Summarize for GapSlice<'_> {
    type Summary = ChunkSummary;

    #[inline]
    fn summarize(&self) -> Self::Summary {
        ChunkSummary {
            bytes: self.len(),
            line_breaks: lines_lf::count_breaks(self.first_segment())
                + lines_lf::count_breaks(self.second_segment()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rope::gap_buffer::GapBuffer;
    use crate::tree::AsSlice;

    #[test]
    fn debug_slice() {
        let buffer = GapBuffer::<10>::from("Hello");
        assert_eq!("\"He~~~~~llo\"", format!("{:?}", buffer.as_slice()));
    }
}
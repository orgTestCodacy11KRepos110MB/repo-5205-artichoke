use core::fmt;
use core::mem;
use core::ops::Range;
use core::slice::SliceIndex;

use bstr::ByteSlice;

use crate::iter::{Bytes, Iter, IterMut};
use crate::ord::OrdError;

#[repr(transparent)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Utf8Str {
    inner: [u8],
}

impl fmt::Debug for Utf8Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Utf8Str").field("bytes", self.inner.as_bstr()).finish()
    }
}

impl Utf8Str {
    fn from_bytes(bytes: &[u8]) -> &Utf8Str {
        // Safety:
        //
        // `Utf8Str` is a `repr(transparent)` wrapper around `[u8]`, which means
        // `&Utf8Str` is guaranteed to have the same layout, size, and alignment
        // as `&[u8]`.
        unsafe { mem::transmute(bytes) }
    }
}

/// Slice and pointer accessors
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn as_byte_slice(&self) -> &[u8] {
        &self.inner
    }

    #[inline]
    #[must_use]
    pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        &mut self.inner
    }

    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline]
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }
}

/// Core iterators
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn iter(&self) -> Iter<'_> {
        let slice = self.inner.as_byte_slice();
        Iter::from_slice(slice)
    }

    #[inline]
    #[must_use]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        let slice = self.inner.as_mut_byte_slice();
        IterMut::from_slice_mut(slice)
    }

    #[inline]
    #[must_use]
    pub fn bytes(&self) -> Bytes<'_> {
        let slice = self.inner.as_byte_slice();
        Bytes::from_slice(slice)
    }
}

/// Size and capacity
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Indexing
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn get<I>(&self, index: I) -> Option<&I::Output>
    where
        I: SliceIndex<[u8]>,
    {
        self.inner.get(index)
    }

    #[inline]
    #[must_use]
    pub fn get_mut<I>(&mut self, index: I) -> Option<&mut I::Output>
    where
        I: SliceIndex<[u8]>,
    {
        self.inner.get_mut(index)
    }

    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked<I>(&self, index: I) -> &I::Output
    where
        I: SliceIndex<[u8]>,
    {
        self.inner.get_unchecked(index)
    }

    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked_mut<I>(&mut self, index: I) -> &mut I::Output
    where
        I: SliceIndex<[u8]>,
    {
        self.inner.get_unchecked_mut(index)
    }
}

/// Encoding-aware character indexing APIs
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn char_len(&self) -> usize {
        let mut bytes = self.as_byte_slice();
        let tail = if let Some(idx) = bytes.find_non_ascii_byte() {
            idx
        } else {
            return bytes.len();
        };
        // Safety:
        //
        // If `ByteSlice::find_non_ascii_byte` returns `Some(_)`, the index is
        // guaranteed to be a valid index within `bytes`.
        bytes = unsafe { bytes.get_unchecked(tail..) };
        if simdutf8::basic::from_utf8(bytes).is_ok() {
            // Addition is guaranteed to never overflow because the char len is
            // at most the length of the slice and `Utf8Str` is derived from a
            // `Vec`, which has a max allocation size of `isize::MAX`.
            return tail + bytecount::num_chars(bytes);
        }
        let mut char_len = tail;
        for chunk in bytes.utf8_chunks() {
            // Addition is guaranteed to never overflow because the char len is
            // at most the length of the slice and `Utf8Str` is derived from a
            // `Vec`, which has a max allocation size of `isize::MAX`.
            char_len += bytecount::num_chars(chunk.valid().as_bytes());
            char_len += chunk.invalid().len();
        }
        char_len
    }

    #[inline]
    #[must_use]
    pub fn get_char(&self, index: usize) -> Option<&'_ Utf8Str> {
        // Fast path rejection for indexes beyond bytesize, which is
        // cheap to retrieve.
        if index >= self.len() {
            return None;
        }
        let bytes = self.as_byte_slice();
        // Fast path for trying to treat the conventionally UTF-8 string as
        // entirely ASCII.
        //
        // If the string is either all ASCII or all ASCII for a prefix of the
        // string that contains the range we wish to slice, fallback to byte
        // slicing as in the ASCII and binary fast path.
        let non_ascii_idx = match bytes.find_non_ascii_byte() {
            // The entire string is ASCII, so byte indexing <=> char
            // indexing.
            None => return bytes.get(index..=index).map(Utf8Str::from_bytes),
            // The string is all ASCII in the region we care about
            Some(non_ascii_idx) if non_ascii_idx > index => return bytes.get(index..=index).map(Utf8Str::from_bytes),
            Some(non_ascii_idx) => non_ascii_idx,
        };
        // Safety:
        //
        // If `ByteSlice::find_non_ascii_byte` returns `Some(_)`, the index is
        // guaranteed to be a valid index within `bytes`.
        //
        // The retrieved slice is at least one byte long, which means the loop
        // below will have at least one iteration.
        let mut slice = unsafe { bytes.get_unchecked(non_ascii_idx..) };
        debug_assert!(
            index > non_ascii_idx,
            "get_char expects to find char after non-ASCII byte"
        );
        let mut remaining = index - non_ascii_idx;

        // This loop will terminate when either:
        //
        // - It counts `index` number of characters.
        // - It consumes the entire slice when scanning for the `index`th character.
        //
        // The loop will advance by at least one byte every iteration.
        loop {
            match bstr::decode_utf8(slice) {
                // If we've run out of slice while trying to find the
                // `index`th character, the lookup fails and we return `nil`.
                (_, 0) => return None,

                // The next two arms mean we've reached the `index`th character.
                //
                // If `decode_utf8` yields `Some(ch)`, the next byte sequence is
                // a valid UTF-8 encoded character, so return a slice of the next
                // `size` bytes.
                (Some(_), size) if remaining == 0 => return Some(Utf8Str::from_bytes(&slice[..size])),
                // ... or if `decode_utf8` yields `None`, the next byte sequence
                // is not a valid UTF-8 encoded character, so return a slice of
                // the next single byte.
                //
                // Size is guaranteed to be positive per the first arm which
                // means this slice operation will not panic and will yield a
                // non-empty slice.
                (None, _) if remaining == 0 => return Some(Utf8Str::from_bytes(&slice[..1])),

                // We found a single UTF-8 encoded character keep track of the
                // count and advance the substring to continue decoding.
                //
                // `remaining` is guaranteed to be positive per the two arms
                // above, so this subtraction will not underflow and the loop is
                // guaranteed to terminate.
                (Some(_), size) => {
                    slice = &slice[size..];
                    remaining -= 1;
                }

                // The next two arms handle the case where we have encountered
                // an invalid UTF-8 byte sequence.
                //
                // In this case, `decode_utf8` will return slices whose length
                // is `1..=3`. The length of this slice is the number of
                // "characters" we can advance the loop by.
                //
                // If the invalid UTF-8 sequence contains more bytes than we have
                // remaining to get to the `index`th char, then the target
                // character is inside the invalid UTF-8 sequence.
                (None, size) if remaining < size => return Some(Utf8Str::from_bytes(&slice[remaining..=remaining])),
                // ... or if there are more characters remaining than the number
                // of bytes yielded in the invalid UTF-8 byte sequence, count
                // `size` bytes and advance the slice to continue decoding.
                //
                // `remaining` is guaranteed to be at least `size` per the arm
                // above, so this subtraction will not underflow and the loop is
                // guaranteed to terminate.
                (None, size) => {
                    slice = &slice[size..];
                    remaining -= size;
                }
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn get_char_slice(&self, range: Range<usize>) -> Option<&'_ Utf8Str> {
        let Range { start, end } = range;
        // Fast path rejection for empty ranges.
        //
        // ```
        // [3.1.1] > s = "abc"
        // => "abc"
        // [3.1.1] > s.length
        // => 3
        // [3.1.1] > s[0, 0]
        // => ""
        // [3.1.1] > s[1, 0]
        // => ""
        // [3.1.1] > s[2, 0]
        // => ""
        // [3.1.1] > s[3, 0]
        // => ""
        // [3.1.1] > s[4, 0]
        // => nil
        // [3.1.1] > s[100, 0]
        // => nil
        // ```
        if start == end {
            return if start <= self.len() {
                Some(Utf8Str::from_bytes(b""))
            } else {
                None
            };
        }

        // Fast path rejection for indexes beyond bytesize, which is
        // cheap to retrieve.
        //
        // ```
        // [3.1.1] > s = "abc"
        // => "abc"
        // [3.1.1] > s.length
        // => 3
        // [3.1.1] > s[2, 10]
        // => "c"
        // [3.1.1] > s[3, 10]
        // => ""
        // [3.1.1] > s[4, 10]
        // => nil
        // ```
        match self.len() {
            len if start > len => return None,
            len if start == len => return Some(Utf8Str::from_bytes(b"")),
            _ => {}
        }

        let bytes = self.as_byte_slice();
        // Fast path for trying to treat the conventionally UTF-8 string
        // as entirely ASCII.
        //
        // If the string is either all ASCII or all ASCII for the subset
        // of the string we wish to slice, fallback to byte slicing as in
        // the ASCII and binary fast path.
        //
        // Perform the same saturate-to-end slicing mechanism if `end`
        // is beyond the character length of the string.
        let consumed = match bytes.find_non_ascii_byte() {
            // The entire string is ASCII, so byte indexing <=> char
            // indexing.
            None => {
                return self
                    .inner
                    .get(start..end)
                    .or_else(|| self.inner.get(start..))
                    .map(Utf8Str::from_bytes)
            }
            // The whole substring we are interested in is ASCII, so
            // byte indexing is still valid.
            Some(non_ascii_byte_offset) if non_ascii_byte_offset > end => {
                return self.get(start..end).map(Utf8Str::from_bytes)
            }
            // We turn non-ASCII somewhere inside before the substring we're
            // interested in, so consume that much.
            Some(non_ascii_byte_offset) if non_ascii_byte_offset <= start => non_ascii_byte_offset,
            // This means we turn non-ASCII somewhere inside the substring.
            // Consume up to start.
            Some(_) => start,
        };
        // Safety:
        //
        // If `ByteSlice::find_non_ascii_byte` returns `Some(_)`, the index is
        // guaranteed to be a valid index within `bytes`.
        //
        // The retrieved slice is at least one byte long, which means the loop
        // below will have at least one iteration.
        let mut slice = unsafe { bytes.get_unchecked(consumed..) };
        debug_assert!(
            index > consumed,
            "get_char_slice expects to find char after non-ASCII byte"
        );
        // Count of "characters" remaining until the `start`th character.
        let mut remaining_to_start_of_slice = start - consumed;
        // This loop will terminate when either:
        //
        // - It counts `start` number of characters.
        // - It consumes the entire slice when scanning for the `start`th
        //   character.
        //
        // The loop will advance by at least one byte every iteration.
        loop {
            if remaining_to_start_of_slice == 0 {
                break;
            }
            match bstr::decode_utf8(slice) {
                // If we've run out of slice while trying to find the `start`th
                // character, the lookup fails and we return `nil`.
                (_, 0) => return None,

                // We found a single UTF-8 encoded character. Keep track of the
                // count and advance the substring to continue decoding.
                //
                // If there's only one more to go, advance and stop the loop.
                (Some(_), size) if remaining_to_start_of_slice == 1 => {
                    slice = &slice[size..];
                    break;
                }
                // Otherwise, keep track of the character we observed and advance
                // the slice to continue decoding.
                (Some(_), size) => {
                    slice = &slice[size..];
                    remaining_to_start_of_slice -= 1;
                }

                // The next two arms handle the case where we have encountered
                // an invalid UTF-8 byte sequence.
                //
                // In this case, `decode_utf8` will return slices whose length
                // is `1..=3`. The length of this slice is the number of
                // "characters" we can advance the loop by.
                //
                // If the invalid UTF-8 sequence contains more bytes than we
                // have remaining to get to the `start`th char, then we can
                // break the loop directly.
                (None, size) if remaining_to_start_of_slice <= size => {
                    slice = &slice[remaining_to_start_of_slice..];
                    break;
                }
                // If there are more characters remaining than the number of
                // bytes yielded in the invalid UTF-8 byte sequence, count
                // `size` bytes and advance the slice to continue decoding.
                (None, size) => {
                    slice = &slice[size..];
                    remaining_to_start_of_slice -= size;
                }
            }
        }

        // Scan the slice for the span of characters we want to return.
        let remaining_chars_in_slice = end - start;
        // We know the number of chars remaining is positive fast-pathed the
        // zero length case above.
        debug_assert!(remaining_chars_in_slice > 0);

        // keep track of the start of the substring from the `start`th
        // character.
        let slice_index = 0;
        let (mut substr, mut tail) = slice.split_at(slice_index);

        // This loop will terminate when either:
        //
        // - It counts the next `start - end` number of characters.
        // - It consumes the entire slice when scanning for the `end`th
        //   character.
        //
        // The loop will advance by at least one byte every iteration.
        loop {
            match bstr::decode_utf8(tail) {
                // If we've run out of slice while trying to find the `end`th
                // character, saturate the slice to the end of the string.
                (_, 0) => return Some(Utf8Str::from_bytes(substr)),

                // We found a single UTF-8 encoded character. Keep track of the
                // count and advance the substring to continue decoding.
                //
                // If there's only one more character to go, take these bytes
                // and return the substring slice.
                (Some(_), size) if remaining_chars_in_slice == 1 => {
                    slice_index += size;
                    (substr, tail) = slice.split_at(slice_index);
                    return Some(Utf8Str::from_bytes(substr));
                }
                // Otherwise, keep track of the character we observed and advance
                // the slice to continue decoding.
                (Some(_), size) => {
                    slice_index += size;
                    remaining_chars_in_slice -= 1;
                    (substr, tail) = slice.split_at(slice_index);
                }

                // The next two arms handle the case where we have encountered
                // an invalid UTF-8 byte sequence.
                //
                // In this case, `decode_utf8` will return slices whose length
                // is `1..=3`. The length of this slice is the number of
                // "characters" we can advance the loop by.
                //
                // If the invalid UTF-8 sequence contains more bytes than we
                // have remaining to get to the `end`th char, then we can break
                // the loop directly.
                (None, size) if remaining_chars_in_slice <= size => {
                    slice_index += remaining_chars_in_slice;
                    (substr, tail) = slice.split_at(slice_index);
                    return Some(Utf8Str::from_bytes(substr));
                }
                // If there are more characters remaining than the number of
                // bytes yielded in the invalid UTF-8 byte sequence, count
                // `size` bytes and advance the slice to continue decoding.
                (None, size) => {
                    slice_index += size;
                    remaining_chars_in_slice -= size;
                    (substr, tail) = slice.split_at(slice_index);
                }
            }
        }
    }
}

/// Encoding validity
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn is_ascii_only(&self) -> bool {
        self.inner.is_ascii()
    }

    #[inline]
    #[must_use]
    pub fn is_valid_encoding(&self) -> bool {
        if self.is_ascii_only() {
            return true;
        }

        simdutf8::basic::from_utf8(&self.inner).is_ok()
    }
}

/// Prefix and suffix matching
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn starts_with<I>(&self, iter: I) -> bool
    where
        I: Iterator,
        I::Item: AsRef<[u8]>,
    {
        let bytes = self.as_byte_slice();
        iter.into_iter().any(|prefix| bytes.starts_with(prefix.as_ref()))
    }

    #[inline]
    #[must_use]
    pub fn ends_with<I>(&self, iter: I) -> bool
    where
        I: Iterator,
        I::Item: AsRef<[u8]>,
    {
        let bytes = self.as_byte_slice();
        iter.into_iter().any(|prefix| bytes.ends_with(prefix.as_ref()))
    }
}

/// Codepoints
impl Utf8Str {
    #[inline]
    #[must_use]
    pub fn chr(&self) -> &Utf8Str {
        let slice = match bstr::decode_utf8(self.inner) {
            (Some(_), size) => &self.inner[..size],
            (None, 0) => &[],
            (None, _) => &self.inner[..1],
        };
        Utf8Str::from_bytes(slice)
    }

    #[inline]
    pub fn ord(&self) -> Result<u32, OrdError> {
        let (ch, size) = bstr::decode_utf8(self.inner);
        match ch {
            // All `char`s are valid `u32`s
            Some(ch) => Ok(u32::from(ch)),
            None if size == 0 => Err(OrdError::empty_string()),
            None => Err(OrdError::invalid_utf8_byte_sequence()),
        }
    }
}

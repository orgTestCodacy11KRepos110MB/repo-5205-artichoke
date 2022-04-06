use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::fmt;
use core::mem;
use core::ops::{Deref, DerefMut};

use bstr::ByteVec;

use super::Utf8Str;
use crate::codepoints::InvalidCodepointError;
use crate::iter::IntoIter;

#[repr(transparent)]
#[derive(Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Utf8String {
    inner: Vec<u8>,
}

// Constructors
impl Utf8String {
    pub const fn with_bytes(buf: Vec<u8>) -> Self {
        Self { inner: buf }
    }
}

impl fmt::Debug for Utf8String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Utf8String").field("bytes", &*self).finish()
    }
}

impl Deref for Utf8String {
    type Target = Utf8Str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_utf8_str()
    }
}

impl DerefMut for Utf8String {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_utf8_str()
    }
}

// Buffer and pointer accessors
impl Utf8String {
    #[inline]
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.inner
    }

    #[inline]
    #[must_use]
    pub fn as_utf8_str(&self) -> &Utf8Str {
        let slice = self.inner.as_slice();
        // Safety:
        //
        // `Utf8Str` is a `repr(transparent)` wrapper around `[u8]`, which means
        // `&Utf8Str` is guaranteed to have the same layout, size, and alignment
        // as `&[u8]`.
        unsafe { mem::transmute(slice) }
    }

    #[inline]
    #[must_use]
    pub fn as_mut_utf8_str(&self) -> &mut Utf8Str {
        let slice = self.inner.as_mut_slice();
        // Safety:
        //
        // `Utf8Str` is a `repr(transparent)` wrapper around `[u8]`, which means
        // `&mut Utf8Str` is guaranteed to have the same layout, size, and
        // alignment as `&mut [u8]`.
        unsafe { mem::transmute(slice) }
    }

    #[inline]
    #[must_use]
    pub fn as_byte_slice(&self) -> &[u8] {
        &*self.inner
    }

    #[inline]
    #[must_use]
    pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        &mut *self.inner
    }

    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        // We shadow the slice method of the same name to avoid going through
        // `deref`, which creates an intermediate reference.
        self.inner.as_ptr()
    }

    #[inline]
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        // We shadow the slice method of the same name to avoid going through
        // `deref_mut`, which creates an intermediate reference.
        self.inner.as_mut_ptr()
    }

    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        self.inner.set_len(len);
    }
}

/// Core iterators
impl Utf8String {
    #[inline]
    #[must_use]
    pub fn into_iter(self) -> IntoIter {
        IntoIter::from_vec(self.inner)
    }
}

/// Size and capacity
impl Utf8String {
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

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

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.inner.truncate(len);
    }
}

/// Memory management
impl Utf8String {
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }

    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional);
    }

    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity);
    }
}

/// Pushing and popping bytes, codepoints, and strings
impl Utf8String {
    #[inline]
    pub fn push_byte(&mut self, byte: u8) {
        self.inner.push_byte(byte);
    }

    #[inline]
    pub fn try_push_codepoint(&mut self, codepoint: i64) -> Result<(), InvalidCodepointError> {
        let codepoint = if let Ok(codepoint) = u32::try_from(codepoint) {
            codepoint
        } else {
            return Err(InvalidCodepointError::codepoint_out_of_range(codepoint));
        };
        if let Ok(ch) = char::try_from(codepoint) {
            self.push_char(ch);
            Ok(())
        } else {
            Err(InvalidCodepointError::invalid_utf8_codepoint(codepoint))
        }
    }

    #[inline]
    pub fn push_char(&mut self, ch: char) {
        self.inner.push_char(ch);
    }

    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.inner.push_str(s);
    }

    #[inline]
    pub fn extend_from_byte_slice(&mut self, other: &[u8]) {
        self.inner.extend_from_slice(other);
    }

    #[inline]
    pub fn extend_from_utf8_slice(&mut self, other: &Utf8Str) {
        let other = other.as_byte_slice();
        self.inner.extend_from_slice(other);
    }
}

// TODO: Use roe for case changing operations.
//   UTF-8 case changing needs to be parameterized on the case folding strategy
//   to account for e.g. Turkic or ASCII-only modes.

/// Unicode case mapping
impl Utf8String {
    #[inline]
    pub fn make_capitalized(&mut self) {
        // This allocation assumes that in the common case, capitalizing and
        // lower-casing `char`s do not change the length of the `String`.
        let mut replacement = Vec::with_capacity(self.len());
        let mut bytes = self.inner.as_slice();

        match bstr::decode_utf8(bytes) {
            (Some(ch), size) => {
                // Converting a UTF-8 character to uppercase may yield
                // multiple codepoints.
                for ch in ch.to_uppercase() {
                    replacement.push_char(ch);
                }
                bytes = &bytes[size..];
            }
            (None, size) if size == 0 => return,
            (None, size) => {
                let (substring, remainder) = bytes.split_at(size);
                replacement.extend_from_slice(substring);
                bytes = remainder;
            }
        }

        while !bytes.is_empty() {
            let (ch, size) = bstr::decode_utf8(bytes);
            if let Some(ch) = ch {
                // Converting a UTF-8 character to lowercase may yield
                // multiple codepoints.
                for ch in ch.to_lowercase() {
                    replacement.push_char(ch);
                }
                bytes = &bytes[size..];
            } else {
                let (substring, remainder) = bytes.split_at(size);
                replacement.extend_from_slice(substring);
                bytes = remainder;
            }
        }
        self.inner = replacement;
    }

    #[inline]
    pub fn make_lowercase(&mut self) {
        // This allocation assumes that in the common case, lower-casing
        // `char`s do not change the length of the `String`.
        let mut replacement = Vec::with_capacity(self.len());
        let mut bytes = self.inner.as_slice();

        while !bytes.is_empty() {
            let (ch, size) = bstr::decode_utf8(bytes);
            if let Some(ch) = ch {
                // Converting a UTF-8 character to lowercase may yield
                // multiple codepoints.
                for ch in ch.to_lowercase() {
                    replacement.push_char(ch);
                }
                bytes = &bytes[size..];
            } else {
                let (substring, remainder) = bytes.split_at(size);
                replacement.extend_from_slice(substring);
                bytes = remainder;
            }
        }
        self.inner = replacement;
    }

    #[inline]
    pub fn make_uppercase(&mut self) {
        // This allocation assumes that in the common case, upper-casing
        // `char`s do not change the length of the `String`.
        let mut replacement = Vec::with_capacity(self.len());
        let mut bytes = self.inner.as_slice();

        while !bytes.is_empty() {
            let (ch, size) = bstr::decode_utf8(bytes);
            if let Some(ch) = ch {
                // Converting a UTF-8 character to lowercase may yield
                // multiple codepoints.
                for ch in ch.to_uppercase() {
                    replacement.push_char(ch);
                }
                bytes = &bytes[size..];
            } else {
                let (substring, remainder) = bytes.split_at(size);
                replacement.extend_from_slice(substring);
                bytes = remainder;
            }
        }
        self.inner = replacement;
    }
}

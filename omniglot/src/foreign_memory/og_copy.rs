// -*- fill-column: 80; -*-

use crate::bit_pattern_validate::BitPatternValidate;
use crate::maybe_valid::MaybeValid;

// use super::og_mut_ref::OGMutRef;
// TODO!

// Flag settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_VALIDATION_CHECKS;

/// A wrapper around some initialized memory of size and align equal to that of
/// `T`, which may or may not contain a valid instance of type `T`.
///
/// This type is useful to represent return values or owned copies of memory
/// modified by foreign code. If `T` implements `BitPatternValidate`, it can be
/// safely transmuted into an instance of or reference to its inner type `T`.
#[repr(transparent)]
#[derive(Debug)]
pub struct OGCopy<T> {
    pub(super) inner: MaybeValid<T>,
}

impl<T> OGCopy<T> {
    /// Create a new `OGCopy` from a valid instance of type `T`.
    pub fn new(val: T) -> Self {
        OGCopy {
            inner: MaybeValid::new(val),
        }
    }

    /// Create a new `OGCopy` with zero-initialized contents.
    pub fn zeroed() -> Self {
        OGCopy {
            inner: MaybeValid::zeroed(),
        }
    }

    /// Create an `OGCopy` by filling its contents from a byte-slice.
    ///
    /// # Panic
    ///
    /// This function will panic if the supplied byte slice does not contain
    /// exactly `core::mem::size_of::<T>()` bytes.
    pub fn from_bytes(src: &[u8]) -> Self {
        OGCopy {
            inner: MaybeValid::from_bytes(src),
        }
    }

    pub unsafe fn assume_valid(self) -> T {
        unsafe { self.inner.assume_valid() }
    }

    pub unsafe fn assume_valid_ref(&self) -> &T {
        unsafe { self.inner.assume_valid_ref() }
    }
}

/// Clone an `OGCopy` by performing a copy of its underlying memory.
///
/// `OGCopy` is a wrapper around some initialized memory of size and align equal
/// to that of `T`, which may or may not contain a valid instance of type `T`.
/// As such, it can be safely copied like any other buffer received from foreign
/// code, irrespective of whether the proclaimed type `T` is itself `Clone`.
impl<T> Clone for OGCopy<T> {
    fn clone(&self) -> Self {
        OGCopy {
            inner: MaybeValid::from_bytes(self.inner.as_bytes()),
        }
    }
}

impl<T: BitPatternValidate> OGCopy<T> {
    pub fn validate(self) -> Result<T, Self> {
        if DISABLE_VALIDATION_CHECKS {
            Ok(unsafe { self.assume_valid() })
        } else {
            if unsafe {
                <T as BitPatternValidate>::validate(&self.inner as *const MaybeValid<T> as *const T)
            } {
                Ok(unsafe { self.inner.assume_valid() })
            } else {
                Err(self)
            }
        }
    }

    pub fn validate_ref<'a>(&'a self) -> Option<&'a T> {
        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid_ref() })
        } else {
            if unsafe {
                <T as BitPatternValidate>::validate(&self.inner as *const MaybeValid<T> as *const T)
            } {
                Some(unsafe { self.inner.assume_valid_ref() })
            } else {
                None
            }
        }
    }
}

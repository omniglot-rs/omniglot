// -*- fill-column: 80; -*-

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

impl<T: zerocopy::IntoBytes> OGCopy<T> {
    /// Create a new `OGCopy` from a valid instance of type `T`.
    // Because for any type `T` in general it may contain padding bytes, we
    // require the bound of `T: zerocopy::IntoBytes`, which ensures that all
    // bytes backing `T` are initialized.
    pub fn new(val: T) -> Self {
        OGCopy {
            inner: MaybeValid::new(val),
        }
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

impl<T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout> OGCopy<T> {
    pub fn validate(self) -> Result<T, Self> {
        if DISABLE_VALIDATION_CHECKS {
            Ok(unsafe { self.assume_valid() })
        } else {
            if <T as zerocopy::TryFromBytes>::try_ref_from_bytes(self.inner.as_bytes()).is_ok() {
                Ok(unsafe { self.inner.assume_valid() })
            } else {
                Err(self)
            }
        }
    }

    // While `T` may have padding bytes, we know that the reference to `T` is
    // immutable and `T` itself does not feature interior mutability. As such,
    // none of `T`'s padding bytes can be written to (with potentially
    // uninitialized data), and so providing this reference is safe. Providing a
    // mutable reference would, in turn, not be safe as that would mutably
    // expose `T`'s padding bytes.
    pub fn validate_ref<'a>(&'a self) -> Option<&'a T> {
        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid_ref() })
        } else {
            <T as zerocopy::TryFromBytes>::try_ref_from_bytes(self.inner.as_bytes()).ok()
        }
    }
}

impl<T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout> OGCopy<T> {
    pub fn valid(self) -> T {
        unsafe { self.assume_valid() }
    }

    // While `T` may have padding bytes, we know that the reference to `T` is
    // immutable and `T` itself does not feature interior mutability. As such,
    // none of `T`'s padding bytes can be written to (with potentially
    // uninitialized data), and so providing this reference is safe. Providing a
    // mutable reference would, in turn, not be safe as that would mutably
    // expose `T`'s padding bytes.
    pub fn valid_ref<'a>(&'a self) -> &'a T {
        unsafe { self.assume_valid_ref() }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<T> OGCopy<*const T> {
    pub fn valid_ptr(self) -> *const T {
        unsafe { self.assume_valid() }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<T> OGCopy<*mut T> {
    pub fn valid_ptr(self) -> *mut T {
        unsafe { self.assume_valid() }
    }
}

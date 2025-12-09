// -*- fill-column: 80; -*-

use crate::maybe_valid::MaybeValid;

use super::og_copy::OGCopy;

// Flag settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_VALIDATION_CHECKS;

/// A value returned by foreign code.
///
/// This is either created from some initilized bytes that are copied
/// from the sandbox, or from a known-valid instance of type `T` which
/// may include uninitialized padding bytes.
#[derive(Debug)]
pub enum OGRet<T> {
    Initialized(MaybeValid<T>),
    Valid(T),
}

impl<T> OGRet<T> {
    pub fn from_valid_value(val: T) -> OGRet<T> {
        OGRet::Valid(val)
    }

    pub fn from_initialized_memory(maybe_valid: MaybeValid<T>) -> OGRet<T> {
        OGRet::Initialized(maybe_valid)
    }

    pub fn from_og_copy(og_copy: OGCopy<T>) -> OGRet<T> {
        OGRet::Initialized(og_copy.inner)
    }
}

impl<T: Copy> Clone for OGRet<T> {
    fn clone(&self) -> Self {
        match self {
            OGRet::Initialized(maybe_valid) => OGRet::Initialized(maybe_valid.clone()),
            OGRet::Valid(val) => OGRet::Valid(val.clone()),
        }
    }
}

impl<T: Copy> Copy for OGRet<T> {}

impl<T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout> OGRet<T> {
    pub fn validate(self) -> Result<T, Self> {
        match self {
            OGRet::Initialized(maybe_valid) => {
                if DISABLE_VALIDATION_CHECKS {
                    Ok(unsafe { maybe_valid.assume_valid() })
                } else if <T as zerocopy::TryFromBytes>::try_ref_from_bytes(maybe_valid.as_bytes())
                    .is_ok()
                {
                    Ok(unsafe { maybe_valid.assume_valid() })
                } else {
                    Err(OGRet::Initialized(maybe_valid))
                }
            }
            OGRet::Valid(val) => Ok(val),
        }
    }

    pub fn validate_ref<'a>(&'a self) -> Option<&'a T> {
        match self {
            OGRet::Initialized(maybe_valid) => {
                if DISABLE_VALIDATION_CHECKS {
                    Some(unsafe { maybe_valid.assume_valid_ref() })
                } else {
                    <T as zerocopy::TryFromBytes>::try_ref_from_bytes(maybe_valid.as_bytes()).ok()
                }
            }
            OGRet::Valid(val) => Some(val),
        }
    }
}

impl<T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout> OGRet<T> {
    pub fn valid(self) -> T {
        match self {
            OGRet::Initialized(maybe_valid) => unsafe { maybe_valid.assume_valid() },
            OGRet::Valid(val) => val,
        }
    }

    pub fn valid_ref<'a>(&'a self) -> &'a T {
        match self {
            OGRet::Initialized(maybe_valid) => unsafe { maybe_valid.assume_valid_ref() },
            OGRet::Valid(val) => val,
        }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<T> OGRet<*const T> {
    pub fn valid_ptr(self) -> *const T {
        match self {
            OGRet::Initialized(maybe_valid) => unsafe { maybe_valid.assume_valid() },
            OGRet::Valid(val) => val,
        }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<T> OGRet<*mut T> {
    pub fn valid_ptr(self) -> *mut T {
        match self {
            OGRet::Initialized(maybe_valid) => unsafe { maybe_valid.assume_valid() },
            OGRet::Valid(val) => val,
        }
    }
}

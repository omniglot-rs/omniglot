// -*- fill-column: 80; -*-

//! This module contains copies of `ptr::as_ref_unchecked` and
//! `ptr::as_mut_unchecked` from the standard library, as they are
//! behind a nightly feature flag currently.
//!
//! TODO: replace their occurences with stabilized versions once they become
//! available.

#[inline]
#[must_use]
pub const unsafe fn as_ref_unchecked<'a, T>(this: *const T) -> &'a T {
    // SAFETY: the caller must guarantee that `self` is valid for a reference
    unsafe { &*this }
}

#[allow(unused)]
#[inline]
#[must_use]
pub const unsafe fn as_mut_unchecked<'a, T>(this: *mut T) -> &'a mut T {
    // SAFETY: the caller must guarantee that `self` is valid for a reference
    unsafe { &mut *this }
}

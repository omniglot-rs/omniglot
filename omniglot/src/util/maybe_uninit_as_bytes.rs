//! This module contains copies of `MaybeUninit::as_bytes` and
//! `MaybeUninit::as_bytes_mut` from the standard library, as they are behind a
//! Nightly feature flag currently.
//!
//! TODO: replace their occurences with stabilized versions once they become
//! available.
//!
//! TODO: analyze safety implications of
//! <https://github.com/rust-lang/rust/issues/93092#issuecomment-2162623067>.
//! While we would be unlikely to use these methods for types with interior
//! mutability, we should ensure that this is enforced in our code, or deploy
//! any other fix that this issue.

use core::mem::{self, MaybeUninit};
use core::slice;

pub(crate) fn as_bytes<'a, T>(mu: &'a MaybeUninit<T>) -> &'a [MaybeUninit<u8>] {
    // SAFETY: MaybeUninit<u8> is always valid, even for padding bytes
    unsafe { slice::from_raw_parts(mu.as_ptr() as *const MaybeUninit<u8>, mem::size_of::<T>()) }
}

pub(crate) fn as_bytes_mut<'a, T>(mu: &'a mut MaybeUninit<T>) -> &'a mut [MaybeUninit<u8>] {
    // SAFETY: MaybeUninit<u8> is always valid, even for padding bytes
    unsafe {
        slice::from_raw_parts_mut(mu.as_mut_ptr() as *mut MaybeUninit<u8>, mem::size_of::<T>())
    }
}

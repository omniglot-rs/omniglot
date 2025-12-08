// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::Deref;

use crate::id::OGID;
use crate::maybe_valid::MaybeValid;

use super::og_ref::OGRef;
use super::og_slice::OGSlice;

pub struct OGVal<'alloc, 'access, ID: OGID, T: ?Sized> {
    pub(super) reference: &'access T,
    pub(super) id_imprint: ID::Imprint,
    pub(super) _alloc_lt: PhantomData<&'alloc T>,
}

impl<'alloc, 'access, ID: OGID, T> OGVal<'alloc, 'access, ID, T> {
    /// Return a raw pointer to this reference's pointee.
    pub fn as_ptr(&self) -> *const T {
        self.reference as *const T
    }
}

impl<'alloc, 'access, ID: OGID, T: Sized> OGVal<'alloc, 'access, ID, T> {
    /// Convert this validated reference into an immutable [`OGRef`] reference.
    // Variant for `T: Sized`:
    pub fn as_ref(&self) -> OGRef<'alloc, ID, T> {
        OGRef {
            reference: unsafe {
                &*(self.reference as *const _ as *const UnsafeCell<MaybeValid<T>>)
            },
            id_imprint: self.id_imprint,
        }
    }
}

impl<'alloc, 'access, ID: OGID, T> OGVal<'alloc, 'access, ID, [T]> {
    /// Convert this validated reference into an immutable [`OGSlice`] reference.
    // Variant for slices:
    pub fn as_ref(&self) -> OGSlice<'alloc, ID, T> {
        OGSlice {
            reference: unsafe {
                &*(self.reference as *const _ as *const [UnsafeCell<MaybeValid<T>>])
            },
            id_imprint: self.id_imprint,
        }
    }
}

impl<'alloc, 'access, ID: OGID, T> Clone for OGVal<'alloc, 'access, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, 'access, ID: OGID, T> Copy for OGVal<'alloc, 'access, ID, T> {}

impl<'alloc, 'access, ID: OGID, T: ?Sized> Deref for OGVal<'alloc, 'access, ID, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.reference
    }
}

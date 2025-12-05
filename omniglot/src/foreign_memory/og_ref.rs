use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::alloc_tracker::AllocTracker;
use crate::bit_pattern_validate::BitPatternValidate;
use crate::id::OGID;
use crate::markers::AccessScope;

use super::og_copy::OGCopy;
use super::og_slice::OGSlice;
use super::og_val::OGVal;
use super::{DISABLE_UPGRADE_CHECKS, DISABLE_VALIDATION_CHECKS};

use crate::util::as_ref_unchecked::as_ref_unchecked;

// A reference which is validated to be well-aligned and contained in
// (im)mutably-accessible memory. It may still be mutable by foreign code, and
// hence we assume interior mutability here:
pub struct OGRef<'alloc, ID: OGID, T> {
    pub(crate) r: &'alloc UnsafeCell<MaybeUninit<T>>,
    id_imprint: ID::Imprint,
}

impl<'alloc, ID: OGID, T> Clone for OGRef<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGRef<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T> OGRef<'alloc, ID, T> {
    pub(crate) unsafe fn new(
        r: &'alloc UnsafeCell<MaybeUninit<T>>,
        id_imprint: ID::Imprint,
    ) -> Self {
        OGRef { r, id_imprint }
    }

    /// TODO: document safety impliciations
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *const T,
        id_imprint: ID::Imprint,
    ) -> OGRef<'alloc, ID, T> {
        OGRef {
            r: unsafe { as_ref_unchecked(ptr as *const UnsafeCell<MaybeUninit<T>>) },
            id_imprint,
        }
    }

    pub fn upgrade_from_ptr<R: AllocTracker>(
        ptr: *const T,
        alloc_scope: super::UpgradeAllocScopeTy<'_, 'alloc, R, ID>,
    ) -> Option<OGRef<'alloc, ID, T>> {
        if DISABLE_UPGRADE_CHECKS {
            Some(unsafe { Self::upgrade_from_ptr_unchecked(ptr, alloc_scope.id_imprint()) })
        } else {
            if ptr.is_aligned()
                && alloc_scope
                    .tracker()
                    .is_valid(ptr as *const (), core::mem::size_of::<T>())
            {
                Some(unsafe { Self::upgrade_from_ptr_unchecked(ptr, alloc_scope.id_imprint()) })
            } else {
                None
            }
        }
    }

    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        // # Safety
        //
        // TODO
        unsafe { OGVal::new(&*(self.r as *const _ as *const T), self.id_imprint) }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.r as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T
    }

    pub fn copy<'access>(&self, access_scope: &'access AccessScope<ID>) -> OGCopy<T> {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        // Safety: we're overwriting the uninit immediately with known values,
        // and hence never creating a non-MaybeUninit reference to uninitialized
        // memory:
        let mut copy = unsafe { OGCopy::<T>::uninit() };
        copy.update_from_ref(*self, access_scope);
        copy
    }

    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }

    pub unsafe fn sub_ref_unchecked<U>(self, byte_offset: usize) -> OGRef<'alloc, ID, U> {
        OGRef {
            r: unsafe {
                &*((self.r as *const UnsafeCell<MaybeUninit<T>>).byte_add(byte_offset)
                    as *const UnsafeCell<MaybeUninit<U>>)
            },
            id_imprint: self.id_imprint,
        }
    }
}

impl<'alloc, ID: OGID, T: BitPatternValidate> OGRef<'alloc, ID, T> {
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, T>> {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid(access_scope) })
        } else {
            if unsafe {
                <T as BitPatternValidate>::validate(
                    self.r as *const UnsafeCell<MaybeUninit<T>> as *const T,
                )
            } {
                Some(unsafe { self.assume_valid(access_scope) })
            } else {
                None
            }
        }
    }
}

impl<'alloc, const N: usize, ID: OGID, T> OGRef<'alloc, ID, [T; N]> {
    pub fn len(&self) -> usize {
        N
    }

    pub unsafe fn get_unchecked(&self, idx: usize) -> OGRef<'alloc, ID, T> {
        OGRef {
            // # Safety
            //
            // TODO
            r: unsafe {
                &*((self.r as *const UnsafeCell<MaybeUninit<[T; N]>>
                    as *const UnsafeCell<MaybeUninit<T>>)
                    .add(idx))
            },
            id_imprint: self.id_imprint,
        }
    }

    pub fn get(&self, idx: usize) -> Option<OGRef<'alloc, ID, T>> {
        if idx < N {
            Some(unsafe { self.get_unchecked(idx) })
        } else {
            None
        }
    }

    pub fn iter(&self) -> OGRefIter<'alloc, ID, N, T> {
        OGRefIter {
            inner: self.clone(),
            idx: 0,
        }
    }

    pub fn as_slice(&self) -> OGSlice<'alloc, ID, T> {
        unsafe {
            OGSlice::new(
                core::slice::from_raw_parts(
                    self.r as *const _ as *const UnsafeCell<MaybeUninit<T>>,
                    N,
                ),
                self.id_imprint,
            )
        }
    }
}

pub struct OGRefIter<'alloc, ID: OGID, const N: usize, T> {
    inner: OGRef<'alloc, ID, [T; N]>,
    idx: usize,
}

impl<'alloc, ID: OGID, const N: usize, T> core::iter::Iterator
    for OGRefIter<'alloc, ID, N, T>
{
    type Item = OGRef<'alloc, ID, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.inner.get(self.idx) {
            // Prevent wraparound by calling .iter() a bunch.
            self.idx += 1;
            Some(item)
        } else {
            None
        }
    }
}

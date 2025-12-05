use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::alloc_tracker::AllocTracker;
use crate::bit_pattern_validate::BitPatternValidate;
use crate::id::OGID;
use crate::markers::AccessScope;

use super::og_ref::OGRef;
use super::og_slice_val::OGSliceVal;
use super::og_val::OGVal;
use super::{DISABLE_UPGRADE_CHECKS, DISABLE_VALIDATION_CHECKS};

pub struct OGSlice<'alloc, ID: OGID, T: 'static> {
    // The length of this slice is encoded in the reference itself (fat
    // pointer), and not located in / accessible to foreign memory:
    pub r: &'alloc [UnsafeCell<MaybeUninit<T>>],
    id_imprint: ID::Imprint,
}

impl<'alloc, ID: OGID, T: 'static> Clone for OGSlice<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T: 'static> Copy for OGSlice<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T: 'static> OGSlice<'alloc, ID, T> {
    pub(crate) unsafe fn new(
        r: &'alloc [UnsafeCell<MaybeUninit<T>>],
        id_imprint: ID::Imprint,
    ) -> Self {
        OGSlice { r, id_imprint }
    }

    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *const T,
        length: usize,
        id_imprint: ID::Imprint,
    ) -> OGSlice<'alloc, ID, T> {
        // TODO: check soudness. Is it always safe to have a [MaybeUninit<T>],
        // when it would be safe to have a MaybeUninit<[T]>, for which the
        // length is valid and initialized?
        OGSlice {
            // # Safety
            //
            // TODO
            r: unsafe {
                core::slice::from_raw_parts(ptr as *const UnsafeCell<MaybeUninit<T>>, length)
            },
            id_imprint,
        }
    }

    pub fn upgrade_from_ptr<R: AllocTracker>(
        ptr: *const T,
        length: usize,
        alloc_scope: super::UpgradeAllocScopeTy<'_, 'alloc, R, ID>,
    ) -> Option<OGSlice<'alloc, ID, T>> {
        if DISABLE_UPGRADE_CHECKS {
            Some(unsafe { Self::upgrade_from_ptr_unchecked(ptr, length, alloc_scope.id_imprint()) })
        } else {
            // As per Rust reference, "An array of [T; N] has a size of
            // size_of::<T>() * N and the same alignment of T", and, "Slices
            // have the same layout as the section of the array they slice", so
            // checking for alignment of T is sufficient.
            //
            // Furthermore, for `std::mem::size_of`, the function documentation
            // reads:
            //
            //     More specifically, this is the offset in bytes between
            //     successive elements in an array with that item type including
            //     alignment padding. Thus, for any type T and length n, [T; n]
            //     has a size of n * size_of::<T>().
            //
            // Hence we perform the check for exactly this expression:
            if ptr.is_aligned()
                && alloc_scope
                    .tracker()
                    .is_valid(ptr as *const (), length * core::mem::size_of::<T>())
            {
                Some(unsafe {
                    Self::upgrade_from_ptr_unchecked(ptr, length, alloc_scope.id_imprint())
                })
            } else {
                None
            }
        }
    }

    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }

    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGSliceVal<'alloc, 'access, ID, T> {
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
        unsafe {
            OGSliceVal::new(
                core::mem::transmute::<&[UnsafeCell<MaybeUninit<T>>], &[T]>(&self.r),
                self.id_imprint,
            )
        }
    }

    pub fn as_ptr(&self) -> *const T {
        self.r as *const [UnsafeCell<MaybeUninit<T>>] as *const T
    }

    pub fn len(&self) -> usize {
        self.r.len()
    }

    pub fn get(&self, idx: usize) -> Option<OGRef<'alloc, ID, T>> {
        self.r
            .get(idx)
            .map(|elem| unsafe { OGRef::new(elem, self.id_imprint) })
    }

    pub fn iter(&self) -> OGSliceIter<'alloc, ID, T> {
        OGSliceIter {
            inner: *self,
            idx: 0,
        }
    }
}

impl<'alloc, ID: OGID, T: BitPatternValidate + 'static> OGSlice<'alloc, ID, T> {
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGSliceVal<'alloc, 'access, ID, T>> {
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
            if self
                .r
                .iter()
                .all(|elem: &UnsafeCell<MaybeUninit<T>>| unsafe {
                    <T as BitPatternValidate>::validate(
                        elem as *const UnsafeCell<MaybeUninit<T>> as *const T,
                    )
                })
            {
                Some(unsafe { self.assume_valid(access_scope) })
            } else {
                None
            }
        }
    }
}

impl<'alloc, ID: OGID> OGSlice<'alloc, ID, u8> {
    pub fn validate_as_str<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, str>> {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe {
                OGVal::new(
                    core::str::from_utf8_unchecked(&*(self.r as *const _ as *const [u8])),
                    self.id_imprint,
                )
            })
        } else {
            // We rely on the fact that u8s are unconditionally valid, and we
            // hold onto an AccessScope here
            core::str::from_utf8(unsafe { &*(self.r as *const _ as *const [u8]) })
                .ok()
                .map(|s| unsafe { OGVal::new(s, self.id_imprint) })
        }
    }
}

pub struct OGSliceIter<'alloc, ID: OGID, T: 'static> {
    inner: OGSlice<'alloc, ID, T>,
    idx: usize,
}

impl<'alloc, ID: OGID, T: 'static> core::iter::Iterator for OGSliceIter<'alloc, ID, T> {
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

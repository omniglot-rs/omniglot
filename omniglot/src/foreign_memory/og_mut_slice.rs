use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::alloc_tracker::AllocTracker;
use crate::bit_pattern_validate::BitPatternValidate;
use crate::id::OGID;
use crate::markers::AccessScope;

use super::og_slice::OGSlice;
use super::og_slice_val::OGSliceVal;
use super::{DISABLE_UPGRADE_CHECKS, DISABLE_VALIDATION_CHECKS};

pub struct OGMutSlice<'alloc, ID: OGID, T: 'static> {
    // The length of this slice is encoded in the reference itself (fat
    // pointer), and not located in / accessible to foreign memory:
    pub r: &'alloc [UnsafeCell<MaybeUninit<T>>],
    id_imprint: ID::Imprint,
}

impl<'alloc, ID: OGID, T: 'static> OGMutSlice<'alloc, ID, T> {
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *mut T,
        length: usize,
        id_imprint: ID::Imprint,
    ) -> OGMutSlice<'alloc, ID, T> {
        // TODO: check soudness. Is it always safe to have a [MaybeUninit<T>],
        // when it would be safe to have a MaybeUninit<[T]>, for which the
        // length is valid and initialized?
        OGMutSlice {
            // # Safety
            //
            // TODO
            r: unsafe {
                core::slice::from_raw_parts(
                    ptr as *mut _ as *mut UnsafeCell<MaybeUninit<T>>,
                    length,
                )
            },
            id_imprint,
        }
    }

    pub fn upgrade_from_ptr<R: AllocTracker>(
        ptr: *mut T,
        length: usize,
        alloc_scope: super::UpgradeAllocScopeTy<'_, 'alloc, R, ID>,
    ) -> Option<OGMutSlice<'alloc, ID, T>> {
        if DISABLE_UPGRADE_CHECKS {
            Some(unsafe { Self::upgrade_from_ptr_unchecked(ptr, length, alloc_scope.id_imprint()) })
        } else {
            // As per Rust reference, "An array of [T; N] has a size of
            // size_of::<T>() * N and the same alignment of T", and, "Slices
            // have the same layout as the section of the array they slice", so
            // checking for alignment of T is sufficient.
            //
            // Furthermore, for `std::mem::size_of`, the function documentation reads:
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
                    .is_valid_mut(ptr as *mut (), length * core::mem::size_of::<T>())
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

    pub fn as_ptr(&self) -> *mut T {
        self.r as *const _ as *mut [UnsafeCell<MaybeUninit<T>>] as *mut T
    }

    pub fn as_immut(&self) -> OGSlice<'alloc, ID, T> {
        unsafe { OGSlice::new(self.r, self.id_imprint) }
    }

    pub fn len(&self) -> usize {
        self.r.len()
    }

    pub fn write_from_iter<'access, I: Iterator<Item = T>>(
        &self,
        src: I,
        access_scope: &'access AccessScope<ID>,
    ) -> OGSliceVal<'alloc, 'access, ID, T> {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        // Safety: taking &mut AccessScope<ID> ensures that no other accessible
        // references into foreign memory exist, and that no foreign code is
        // accessing this memory. The existance of this type ensures that this
        // memory is mutably accessible and well-aligned.
        let mut count = 0;
        self.r.iter().zip(src).for_each(|(dst, val)| {
            (unsafe { &mut *dst.get() }).write(val);
            count += 1;
        });
        assert!(count == self.r.len());

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that the reference must have a valid value right
        // now, based on the knowledge that every element of `src` was a valid
        // instance of T, and we copied self.r.len() elements from `src`:
        unsafe { self.assume_valid(access_scope) }
    }
}

impl<'alloc, ID: OGID, T: BitPatternValidate + 'static> OGMutSlice<'alloc, ID, T> {
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

impl<'alloc, ID: OGID, T: Copy + 'static> OGMutSlice<'alloc, ID, T> {
    pub fn copy_from_slice<'access>(
        &self,
        src: &[T],
        access_scope: &'access AccessScope<ID>,
    ) -> OGSliceVal<'alloc, 'access, ID, T> {
        self.write_from_iter(src.iter().copied(), access_scope)
    }
}

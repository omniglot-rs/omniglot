use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use crate::alloc_tracker::AllocTracker;
use crate::bit_pattern_validate::BitPatternValidate;
use crate::id::OGID;
use crate::markers::AccessScope;
use crate::util::as_ref_unchecked::as_ref_unchecked;
use crate::util::maybe_uninit_as_bytes;

use super::og_copy::OGCopy;
use super::og_ref::OGRef;
use super::og_val::OGVal;
use super::{DISABLE_UPGRADE_CHECKS, DISABLE_VALIDATION_CHECKS};

// A reference which is validated to be well-aligned and contained in
// mutably-accessible memory.
pub struct OGMutRef<'alloc, ID: OGID, T> {
    pub(crate) r: &'alloc UnsafeCell<MaybeUninit<T>>,
    id_imprint: ID::Imprint,
}

impl<'alloc, ID: OGID, T> OGMutRef<'alloc, ID, T> {
    pub(crate) unsafe fn new(
        r: &'alloc UnsafeCell<MaybeUninit<T>>,
        id_imprint: ID::Imprint,
    ) -> Self {
        OGMutRef { r, id_imprint }
    }

    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }
}

impl<'alloc, ID: OGID, T> Clone for OGMutRef<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGMutRef<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T: BitPatternValidate> OGMutRef<'alloc, ID, T> {
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

impl<'alloc, ID: OGID, T> OGMutRef<'alloc, ID, T> {
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *mut T,
        id_imprint: ID::Imprint,
    ) -> OGMutRef<'alloc, ID, T> {
        OGMutRef {
            r: unsafe { as_ref_unchecked(ptr as *mut UnsafeCell<MaybeUninit<T>> as *const _) },
            id_imprint,
        }
    }

    pub fn upgrade_from_ptr<R: AllocTracker>(
        ptr: *mut T,
        alloc_scope: super::UpgradeAllocScopeTy<'_, 'alloc, R, ID>,
    ) -> Option<OGMutRef<'alloc, ID, T>> {
        if DISABLE_UPGRADE_CHECKS {
            Some(unsafe { Self::upgrade_from_ptr_unchecked(ptr, alloc_scope.id_imprint()) })
        } else {
            if ptr.is_aligned()
                && alloc_scope
                    .tracker()
                    .is_valid_mut(ptr as *mut (), core::mem::size_of::<T>())
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

    pub unsafe fn sub_ref_unchecked<U>(
        self,
        byte_offset: usize,
    ) -> OGMutRef<'alloc, ID, U> {
        OGMutRef {
            r: unsafe {
                &*((self.r as *const UnsafeCell<MaybeUninit<T>>).byte_add(byte_offset)
                    as *const UnsafeCell<MaybeUninit<U>>)
            },
            id_imprint: self.id_imprint,
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.r as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T
    }

    pub fn as_immut(&self) -> OGRef<'alloc, ID, T> {
        unsafe { OGRef::new(self.r, self.id_imprint) }
    }

    pub fn write<'access>(
        &self,
        val: T,
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
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
        (unsafe { &mut *self.r.get() }).write(val);

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that the reference must have a valid value right
        // now, based on the knowledge that `val` was a valid instance of T:
        unsafe { self.assume_valid(access_scope) }
    }

    pub fn write_copy<'access>(
        &self,
        copy: &OGCopy<T>,
        access_scope: &'access mut AccessScope<ID>,
    ) {
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
        maybe_uninit_as_bytes::as_bytes_mut(unsafe { &mut *self.r.get() })
            .copy_from_slice(maybe_uninit_as_bytes::as_bytes(&copy.0))
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
        copy.update_from_mut_ref(*self, access_scope);
        copy
    }
}

impl<'alloc, ID: OGID, T: Copy> OGMutRef<'alloc, ID, T> {
    pub fn write_ref<'access>(
        &self,
        val: &T,
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
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
        //
        // TODO: need to ensure that this does not create any imtermediate full
        // copies on the stack. It should copy directly from the reference
        // (effectively a memcpy):
        (unsafe { &mut *self.r.get() }).write(*val);

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that the reference must have a valid value right
        // now, based on the knowledge that `val` was a valid instance of T:
        unsafe { self.assume_valid(access_scope) }
    }
}

impl<'alloc, const N: usize, ID: OGID, T> OGMutRef<'alloc, ID, [T; N]> {
    pub fn len(&self) -> usize {
        N
    }

    pub unsafe fn get_unchecked(&self, idx: usize) -> OGMutRef<'alloc, ID, T> {
        OGMutRef {
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

    pub fn get(&self, idx: usize) -> Option<OGMutRef<'alloc, ID, T>> {
        if idx < N {
            Some(unsafe { self.get_unchecked(idx) })
        } else {
            None
        }
    }

    pub fn iter(&self) -> OGMutRefIter<'alloc, ID, N, T> {
        OGMutRefIter {
            inner: self.clone(),
            idx: 0,
        }
    }
}

impl<'alloc, const N: usize, ID: OGID, T: Copy> OGMutRef<'alloc, ID, [T; N]> {
    pub fn copy_from_slice<'access>(&self, src: &[T], access_scope: &'access mut AccessScope<ID>) {
        if self.id_imprint != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                self.id_imprint,
                access_scope.id_imprint()
            );
        }

        if src.len() != N {
            // Meaningful error message, and optimize panic with a cold
            // function:
            panic!(
                "Called OGMutRef::<[_; {}]>::copy_from_slice with a slice of length {}",
                N,
                src.len()
            );
        }

        self.iter().zip(src.iter()).for_each(|(dst, src)| {
            dst.write(*src, access_scope);
        })
    }
}

pub struct OGMutRefIter<'alloc, ID: OGID, const N: usize, T> {
    inner: OGMutRef<'alloc, ID, [T; N]>,
    idx: usize,
}

impl<'alloc, ID: OGID, const N: usize, T> core::iter::Iterator
    for OGMutRefIter<'alloc, ID, N, T>
{
    type Item = OGMutRef<'alloc, ID, T>;

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

/// Get an `OGMutRef` reference to a member of a struct wrapped in an
/// `OGMutRef`
///
/// TODO: this is a workaround until we derive OGType for nested types
/// in bindgen and provide safe methods for accessing struct members.
///
/// Usage example:
/// ```
/// use omniglot::foreign_memory::og_mut_ref::OGMutRef;
/// use omniglot::id::OGID;
/// use omniglot::ogmutref_get_field;
///
/// struct TestStruct {
///     test_member: u32,
/// }
///
/// fn test_fn<'alloc, ID: OGID>(test_struct: OGMutRef<'alloc, ID, TestStruct>) {
///     let _test_member_ref: OGMutRef<'alloc, ID, u32> =
///         unsafe { ogmutref_get_field!(TestStruct, u32, test_struct, test_member) };
/// }
/// ```
#[macro_export]
macro_rules! ogmutref_get_field {
    ($outer_type:ty, $inner_type:ty, $outer_ref:expr, $member:ident) => {{
        unsafe fn ogmutref_get_field_helper<'alloc, ID: $crate::id::OGID>(
            outer: $crate::foreign_memory::og_mut_ref::OGMutRef<'alloc, ID, $outer_type>,
        ) -> $crate::foreign_memory::og_mut_ref::OGMutRef<'alloc, ID, $inner_type> {
            let outer_ptr: *mut () = outer.as_ptr().cast::<()>().into();
            let inner_ptr: *mut $inner_type = unsafe {
                outer_ptr.byte_offset(::core::mem::offset_of!($outer_type, $member,) as isize)
                    as *mut $inner_type
            };
            unsafe {
                $crate::foreign_memory::og_mut_ref::OGMutRef::upgrade_from_ptr_unchecked(
                    inner_ptr,
                    outer.id_imprint(),
                )
            }
        }

        ogmutref_get_field_helper($outer_ref)
    }};
}

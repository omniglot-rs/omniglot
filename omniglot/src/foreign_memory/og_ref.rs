// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;
use core::marker::PhantomData;

use crate::alloc_tracker::AllocTracker;
use crate::id::OGID;
use crate::markers::AccessScope;
use crate::maybe_valid::MaybeValid;

use super::og_copy::OGCopy;
use super::og_slice::OGSlice;
use super::og_val::OGVal;

// Flags settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_UPGRADE_CHECKS;
use super::DISABLE_VALIDATION_CHECKS;

use crate::util::as_ref_unchecked::as_ref_unchecked;

/// A reference into a region of allocated and readable foreign memory with size
/// and alignment of type `T`.
///
/// This reference is created within and bound to an
/// [`AllocScope`](crate::markers::AllocScope) valid for lifetime `'alloc`. This
/// ensures that the memory backing this reference is initialized and allocated,
/// for as long as this reference type exists.
///
/// This reference may or may not contain a valid instance of type `T`. It can
/// be mutably aliased with other references into foreign memory, and its
/// underlying memory may be modified whenever foreign code runs.
pub struct OGRef<'alloc, ID: OGID, T> {
    pub(super) reference: &'alloc UnsafeCell<MaybeValid<T>>,
    pub(super) id_imprint: ID::Imprint,
}

// `OGRef` is safe to clone and copy, as its copies will carry the same `'alloc`
// lifetime constraints.
impl<'alloc, ID: OGID, T> Clone for OGRef<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGRef<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T> OGRef<'alloc, ID, T> {
    /// Create an `OGRef` from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function requires that the supplied pointer points into an
    /// allocated, initialized, and readable region of memory belonging to a
    /// foreign library instance, of size and alignment following that of type
    /// `T`. This memory must remain allocated, intialized, and readable for the
    /// duration of `'alloc`. The supplied `ID::Imprint` must originate from the
    /// [`OGID`] instance used to construct the
    /// [`OGRuntime`](crate::rt::OGRuntime) managing the memory belonging to
    /// this foreign libray instance.
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *const T,
        id_imprint: ID::Imprint,
    ) -> OGRef<'alloc, ID, T> {
        OGRef {
            reference: unsafe { as_ref_unchecked(ptr as *const UnsafeCell<MaybeValid<T>>) },
            id_imprint,
        }
    }

    /// Create an `OGRef` from a raw pointer within an
    /// [`AllocScope`](crate::markers::AllocScope).
    ///
    /// This function checks whether this pointer is well-aligned for type `T`
    /// and wholly located in a readable memory region of the foreign
    /// library. If these conditions are not true (and the unsound
    /// `disable_upgrade_checks` crate feature is not enabled), the function
    /// will return. Otherwise, it will return a reference over this pointer,
    /// bound to the supplied `AllocScope`.
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

    /// Return a raw pointer to this reference's pointee.
    pub fn as_ptr(&self) -> *const T {
        self.reference as *const UnsafeCell<MaybeValid<T>> as *const T
    }

    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this [`OGRef`].
    ///
    /// This function takes a shared `AccessScope` reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory for the duration that the returned reference
    /// exists.
    ///
    /// # Safety
    ///
    /// This function requires that the memory behind this reference, at the
    /// time of calling, can be safely transmuted into a readable instance of
    /// type `T`, and that `T` does not feature interior mutability.
    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        unsafe {
            super::check_access_scope_imprint(self.id_imprint, access_scope);

            OGVal {
                reference: &*(self.reference as *const _ as *const T),
                id_imprint: self.id_imprint,
                _alloc_lt: PhantomData,
            }
        }
    }

    /// Create an owned copy of this reference's underlying memory.
    ///
    /// This performs a byte-wise copy of this reference's pointee into an owned
    /// [`OGCopy`]. This method does not perform any validation.
    ///
    /// This function takes a shared `AccessScope` reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory over the duration of the copy operation.
    pub fn copy<'access>(&self, access_scope: &'access AccessScope<ID>) -> OGCopy<T> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        // Safety: taking &AccessScope<ID> and checking its imprint against this
        // reference's internal copy ensures no host or foreign code is
        // modifying this memory concurrently. The existence of `OGRef` ensures
        // that this memory is readable and well-aligned.
        let self_maybevalid = unsafe { &*self.reference.get() };

        OGCopy::from_bytes(self_maybevalid.as_bytes())
    }

    /// Create a sub-reference to another value of type `U` at a given offset
    /// within this reference.
    ///
    /// This is identical to [`Self::sub_ref`], except that it does not perform
    /// any checks for whether the new reference would be contained within the
    /// original one, or well-aligned.
    ///
    /// # Safety
    ///
    /// Callers must ensure that the new reference is fully contained within
    /// `self` (i.e., `byte_offset + size_of::<U>() <= size_of::<T>()`, and that
    /// the resulting reference to a value of type `U` is well-aligned.
    pub unsafe fn sub_ref_unchecked<U>(self, byte_offset: usize) -> OGRef<'alloc, ID, U> {
        OGRef {
            reference: unsafe {
                &*((self.reference as *const UnsafeCell<MaybeValid<T>>).byte_add(byte_offset)
                    as *const UnsafeCell<MaybeValid<U>>)
            },
            id_imprint: self.id_imprint,
        }
    }

    /// Create a sub-reference to another value of type `U` at a given offset
    /// within this reference.
    ///
    /// This creates another reference to a value of type `U`, located at
    /// `byte_offset` within this reference's pointee of type `T`. This method
    /// checks that the new reference is well-aligned, and is contained within
    /// the original value of type `T`.
    ///
    /// This checks are performed statically, without dynamic inspection of this
    /// `OGRef`'s pointer value. Therefore, this method cannot be used to create
    /// references for types whose alignment is greater than that of type
    /// `T`. For instance, if `self` points to address `24` and `T` is 32 bytes
    /// in size, then a 16 byte size value with 16 byte alignment at offset `8`
    /// would happen to be well-aligned, but this method will return `None` (as
    /// it would not be well-aligned for _every_ `T`).
    pub fn sub_ref<U>(self, byte_offset: usize) -> Option<OGRef<'alloc, ID, U>> {
        if super::sub_ref_check::<T, U>(byte_offset) {
            Some(unsafe { self.sub_ref_unchecked(byte_offset) })
        } else {
            None
        }
    }
}

impl<'alloc, ID: OGID, T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGRef<'alloc, ID, T>
{
    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this [`OGRef`].
    ///
    /// This function takes a shared [`AccessScope`] reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory for the duration that the returned reference
    /// exists.
    ///
    /// It then checks whether the current contents of this memory would
    /// constitute a valid instance of type `T`, as determined by
    /// [`zerocopy::TryFromBytes`]. If the reference does not point to a valid
    /// instance of type `T` (and the unsound `disable_validation_checks` crate
    /// feature is not enabled), it returns None. Otherwise, it returns a
    /// dereferencable reference, bound to the supplied [`AccessScope`].
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, T>> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid(access_scope) })
        } else {
            if <T as zerocopy::TryFromBytes>::try_ref_from_bytes(
                unsafe { &*self.reference.get() }.as_bytes(),
            )
            .is_ok()
            {
                Some(unsafe { self.assume_valid(access_scope) })
            } else {
                None
            }
        }
    }
}

impl<'alloc, ID: OGID, T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGRef<'alloc, ID, T>
{
    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this [`OGRef`].
    ///
    /// This function takes a shared [`AccessScope`] reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory for the duration that the returned reference
    /// exists.
    ///
    /// Because of the trait bound of [`zerocopy::FromBytes`], we know that
    /// every bit pattern underneath this reference is a valid value for type
    /// `T`. As such, this conversion is infallible.
    pub fn valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        // Every bit-pattern of this reference is a valid instance of T, so we
        // can safely call `assume_valid`. This function also checks the access
        // scope imprint:
        unsafe { self.assume_valid(access_scope) }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<'alloc, ID: OGID, T> OGRef<'alloc, ID, *const T> {
    pub fn valid_ptr<'access>(
        self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, *const T> {
        unsafe { self.assume_valid(access_scope) }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<'alloc, ID: OGID, T> OGRef<'alloc, ID, *mut T> {
    pub fn valid_ptr<'access>(
        self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, *mut T> {
        unsafe { self.assume_valid(access_scope) }
    }
}

impl<'alloc, const N: usize, ID: OGID, T> OGRef<'alloc, ID, [T; N]> {
    /// Obtain a slice reference to this array in foreign memory.
    ///
    /// The returned slice reference is bound to the same allocation scope as
    /// the original array reference.
    pub fn as_slice(&self) -> OGSlice<'alloc, ID, T> {
        OGSlice {
            reference: unsafe {
                core::slice::from_raw_parts(
                    self.reference as *const UnsafeCell<MaybeValid<[T; N]>>
                        as *const UnsafeCell<MaybeValid<T>>,
                    N,
                )
            },
            id_imprint: self.id_imprint,
        }
    }
}

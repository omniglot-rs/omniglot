// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;

use crate::alloc_tracker::AllocTracker;
use crate::id::OGID;
use crate::markers::AccessScope;
use crate::maybe_valid::MaybeValid;
use crate::util::as_ref_unchecked::as_ref_unchecked;

use super::og_copy::OGCopy;
use super::og_mut_slice::OGMutSlice;
use super::og_ref::OGRef;
use super::og_val::OGVal;

// Flags settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_UPGRADE_CHECKS;

/// A reference into a region of allocated, readable, and writeable foreign
/// memory with size and alignment of type `T`.
///
/// This reference is created within and bound to an
/// [`AllocScope`](crate::markers::AllocScope) valid for lifetime `'alloc`. This
/// ensures that the memory backing this reference is initialized and allocated,
/// for as long as this reference type exists.
///
/// This reference may or may not contain a valid instance of type `T`. It can
/// be mutably aliased with other references into foreign memory, and its
/// underlying memory may be modified whenever foreign code runs.
pub struct OGMutRef<'alloc, ID: OGID, T> {
    pub(super) reference: &'alloc UnsafeCell<MaybeValid<T>>,
    pub(super) id_imprint: ID::Imprint,
}

// `OGMutRef` is safe to clone and copy, as its copies will carry the same
// `'alloc` lifetime constraints.
impl<'alloc, ID: OGID, T> Clone for OGMutRef<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGMutRef<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T> OGMutRef<'alloc, ID, T> {
    /// Create an `OGMutRef` from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function requires that the supplied pointer points into an
    /// allocated, initialized, readable, and writeable region of memory
    /// belonging to a foreign library instance, of size and alignment following
    /// that of type `T`. This memory must remain allocated, intialized,
    /// readable, and writeable for the duration of `'alloc`. The supplied
    /// `ID::Imprint` must originate from the [`OGID`] instance used to
    /// construct the [`OGRuntime`](crate::rt::OGRuntime) managing the memory
    /// belonging to this foreign libray instance.
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *mut T,
        id_imprint: ID::Imprint,
    ) -> OGMutRef<'alloc, ID, T> {
        OGMutRef {
            reference: unsafe {
                as_ref_unchecked(ptr as *mut UnsafeCell<MaybeValid<T>> as *const _)
            },
            id_imprint,
        }
    }

    /// Create an `OGMutRef` from a raw pointer within an
    /// [`AllocScope`](crate::markers::AllocScope).
    ///
    /// This function checks whether this pointer is well-aligned for type `T`
    /// and wholly located in a readable and writeable memory region of the
    /// foreign library. If these conditions are not true (and the unsound
    /// `disable_upgrade_checks` crate feature is not enabled), the function
    /// will return. Otherwise, it will return a reference over this pointer,
    /// bound to the supplied `AllocScope`.
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

    /// Return a raw pointer to this reference's pointee.
    pub fn as_ptr(&self) -> *mut T {
        self.reference as *const _ as *mut UnsafeCell<MaybeValid<T>> as *mut T
    }

    /// Convert this mutable reference into an immutable [`OGRef`] reference.
    pub fn as_immut(&self) -> OGRef<'alloc, ID, T> {
        OGRef {
            reference: self.reference,
            id_imprint: self.id_imprint,
        }
    }

    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this [`OGMutRef`].
    ///
    /// This function has the same semantics as [`OGRef::assume_valid`], please
    /// refer to its documentation.
    // This is using `as_immut` and OGRef's `assume_valid`, to reduce the amount
    // of unsafe code we need to maintain and check:
    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        // OGRef's `assume_valid` will perform an ID imprint check.

        let og_ref = self.as_immut();

        // # Safety
        //
        // See method doc comment, identical to safety requirements of
        // [`OGRef::assume_valid`].
        unsafe { og_ref.assume_valid(access_scope) }
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
    pub unsafe fn sub_ref_unchecked<U>(self, byte_offset: usize) -> OGMutRef<'alloc, ID, U> {
        OGMutRef {
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
    pub fn sub_ref<U>(self, byte_offset: usize) -> Option<OGMutRef<'alloc, ID, U>> {
        if super::sub_ref_check::<T, U>(byte_offset) {
            Some(unsafe { self.sub_ref_unchecked(byte_offset) })
        } else {
            None
        }
    }

    /// Write a value of type `T` to this reference.
    ///
    /// This function requires a unique (mutable) reference to the
    /// [`AccessScope`] marker. This ensures it has unique access to foreign
    /// memory, with no concurrent reads or writes by the host or foreign
    /// library. Calling this function closes the current, and opens a new
    /// access scope.
    ///
    /// It returns a validated reference ([`OGVal`]) to the written
    /// value. However, this reference is bound to a _unique_ borrow of the
    /// [`AccessScope`] marker. This means that, while this reference exists,
    /// neither host nor foreign code can create other references into foreign
    /// memory. To have multiple, concurrent readable references into foreign
    /// memory, drop the return value and use `validate` to re-obtain it.
    pub fn write<'access>(
        &self,
        val: T,
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        // # Safety
        //
        // Taking &mut AccessScope<ID> ensures that no other accessible host
        // references into foreign memory exist, and that no foreign code is
        // accessing this memory. The existence of this [`OGMutRef`] type
        // ensures that this memory is mutably accessible and well-aligned.
        (unsafe { &mut *self.reference.get() }).write(val);

        // # Safety
        //
        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that the reference must have a valid value right
        // now, based on the knowledge that `val` was a valid instance of T:
        unsafe { self.assume_valid(access_scope) }
    }

    /// Write the contents of an [`OGCopy`] to this reference.
    ///
    /// This function requires a unique (mutable) reference to the
    /// [`AccessScope`] marker. This ensures it has unique access to foreign
    /// memory, with no concurrent reads or writes by the host or foreign
    /// library. Calling this function closes the current, and opens a new
    /// access scope.
    ///
    /// In contrast to [`OGMutRef::write`], it does not return a validated
    /// reference. This is because an [`OGCopy`] may hold an invalid value for
    /// type `T`.
    pub fn write_copy<'access>(
        &self,
        copy: &OGCopy<T>,
        access_scope: &'access mut AccessScope<ID>,
    ) {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        // Safety: taking &mut AccessScope<ID> ensures that no other accessible
        // references into foreign memory exist, and that foreign code cannot
        // access this memory concurrenty. The existence of this type ensures
        // that this memory is mutably accessible and well-aligned.
        MaybeValid::as_bytes_mut(unsafe { &mut *self.reference.get() })
            .copy_from_slice(copy.inner.as_bytes())
    }

    /// Create an owned copy of this reference's underlying memory.
    ///
    /// This function has the same semantics as [`OGRef::copy`], please refer to
    /// its documentation.
    pub fn copy<'access>(&self, access_scope: &'access AccessScope<ID>) -> OGCopy<T> {
        // OGRef's `copy` will perform an ID imprint check.

        self.as_immut().copy(access_scope)
    }
}

impl<'alloc, ID: OGID, T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGMutRef<'alloc, ID, T>
{
    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this `OGMutRef`.
    ///
    /// This function has the same semantics as [`OGRef::copy`], please refer to
    /// its documentation.
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, T>> {
        // OGRef's `validate` will perform an ID imprint check.

        self.as_immut().validate(access_scope)
    }
}

impl<'alloc, ID: OGID, T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGMutRef<'alloc, ID, T>
{
    /// Create a readable, dereferencable reference of type `T` to the memory
    /// behind this `OGMutRef`.
    ///
    /// This function has the same semantics as [`OGRef::valid`], please refer to
    /// its documentation.
    pub fn valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        // OGRef's `validate` will perform an ID imprint check.

        self.as_immut().valid(access_scope)
    }
}

impl<'alloc, ID: OGID, T: Copy> OGMutRef<'alloc, ID, T> {
    /// Copy the contents of a shared reference to this mutable reference.
    ///
    /// This function requires a unique (mutable) reference to the
    /// [`AccessScope`] marker. This ensures it has unique access to foreign
    /// memory, with no concurrent reads or writes by the host or foreign
    /// library. Calling this function closes the current, and opens a new
    /// access scope.
    ///
    /// This is guaranteed to use a `memcpy` and avoid creating intermediate
    /// copies on the host stack.
    pub fn write_ref<'access>(
        &self,
        val: &T,
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, T> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        // Safety: taking &mut AccessScope<ID> ensures that no other accessible
        // references into foreign memory exist, and that no foreign code is
        // accessing this memory. The existence of this type ensures that this
        // memory is mutably accessible and well-aligned.
        //
        // We use `copy_nonoverlapping` to avoid creating intermediate copies on
        // the stack. `copy_nonoverlapping` is safe here:
        //
        // 1. `self`, by definition, is restricted to referencing foreign
        //    memory.
        //
        // 2. Any references of type `T` that may be possibly aliased with
        //    `self` would thus need to also reside in foreign memory.
        //
        // 3. However, all dereferencable references into foreign memory must be
        //    bound to a shared (immutable) borrow of this same `AccessScope`
        //    accepted in this function. This function, in turn, requires a
        //    unique (mutable) borrow of the `AccessScope`.
        //
        // Thus, this ensures that no dereferencable references into foreign
        // memory exist, that `val` cannot possibly point into foreign memory,
        // and that `val` may never alias `self`.
        unsafe {
            core::ptr::copy_nonoverlapping(
                val as *const _,
                // `MaybeValid<T>` is repr(transparent) over `T`, and every `T` is
                // valid for `MaybeValid<T>`.
                self.reference.get() as *mut T,
                1,
            )
        }

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that the reference must have a valid value right
        // now, based on the knowledge that `val` was a valid instance of T:
        unsafe { self.assume_valid(access_scope) }
    }
}

impl<'alloc, const N: usize, ID: OGID, T> OGMutRef<'alloc, ID, [T; N]> {
    /// Obtain a mutable slice reference to this array in foreign memory.
    ///
    /// The returned slice reference is bound to the same allocation scope as
    /// the original array reference.
    pub fn as_slice(&self) -> OGMutSlice<'alloc, ID, T> {
        OGMutSlice {
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
            outer
                .sub_ref(::core::mem::offset_of!($outer_type, $member,))
                .unwrap()
        }

        ogmutref_get_field_helper($outer_ref)
    }};
}

// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;
use core::marker::PhantomData;

use crate::alloc_tracker::AllocTracker;
use crate::id::OGID;
use crate::markers::AccessScope;
use crate::maybe_valid::MaybeValid;

use super::og_ref::OGRef;
use super::og_val::OGVal;

// Flags settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_UPGRADE_CHECKS;
use super::DISABLE_VALIDATION_CHECKS;

/// A slice of allocated and readable foreign memory containing objects with
/// size and alignment of type `T`.
///
/// This type is created within and bound to an
/// [`AllocScope`](crate::markers::AllocScope) valid for lifetime `'alloc`. This
/// ensures that the memory backing this slice is initialized and allocated, for
/// as long as this reference type exists.
///
/// This type may or may not contain a valid instances of type `T`. It can be
/// mutably aliased with other references into foreign memory, and its
/// underlying memory may be modified whenever foreign code runs.
pub struct OGSlice<'alloc, ID: OGID, T> {
    // The length of this slice is encoded in the reference itself (fat
    // pointer), and not located in / accessible to foreign memory:
    pub(super) reference: &'alloc [UnsafeCell<MaybeValid<T>>],
    pub(super) id_imprint: ID::Imprint,
}

// `OGSlice` is safe to clone and copy, as its copies will carry the same
// `'alloc` lifetime constraints.
impl<'alloc, ID: OGID, T> Clone for OGSlice<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGSlice<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T> OGSlice<'alloc, ID, T> {
    /// Create an `OGSlice` from a raw pointer and length.
    ///
    /// # Safety
    ///
    /// This function requires that the supplied pointer points into an
    /// allocated, initialized, and readable region of memory belonging to a
    /// foreign library instance, of at least `length * size_of::<T>()`
    /// bytes. `ptr` must be correctly aligned for type `T`. This memory must
    /// remain allocated, intialized, readable, and writeable for the duration
    /// of `'alloc`. The supplied `ID::Imprint` must originate from the [`OGID`]
    /// instance used to construct the [`OGRuntime`](crate::rt::OGRuntime)
    /// managing the memory belonging to this foreign libray instance.
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *const T,
        length: usize,
        id_imprint: ID::Imprint,
    ) -> OGSlice<'alloc, ID, T> {
        OGSlice {
            reference: unsafe {
                core::slice::from_raw_parts(ptr as *const UnsafeCell<MaybeValid<T>>, length)
            },
            id_imprint,
        }
    }

    /// Create an `OGSlice` from a raw pointer within an
    /// [`AllocScope`](crate::markers::AllocScope).
    ///
    /// This function checks whether the supplied pointer is well-aligned for
    /// type `T` and the memory at `[ptr; ptr + length * size_of::<T>())` is
    /// wholly located in a readable memory region of the foreign library. If
    /// these conditions are not true (and the unsound `disable_upgrade_checks`
    /// crate feature is not enabled), the function will return. Otherwise, it
    /// will return a reference over this pointer, bound to the supplied
    /// `AllocScope`.
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
            // checking for alignment of `T` is sufficient.
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

    /// Return a raw pointer to this reference's pointee.
    pub fn as_ptr(&self) -> *const T {
        self.reference as *const [UnsafeCell<MaybeValid<T>>] as *const T
    }

    /// Return a raw pointer to this reference's pointee.
    pub fn len(&self) -> usize {
        self.reference.len()
    }

    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
    ///
    /// This function takes a shared `AccessScope` reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory for the duration that the returned reference
    /// exists.
    ///
    /// # Safety
    ///
    /// This function requires that the memory backing this slice, at the time
    /// of calling, can be safely transmuted into a readable slice of elements
    /// of `T` with length `self.len()`, and that `T` does not feature interior
    /// mutability.
    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        unsafe {
            super::check_access_scope_imprint(self.id_imprint, access_scope);

            OGVal {
                reference: core::mem::transmute::<&[UnsafeCell<MaybeValid<T>>], &[T]>(
                    &self.reference,
                ),
                id_imprint: self.id_imprint,
                _alloc_lt: PhantomData,
            }
        }
    }

    /// Get a reference to an element within this slice, at index `idx`.
    ///
    /// If `idx < self.len()`, this returns a [`OGRef`] reference to an element
    /// in the slice at index `idx`. The reference is bound to the same
    /// allocation scope as this `OGSlice`.
    pub fn get(&self, idx: usize) -> Option<OGRef<'alloc, ID, T>> {
        self.reference.get(idx).map(|elem| OGRef {
            reference: elem,
            id_imprint: self.id_imprint,
        })
    }

    /// Obtain an iterator over the elements in this slice, yielding `OGRef`
    /// references for each element of the slice.
    pub fn iter(&self) -> OGSliceIter<'alloc, ID, T> {
        OGSliceIter {
            inner: *self,
            idx: 0,
        }
    }
}

impl<'alloc, ID: OGID, T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGSlice<'alloc, ID, T>
{
    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
    ///
    /// This function takes a shared [`AccessScope`] reference, ensuring that
    /// neither host nor foreign code can concurrently modify any (possibly
    /// aliased) foreign memory for the duration that the returned reference
    /// exists.
    ///
    /// It then checks whether the memory contents of each element in this slice
    /// would constitute a valid instance of type `T`, as determined by
    /// [`zerocopy::TryFromBytes`]. If the slice contains at least one element
    /// whose memory does not contain a valid instance of type `T` (and the
    /// unsound `disable_validation_checks` crate feature is not enabled), it
    /// returns None. Otherwise, it returns a dereferencable reference, bound to
    /// the supplied [`AccessScope`].
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, [T]>> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid(access_scope) })
        } else {
            if self
                .reference
                .iter()
                .all(|elem: &UnsafeCell<MaybeValid<T>>| {
                    <T as zerocopy::TryFromBytes>::try_ref_from_bytes(
                        unsafe { &*elem.get() }.as_bytes(),
                    )
                    .is_ok()
                })
            {
                Some(unsafe { self.assume_valid(access_scope) })
            } else {
                None
            }
        }
    }
}

impl<'alloc, ID: OGID, T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGSlice<'alloc, ID, T>
{
    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
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
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        // Every bit-pattern of every element of this slice reference is a valid
        // instance of T, so we can safely call `assume_valid`. This function
        // also checks the access scope imprint:
        unsafe { self.assume_valid(access_scope) }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<'alloc, ID: OGID, T> OGSlice<'alloc, ID, *const T> {
    pub fn valid_ptr<'access>(
        self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [*const T]> {
        unsafe { self.assume_valid(access_scope) }
    }
}

// `zerocopy` does not implement `FromBytes` for raw pointers because of
// provenance footguns, even though it is not necessarily unsound. We do need to
// be able to extract pointer values:
impl<'alloc, ID: OGID, T> OGSlice<'alloc, ID, *mut T> {
    pub fn valid_ptr<'access>(
        self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [*mut T]> {
        unsafe { self.assume_valid(access_scope) }
    }
}

pub struct OGSliceIter<'alloc, ID: OGID, T> {
    inner: OGSlice<'alloc, ID, T>,
    idx: usize,
}

impl<'alloc, ID: OGID, T> core::iter::Iterator for OGSliceIter<'alloc, ID, T> {
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

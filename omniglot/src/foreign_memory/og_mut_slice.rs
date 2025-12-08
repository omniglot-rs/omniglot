// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;

use crate::alloc_tracker::AllocTracker;
use crate::id::OGID;
use crate::markers::AccessScope;
use crate::maybe_valid::MaybeValid;

use super::og_mut_ref::OGMutRef;
use super::og_slice::OGSlice;
use super::og_val::OGVal;

// Flags settable when enabling the `unsound` crate feature, for benchmarks only:
use super::DISABLE_UPGRADE_CHECKS;

/// A slice of allocated, readable, and writeable foreign memory containing
/// objects with size and alignment of type `T`.
///
/// This type is created within and bound to an
/// [`AllocScope`](crate::markers::AllocScope) valid for lifetime `'alloc`. This
/// ensures that the memory backing this slice is initialized and allocated, for
/// as long as this reference type exists.
///
/// This type may or may not contain a valid instances of type `T`. It can be
/// mutably aliased with other references into foreign memory, and its
/// underlying memory may be modified whenever foreign code runs.
pub struct OGMutSlice<'alloc, ID: OGID, T> {
    // The length of this slice is encoded in the reference itself (fat
    // pointer), and not located in / accessible to foreign memory:
    pub(super) reference: &'alloc [UnsafeCell<MaybeValid<T>>],
    pub(super) id_imprint: ID::Imprint,
}

// `OGMutSlice` is safe to clone and copy, as its copies will carry the same
// `'alloc` lifetime constraints.
impl<'alloc, ID: OGID, T> Clone for OGMutSlice<'alloc, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, ID: OGID, T> Copy for OGMutSlice<'alloc, ID, T> {}

impl<'alloc, ID: OGID, T> OGMutSlice<'alloc, ID, T> {
    /// Create an `OGMutSlice` from a raw pointer and length.
    ///
    /// # Safety
    ///
    /// This function requires that the supplied pointer points into an
    /// allocated, initialized, readable, and writeable region of memory
    /// belonging to a foreign library instance, of at least `length *
    /// size_of::<T>()` bytes. `ptr` must be correctly aligned for type
    /// `T`. This memory must remain allocated, intialized, readable, and
    /// writeable for the duration of `'alloc`. The supplied `ID::Imprint` must
    /// originate from the [`OGID`] instance used to construct the
    /// [`OGRuntime`](crate::rt::OGRuntime) managing the memory belonging to
    /// this foreign libray instance.
    pub unsafe fn upgrade_from_ptr_unchecked(
        ptr: *mut T,
        length: usize,
        id_imprint: ID::Imprint,
    ) -> OGMutSlice<'alloc, ID, T> {
        OGMutSlice {
            reference: unsafe {
                core::slice::from_raw_parts(ptr as *mut _ as *mut UnsafeCell<MaybeValid<T>>, length)
            },
            id_imprint,
        }
    }

    /// Create an `OGMutSlice` from a raw pointer within an
    /// [`AllocScope`](crate::markers::AllocScope).
    ///
    /// This function checks whether the supplied pointer is well-aligned for
    /// type `T` and the memory at `[ptr; ptr + length * size_of::<T>())` is
    /// wholly located in a writeable memory region of the foreign library. If
    /// these conditions are not true (and the unsound `disable_upgrade_checks`
    /// crate feature is not enabled), the function will return. Otherwise, it
    /// will return a reference over this pointer, bound to the supplied
    /// `AllocScope`.
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

    /// Return a raw pointer to this reference's pointee.
    pub fn as_ptr(&self) -> *mut T {
        self.reference as *const [UnsafeCell<MaybeValid<T>>] as *mut [UnsafeCell<MaybeValid<T>>]
            as *mut T
    }

    /// Return a raw pointer to this reference's pointee.
    pub fn len(&self) -> usize {
        self.reference.len()
    }

    /// Convert this mutable slice into an immutable [`OGSlice`] slice reference.
    pub fn as_immut(&self) -> OGSlice<'alloc, ID, T> {
        OGSlice {
            reference: self.reference,
            id_imprint: self.id_imprint,
        }
    }

    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
    ///
    /// This function has the same semantics as [`OGSlice::assume_valid`],
    /// please refer to its documentation.
    pub unsafe fn assume_valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        // OGSlice's `assume_valid` will perform an ID imprint check.

        let og_slice = self.as_immut();

        // # Safety
        //
        // See method doc comment, identical to safety requirements of
        // [`OGSlice::assume_valid`].
        unsafe { og_slice.assume_valid(access_scope) }
    }

    /// Fill this slice with elements from the provided iterator.
    ///
    /// This function requires a unique (mutable) reference to the
    /// [`AccessScope`] marker. This ensures it has unique access to foreign
    /// memory, with no concurrent reads or writes by the host or foreign
    /// library. Calling this function closes the current, and opens a new
    /// access scope.
    ///
    /// It returns a validated reference ([`OGVal`]) to the filled
    /// slice. However, this reference is bound to a _unique_ borrow of the
    /// [`AccessScope`] marker. This means that, while this reference exists,
    /// neither host nor foreign code can create other references into foreign
    /// memory. To have multiple, concurrent readable references into foreign
    /// memory, drop the return value and use `validate` to re-obtain it.
    ///
    /// # Panics
    ///
    /// This function will panic if the iterator does not at least as many
    /// elements as the slice length.
    pub fn write_from_iter<'access, I: Iterator<Item = T>>(
        &self,
        src: I,
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        super::check_access_scope_imprint(self.id_imprint, access_scope);

        // Safety: taking &mut AccessScope<ID> ensures that no other accessible
        // references into foreign memory exist, and that no foreign code is
        // accessing this memory. The existence of this type ensures that this
        // memory is mutably accessible and well-aligned.
        let mut count = 0;
        self.reference.iter().zip(src).for_each(|(dst, val)| {
            (unsafe { &mut *dst.get() }).write(val);
            count += 1;
        });
        assert!(count == self.len());

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that all elements in the slice reference must have a
        // valid value right now, based on the knowledge that every element of
        // `src` was a valid instance of T, and we copied self.r.len() elements
        // from `src`:
        unsafe { self.assume_valid(access_scope) }
    }

    /// Get a reference to an element within this slice, at index `idx`.
    ///
    /// If `idx < self.len()`, this returns a [`OGMutRef`] reference to an
    /// element in the slice at index `idx`. The reference is bound to the same
    /// allocation scope as this `OGMutSlice`.
    pub fn get(&self, idx: usize) -> Option<OGMutRef<'alloc, ID, T>> {
        self.reference.get(idx).map(|elem| OGMutRef {
            reference: elem,
            id_imprint: self.id_imprint,
        })
    }

    /// Obtain an iterator over the elements in this slice, yielding `OGMutRef`
    /// references for each element of the slice.
    pub fn iter(&self) -> OGMutSliceIter<'alloc, ID, T> {
        OGMutSliceIter {
            inner: *self,
            idx: 0,
        }
    }
}

impl<'alloc, ID: OGID, T: Copy> OGMutSlice<'alloc, ID, T> {
    pub fn copy_from_slice<'access>(
        &self,
        src: &[T],
        access_scope: &'access mut AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        // Iterate over the slice itself and use `write_ref`, which is
        // guaranteed to use a `memcpy` without intermedidate copies on the
        // stack:
        let mut count = 0;
        self.iter().zip(src.iter()).for_each(|(dst, src)| {
            dst.write_ref(src, access_scope);
            count += 1;
        });
        assert!(count == self.len());

        // Provide a validated reference to the newly written memory, bound to
        // 'access. We know that all elements in the slice reference must have a
        // valid value right now, based on the knowledge that every element of
        // `src` was a valid instance of T, and we copied self.r.len() elements
        // from `src`:
        unsafe { self.assume_valid(access_scope) }
    }
}

impl<
    'alloc,
    ID: OGID,
    T: zerocopy::TryFromBytes + zerocopy::Immutable + zerocopy::KnownLayout + zerocopy::KnownLayout,
> OGMutSlice<'alloc, ID, T>
{
    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
    ///
    /// This function has the same semantics as [`OGSlice::validate`],
    /// please refer to its documentation.
    pub fn validate<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> Option<OGVal<'alloc, 'access, ID, [T]>> {
        self.as_immut().validate(access_scope)
    }
}

impl<'alloc, ID: OGID, T: zerocopy::FromBytes + zerocopy::Immutable + zerocopy::KnownLayout>
    OGMutSlice<'alloc, ID, T>
{
    /// Create a readable, dereferencable slice reference over `self.len()`
    /// elements of type `T` to the memory behind this [`OGSlice`].
    ///
    /// This function has the same semantics as [`OGSlice::valid`],
    /// please refer to its documentation.
    pub fn valid<'access>(
        &self,
        access_scope: &'access AccessScope<ID>,
    ) -> OGVal<'alloc, 'access, ID, [T]> {
        self.as_immut().valid(access_scope)
    }
}

pub struct OGMutSliceIter<'alloc, ID: OGID, T> {
    inner: OGMutSlice<'alloc, ID, T>,
    idx: usize,
}

impl<'alloc, ID: OGID, T> core::iter::Iterator for OGMutSliceIter<'alloc, ID, T> {
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

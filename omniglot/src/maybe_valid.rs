// -*- fill-column: 80; -*-

use core::mem::MaybeUninit;

/// A type representing intialized bytes with size and alignment of type `T`,
/// but not necessarily containing a valid instance of type `T`.
///
/// This is a wrapper around `MaybeUninit`, with one additional guarantee: the
/// memory it spans over must be "fixed".
///
/// TODO: Safety docs.
#[repr(transparent)]
pub struct MaybeValid<T> {
    inner: MaybeUninit<T>,
}

impl<T> MaybeValid<T> {
    pub fn zeroed() -> Self {
        MaybeValid {
            inner: MaybeUninit::zeroed(),
        }
    }

    /// Create a `MaybeValid` by filling its contents from a byte-slice.
    ///
    /// # Panic
    ///
    /// This function will panic if the supplied byte slice does not contain
    /// exactly `core::mem::size_of::<T>()` bytes.
    pub fn from_bytes(src: &[u8]) -> Self {
        let mut inner = MaybeUninit::uninit();
        let inner_bytes = crate::util::maybe_uninit_as_bytes::as_bytes_mut(&mut inner);

        // This initializes all bytes of the inner `MaybeUninit`:
        assert_eq!(inner_bytes.len(), src.len());
        inner_bytes
            .iter_mut()
            .zip(src.iter())
            .for_each(|(dst, src)| {
                dst.write(*src);
            });

        MaybeValid { inner }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::mem::transmute::<&[MaybeUninit<u8>], &[u8]>(
                crate::util::maybe_uninit_as_bytes::as_bytes(&self.inner),
            )
        }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(
                crate::util::maybe_uninit_as_bytes::as_bytes_mut(&mut self.inner),
            )
        }
    }

    pub unsafe fn assume_valid(self) -> T {
        unsafe { self.inner.assume_init() }
    }

    pub unsafe fn assume_valid_ref(&self) -> &T {
        unsafe { self.inner.assume_init_ref() }
    }

    pub fn write(&mut self, val: T) -> &mut T {
        self.inner.write(val)
    }
}

impl<T: zerocopy::IntoBytes> MaybeValid<T> {
    pub fn new(val: T) -> Self {
        MaybeValid {
            inner: MaybeUninit::new(val),
        }
    }
}

impl<T: Copy> Clone for MaybeValid<T> {
    fn clone(&self) -> Self {
        MaybeValid {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Copy> Copy for MaybeValid<T> {}

impl<T> core::fmt::Debug for MaybeValid<T> {
    // Copied (and adjusted) from `MaybeUninit::fmt`
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // NB: there is no `.pad_fmt` so we can't use a simpler `format_args!("MaybeValid<{..}>").
        let full_name = core::any::type_name::<Self>();
        let prefix_len = full_name.find("MaybeValid").unwrap();
        f.pad(&full_name[prefix_len..])
    }
}

use core::mem::MaybeUninit;

use crate::bit_pattern_validate::BitPatternValidate;
use crate::id::OGID;
use crate::markers::AccessScope;

use super::DISABLE_VALIDATION_CHECKS;
use super::og_mut_ref::OGMutRef;
use super::og_ref::OGRef;

use crate::util::maybe_uninit_as_bytes;

// An owned copy from some unvalidated foreign memory
#[repr(transparent)]
pub struct OGCopy<T: 'static>(pub(crate) MaybeUninit<T>);

impl<T: 'static> core::fmt::Debug for OGCopy<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.pad(core::any::type_name::<Self>())
    }
}

// Need to manually implement Clone, Rust won't derive it automatically because
// not necessarily T: Clone.
impl<T: 'static> Clone for OGCopy<T> {
    fn clone(&self) -> Self {
        // Do a safe byte-wise copy of the MaybeUninit. It does not necessarily
        // implement Copy. However, we only support dereferencing it after being
        // validated through BitPatternValidate, and that must never support validating a
        // value that is not safely copy-able.
        let mut clone = MaybeUninit::<T>::uninit();
        maybe_uninit_as_bytes::as_bytes_mut(&mut clone)
            .copy_from_slice(maybe_uninit_as_bytes::as_bytes(&self.0));
        OGCopy(clone)
    }
}

impl<T: 'static> From<MaybeUninit<T>> for OGCopy<T> {
    fn from(from: MaybeUninit<T>) -> Self {
        OGCopy(from)
    }
}

impl<T: 'static> OGCopy<T> {
    pub fn new(val: T) -> Self {
        OGCopy(MaybeUninit::new(val))
    }

    // TODO: does this need to be unsafe? Presumably yes, based on my
    // interpretation of a conversation with Ralf. Document safety invariants!
    pub unsafe fn uninit() -> Self {
        OGCopy(MaybeUninit::uninit())
    }

    pub fn zeroed() -> Self {
        OGCopy(MaybeUninit::zeroed())
    }

    pub fn update_from_ref<ID: OGID>(
        &mut self,
        r: OGRef<'_, ID, T>,
        access_scope: &AccessScope<ID>,
    ) {
        if r.id_imprint() != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                r.id_imprint(),
                access_scope.id_imprint()
            );
        }

        // Safety: taking &AccessScope<ID> ensures that no mutable accessible
        // references into foreign memory exist, and that no foreign code is
        // accessing this memory. The existance of this type ensures that this
        // memory is mutably accessible and well-aligned.
        maybe_uninit_as_bytes::as_bytes_mut(&mut self.0)
            .copy_from_slice(maybe_uninit_as_bytes::as_bytes(unsafe { &*r.r.get() }));
    }

    pub fn update_from_mut_ref<ID: OGID>(
        &mut self,
        r: OGMutRef<'_, ID, T>,
        access_scope: &AccessScope<ID>,
    ) {
        if r.id_imprint() != access_scope.id_imprint() {
            panic!(
                "ID mismatch: {:?} vs. {:?}!",
                r.id_imprint(),
                access_scope.id_imprint()
            );
        }

        self.update_from_ref(r.as_immut(), access_scope)
    }

    pub unsafe fn assume_valid(self) -> T {
        unsafe { self.0.assume_init() }
    }

    pub unsafe fn assume_valid_ref(&self) -> &T {
        unsafe { self.0.assume_init_ref() }
    }
}

impl<T: BitPatternValidate + 'static> OGCopy<T> {
    pub fn validate(self) -> Result<T, Self> {
        if DISABLE_VALIDATION_CHECKS {
            Ok(unsafe { self.assume_valid() })
        } else {
            if unsafe {
                <T as BitPatternValidate>::validate(&self.0 as *const MaybeUninit<T> as *const T)
            } {
                Ok(unsafe { self.0.assume_init() })
            } else {
                Err(self)
            }
        }
    }

    pub fn validate_ref<'a>(&'a self) -> Option<&'a T> {
        if DISABLE_VALIDATION_CHECKS {
            Some(unsafe { self.assume_valid_ref() })
        } else {
            if unsafe {
                <T as BitPatternValidate>::validate(&self.0 as *const MaybeUninit<T> as *const T)
            } {
                Some(unsafe { self.0.assume_init_ref() })
            } else {
                None
            }
        }
    }

    pub fn validate_copy(&self) -> Option<T> {
        // TODO: maybe more efficient to validate ref first, then clone:
        let cloned = self.clone();
        cloned.validate().ok()
    }
}

use core::cmp::{PartialEq, PartialOrd};
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{OGID, OGIDImprint};

static OG_RUNTIME_BRANDING_CTR: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct OGRuntimeBranding {
    id: u64,
    /// Prevent this struct from being constructed outside of this module
    _private: (),
}

impl OGRuntimeBranding {
    pub fn new() -> Self {
        let id = OG_RUNTIME_BRANDING_CTR
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev_id| {
                prev_id.checked_add(1)
            })
            .expect("Overflow generating new OGRuntimeBranding ID");

        OGRuntimeBranding { id, _private: () }
    }
}

unsafe impl OGID for OGRuntimeBranding {
    type Imprint = OGRuntimeBrandingImprint;

    #[inline(always)]
    fn get_imprint(&self) -> Self::Imprint {
        OGRuntimeBrandingImprint {
            id: self.id,
            _private: (),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub struct OGRuntimeBrandingImprint {
    id: u64,
    /// Prevent this struct from being constructed outside of this module
    _private: (),
}
unsafe impl OGIDImprint for OGRuntimeBrandingImprint {}

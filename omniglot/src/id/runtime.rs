use core::cmp::{PartialEq, PartialOrd};
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{OGID, OGIDImprint};

static OG_RUNTIME_BRANDING_CTR: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct OGRuntimeBranding(u64);

impl OGRuntimeBranding {
    pub fn new() -> Self {
        let id = OG_RUNTIME_BRANDING_CTR
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev_id| {
                prev_id.checked_add(1)
            })
            .expect("Overflow generating new OGRuntimeBranding ID");

        OGRuntimeBranding(id)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub struct OGRuntimeBrandingImprint(u64);

unsafe impl OGIDImprint for OGRuntimeBrandingImprint {
    fn numeric_id(&self) -> u64 {
        self.0
    }
}

unsafe impl OGID for OGRuntimeBranding {
    type Imprint = OGRuntimeBrandingImprint;

    #[inline(always)]
    fn get_imprint(&self) -> Self::Imprint {
        OGRuntimeBrandingImprint(self.0)
    }
}

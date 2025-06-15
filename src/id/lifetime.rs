use core::cell::Cell;
use core::cmp::{PartialEq, PartialOrd};
use core::fmt::Debug;
use core::marker::PhantomData;

use super::{OGID, OGIDImprint};

/// TODO: Write docs
///
///
/// ```compile_fail
/// use encapfn::branding::{OGID, OGLifetimeBranding};
///
/// OGLifetimeBranding::new::<()>(move |brand_a| {
///     OGLifetimeBranding::new::<()>(move |brand_b| {
///	    assert!(!OGLifetimeBranding::compare(&brand_a.get_imprint(), &brand_b.get_imprint()));
///     });
/// });
/// ```
#[derive(Debug)]
pub struct OGLifetimeBranding<'id>(PhantomData<Cell<&'id ()>>);

impl OGLifetimeBranding<'_> {
    #[inline(always)]
    pub fn new<R>(f: impl for<'new_id> FnOnce(OGLifetimeBranding<'new_id>) -> R) -> R {
        f(OGLifetimeBranding(PhantomData))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialOrd)]
pub struct OGLifetimeBrandingImprint<'id>(PhantomData<Cell<&'id ()>>);

unsafe impl<'id> OGIDImprint for OGLifetimeBrandingImprint<'id> {
    fn numeric_id(&self) -> u64 {
        0_u64
    }
}

impl<'id> PartialEq<OGLifetimeBrandingImprint<'id>> for OGLifetimeBrandingImprint<'id> {
    fn eq(&self, _rhs: &OGLifetimeBrandingImprint<'id>) -> bool {
        // Imprint is invariant over the `'id` lifetime. Thus, the fact that
        // we're provided two types with identical lifetimes means that the
        // imprint must have been issued from the same branded lifetime, no
        // runtime check required:
        true
    }
}

unsafe impl<'id> OGID for OGLifetimeBranding<'id> {
    type Imprint = OGLifetimeBrandingImprint<'id>;

    #[inline(always)]
    fn get_imprint(&self) -> Self::Imprint {
        OGLifetimeBrandingImprint(PhantomData)
    }
}

#[test]
fn test_lifetime_branding_equality() {
    OGLifetimeBranding::new::<()>(|brand| {
        let imprint_a = brand.get_imprint();
        let imprint_b = brand.get_imprint();
        assert_eq!(imprint_a, imprint_b);
    })
}

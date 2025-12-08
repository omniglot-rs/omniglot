// -*- fill-column: 80; -*-

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
///         assert!(!OGLifetimeBranding::compare(&brand_a.get_imprint(), &brand_b.get_imprint()));
///     });
/// });
/// ```
#[derive(Debug)]
pub struct OGLifetimeBranding<'id> {
    /// Make struct invariant over `'id` lifetime
    _inv_lt: PhantomData<Cell<&'id ()>>,
    /// Prevent this struct from being constructed outside of this module
    _private: (),
}

impl OGLifetimeBranding<'_> {
    #[inline(always)]
    pub fn new<R>(f: impl for<'new_id> FnOnce(OGLifetimeBranding<'new_id>) -> R) -> R {
        f(OGLifetimeBranding {
            _inv_lt: PhantomData,
            _private: (),
        })
    }
}

unsafe impl<'id> OGID for OGLifetimeBranding<'id> {
    type Imprint = OGLifetimeBrandingImprint;

    #[inline(always)]
    fn get_imprint(&self) -> Self::Imprint {
        OGLifetimeBrandingImprint { _private: () }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialOrd)]
pub struct OGLifetimeBrandingImprint {
    /// Prevent this struct from being constructed outside of this module
    _private: (),
}
unsafe impl OGIDImprint for OGLifetimeBrandingImprint {}

impl PartialEq<OGLifetimeBrandingImprint> for OGLifetimeBrandingImprint {
    fn eq(&self, _rhs: &OGLifetimeBrandingImprint) -> bool {
        // [`OGLifetimeBranding`] is invariant over its `'id` lifetime. Thus,
        // the fact that we're provided two imprints that the caller claims to
        // have originated from the same [`OGLifetimeBranding`] type means that
        // the imprint must have been issued from the same branded lifetime, no
        // runtime check required:
        true
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

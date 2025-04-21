//! Omniglot-instance IDs.
//!
//! Omiglot enforces many of its safety invariants by issuing affine marker type
//! instances ([`AllocScope`], [`AccessScope`]), which are used to construct a
//! form of compile-time mutual exclusion (like ensuring that foreign memory is
//! not modified while dereferencable references like an [`OGVal`] exist to it).
//!
//! However, for this to work, it is important that each Omniglot runtime issues
//! only a single such instance of each marker type, and that only this instance
//! can be used in conjuction with Omniglot runtime functions that rely on them
//! to enforce these mutual exclusion properties. With multiple Omniglot
//! runtimes, each wrapping their own library instances, this may not hold.
//!
//! For this reason, we brand each Omniglot runtime and marker type with an
//! [`OGID`] generic argument. There are two families of branding we can use:
//!
//! - We can make each Omniglot runtime instance use a different [`OGID`] type.
//!
//!   Then, Rust's type system itself ensures that no runtime's marker types can
//!   be used with another runtime that they weren't created alongside
//!   with. Thus, such a form of branding carries no runtime overhead.
//!
//!   Within this family, there are two approaches:
//!
//!   - Users of Omniglot can implement the [`OGID`] unsafe trait for their own
//!     types. They are themselves responsible for ensuring that each [`OGID`]
//!     type is only ever used once for instantiating an Omniglot runtime.
//!
//!   - Users can use lifetime-branded IDs through the provided
//!     [`OGLifetimeBranding`] type. This type can be safely instantiated for an
//!     anonymous, invariant lifetime `'id` through [`OGLifetimeBranding::new`].
//!     Key to the safety of this function is that no two calls can produce an
//!     [`OGLifetimeBranding`] generic over lifetimes that can be subtyped by
//!     another (meaning they are _invariant_). See the GhostCell paper, section
//!     2.2 [1] for more information.
//!
//! - We can safely instantiate multiple Omniglot runtime instances using the
//!   same [`OGRuntimeBranding`] type, and perform runtime checks to ensure that
//!   two Omniglot components (such as the runtime and wrappers) have been
//!   created from the same [`OGRuntimeBranding`] instance.
//!
//! While the former approach is very efficient, it can be limiting: because it
//! forces each Omniglot runtime instance to be generic over a different type,
//! it can produce significant code bloat through monomorphization, and can
//! result in unwieldy type expressions. In particular, lifetime-branding can be
//! difficult to program against, as developers can never _name_ the proper
//! [`OGID`] type, and have to express all functions as generic over any
//! [`OGLifetimeBranding`] (or any [`OGID`]).
//!
//! Esp. when maintaining a thread-pool of different Omniglot instances, it may
//! be problematic to have each instance be of a different type. In such cases,
//! developers can use [`OGRuntimeBranding`]s to work around these issues.
//!
//! [1]: https://plv.mpi-sws.org/rustbelt/ghostcell/paper.pdf

use core::cell::Cell;
use core::cmp::{PartialEq, PartialOrd};
use core::fmt::Debug;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};

/// An Omniglot instance ID type.
///
/// See the module-level documentation for more information on [`OGID`]s.
///
/// An [`OGID`] must never be able to be duplicated, i.e., it must be an
/// `affine` type.
///
/// The [`OGID::get_imprint`] method can be used to obtain an "imprint" of this
/// ID instance, which is not subject to those same constraints: it can be
/// freely duplicated. However, the follwing constraints must hold:
///
/// - for any [`OGID`] instance `A` and any two imprints of `A` `a_1` and `a_2`,
/// `a_1 == a_2`.
///
/// - for any [`OGID`] instance `A`, and imprint of `A` `a`, and a different
///   [`OGID`] instance `B` with an imprint of `B` `b`, `a != b`.
///
///
///
/// Implementations of this trait can satisfy these constraints two one of two
/// ways:
///
/// - they must either guarantee uniqueness of instances of this type and its
///   imprints (i.e., guarantee that a type implementing this trait can only
///   ever be instantiated into a single instance through the program's
///   lifetime), in which case they may implement their imprints' `eq` function
///   as a no-op that is unconditionally true.
///
/// - otherwise, when there can be multiple instances of a type implementing
///   [`OGID`], they must maintain the above equality relations by comparing
///   runtime-available state.
pub unsafe trait OGID: Debug {
    type Imprint: OGIDImprint + Debug + Copy + Clone + Eq + PartialEq + PartialOrd;

    fn get_imprint(&self) -> Self::Imprint;
}

pub unsafe trait OGIDImprint {
    /// Return a numeric ID that unique identifies this Imprint's originating
    /// [`OGID`]'s instance among other instances of that same [`OGID`] type.
    ///
    /// [`OGID`]s that guarantee singleton-type uniqueness can return any fixed
    /// number here.
    fn numeric_id(&self) -> u64;
}

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

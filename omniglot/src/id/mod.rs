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

use core::cmp::{PartialEq, PartialOrd};
use core::fmt::Debug;

pub mod lifetime;

#[cfg(feature = "runtime_id")]
pub mod runtime;

/// An Omniglot instance ID type.
///
/// See the module-level documentation for more information on [`OGID`]s.
///
/// An [`OGID`] must never be able to be duplicated, i.e., it must be an
/// `affine` type.
///
/// The [`OGID::get_imprint`] method can be used to obtain an [`OGImprint`] of
/// this ID instance, which is not subject to those same constraints: it can be
/// freely duplicated. However, the follwing constraints must hold:
///
/// - for any [`OGID`] instance `A` and any two imprints of `A` `a_1` and `a_2`,
/// `a_1 == a_2`.
///
/// - for any [`OGID`] instance `A`, and imprint of `A` `a`, and a
///   different [`OGID`] instance `B` with an imprint of `B` `b`,
///   where the type of `A` and `B` is identical, `a != b`.
///
/// As such, comparing two [`OGImprint`]s originating from the same [`OGID`]
/// type indicates whether they originate from the same [`OGID`] instance. It
/// does not make sense to compare imprints originating from disparate [`OGID`]s.
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
    type Imprint: OGIDImprint;

    fn get_imprint(&self) -> Self::Imprint;
}

pub unsafe trait OGIDImprint:
    Debug + Copy + Clone + Eq + PartialEq + PartialOrd + 'static
{
}

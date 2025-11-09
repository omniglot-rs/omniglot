#![no_std]
// The doc_cfg feature allows an API be documented as only available in some
// specific platforms. As this is only available on nightly, we gate it behind
// this crate's `nightly` feature flag.
//
// https://doc.rust-lang.org/unstable-book/language-features/doc-cfg.html
#![cfg_attr(feature = "nightly", feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

// Public modules:
pub mod abi;
pub mod alloc_tracker;
pub mod bit_pattern_validate;
pub mod foreign_memory;
pub mod id;
pub mod markers;
pub mod rt;

// Internal modules:
mod util;

/// Shared Omniglot error type
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OGError {
    /// An internal error occurred.
    ///
    /// TODO: If the `log` feature is enabled, additional context of this error
    /// will be logged at level `ERROR`.
    InternalError,

    /// The Omniglot runtime could not allocate sufficient memory for the
    /// requested operation.
    ///
    /// This may either indicate that the main program's global heap is
    /// exhausted, or that the foreign domain's assigned memory cannot hold the
    /// requested allocation.
    AllocNoMem,

    /// The requested operation requires an allocation with invalid layout (such
    /// as a zero-length allocation).
    AllocInvalidLayout,

    /// The Omniglot runtime could not allocate the callback due to an
    /// insufficient number of callback slots.
    SetupCallbackInsufficientSlots,

    /// The operation could not be completed, as there is a mismatch between the
    /// IDs of different Omniglot runtime components.
    ///
    /// Most likely, this error indicates that the supplied [`AllocScope`] or
    /// [`AccessScope`] marker type is of the expected type, but belongs to a
    /// different Omniglot runtime instance. Marker types must always be used
    /// with the exact Omniglot instance alongside which they were created.
    IDMismatch,

    /// A stack overflow occurred in the foreign library.
    StackOverflow,
}

pub type OGResult<T> = Result<T, OGError>;

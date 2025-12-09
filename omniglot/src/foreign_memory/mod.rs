// -*- fill-column: 80; -*-

use crate::alloc_tracker::AllocTracker;
use crate::markers::{AccessScope, AllocScope};

pub mod og_copy;
pub mod og_mut_ref;
pub mod og_mut_slice;
pub mod og_ref;
pub mod og_ret;
pub mod og_slice;
pub mod og_val;

// Features for disabling checks on `upgrade` and `validation`
// operations. Enabling these features is unsound and only supported for
// benchmarking purposes.
//
// We require the `unsound` feature to also be enabled for each of those
// features. Otherwise, we exit with a compile error.
//
// The `build.rs` script will further issue a warning when either of these
// features is enabled.
#[cfg(all(not(feature = "unsound"), feature = "disable_upgrade_checks"))]
compile_error!("Must enable feature \"unsound\" when enabling feature \"disable_upgrade_checks\"");
const DISABLE_UPGRADE_CHECKS: bool = cfg!(feature = "disable_upgrade_checks");

#[cfg(all(not(feature = "unsound"), feature = "disable_validation_checks"))]
compile_error!(
    "Must enable feature \"unsound\" when enabling feature \"disable_validation_checks\""
);
const DISABLE_VALIDATION_CHECKS: bool = cfg!(feature = "disable_validation_checks");

// The type of `AllocScope` accepted for `upgrade` methods. Depending on whether
// the `alloc_scope_separate_active_valid_lt` feature is enabled, we either bind
// {OGRef,OGMutRef} references only to the "valid" lifetime of the `AllocScope`,
// or both the "active" and "valid" lifetimes:
#[cfg(feature = "alloc_scope_separate_active_valid_lt")]
type UpgradeAllocScopeTy<'anon, 'alloc, R, ID> = &'anon AllocScope<'alloc, R, ID>;
#[cfg(not(feature = "alloc_scope_separate_active_valid_lt"))]
type UpgradeAllocScopeTy<'anon, 'alloc, R, ID> = &'alloc AllocScope<'alloc, R, ID>;

// Helper function to check the imprint of the OGID from an `AllocScope` against
// the imprint stored in a reference type:
#[allow(dead_code)]
fn check_alloc_scope_imprint<ID: crate::id::OGID, R: AllocTracker>(
    ref_imprint: ID::Imprint,
    alloc_scope: &AllocScope<'_, R, ID>,
) {
    if ref_imprint != alloc_scope.id_imprint() {
        check_scopes_imprint_panic::<ID>(ref_imprint, alloc_scope.id_imprint());
    }
}

// Helper function to check the imprint of the OGID from an `AccessScope`
// against the imprint stored in a reference type:
fn check_access_scope_imprint<ID: crate::id::OGID>(
    ref_imprint: ID::Imprint,
    access_scope: &AccessScope<ID>,
) {
    if ref_imprint != access_scope.id_imprint() {
        check_scopes_imprint_panic::<ID>(ref_imprint, access_scope.id_imprint());
    }
}

// Helper function to check the imprint of the OGIDs from an `AllocScope` and an
// `AccessScope`1 against the imprint stored in a reference type:
#[allow(dead_code)]
fn check_scopes_imprint<ID: crate::id::OGID, R: AllocTracker>(
    ref_imprint: ID::Imprint,
    alloc_scope: &AllocScope<'_, R, ID>,
    access_scope: &AccessScope<ID>,
) {
    if ref_imprint != alloc_scope.id_imprint() {
        check_scopes_imprint_panic::<ID>(ref_imprint, alloc_scope.id_imprint());
    } else if ref_imprint != access_scope.id_imprint() {
        check_scopes_imprint_panic::<ID>(ref_imprint, access_scope.id_imprint());
    }
}

// Common panic function for failed scope imprint checks:
pub(self) fn check_scopes_imprint_panic<ID: crate::id::OGID>(
    imprint_a: ID::Imprint,
    imprint_b: ID::Imprint,
) {
    panic!("ID mismatch: {:?} vs. {:?}!", imprint_a, imprint_b,);
}

const fn sub_ref_check<T, U>(byte_offset: usize) -> bool {
    use core::mem::{align_of, size_of};

    // First, ensure that an element of type `U` at offset `byte_offset` fits
    // within `T`.
    //
    // TODO: use `is_none_or` once its const-stabilized:
    if let Some(s) = byte_offset.checked_add(size_of::<U>()) {
        if s > size_of::<T>() {
            // Would exceed the size of T:
            return false;
        }

    // Size check OK, fall-through:
    } else {
        // Overflow calculating size:
        return false;
    }

    // Now, check if a value of type `U`, at offset `byte_offset` within a value
    // of type `T`, would be well-aligned.
    //
    // This, effectively, amounts to computing the greatest common divisor
    // between `T`'s alignment, and the requested `byte_offset`. For instance,
    // for `align(T) = 8` and a byte offset `12`, the alignment of `U` must be
    // less than or equal to `gcd(8, 12) = 4`.
    //
    // Because Rust guarantees us [1] that type alignment is *always* a power of
    // two, we can compute this GCD efficiently by OR-ing `T`'s alignment with
    // `byte_offset`, and counting the trailing zeroes. If
    //
    //     U.align() <= 2**(trailing_zeroes(T.align() | byte_offset))
    //
    // then a value of type `U` within a value of type `T` at an offset of
    // `offset_bytes` will be well-aligned.
    if align_of::<U>() > (1 << (align_of::<T>() | byte_offset).trailing_zeros()) {
        return false;
    }

    // Value of type `U` at `byte_offset` in `T` fits and is well-aligned:
    true
}

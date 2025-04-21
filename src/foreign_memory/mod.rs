pub mod og_copy;
pub mod og_mut_ref;
pub mod og_mut_slice;
pub mod og_ref;
pub mod og_slice;
pub mod og_slice_val;
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

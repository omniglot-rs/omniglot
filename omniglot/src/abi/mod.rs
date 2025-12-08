// -*- fill-column: 80; -*-

pub trait OGABI {}

pub mod calling_convention;
pub mod rv32i_c;
pub mod sysv_amd64;

// For Mock implementations, that don't have any ABI constraints
pub enum GenericABI {}
impl OGABI for GenericABI {}

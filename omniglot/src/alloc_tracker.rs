// -*- fill-column: 80; -*-

pub unsafe trait AllocTracker {
    fn is_valid(&self, ptr: *const (), len: usize) -> bool;
    fn is_valid_mut(&self, ptr: *mut (), len: usize) -> bool;
}

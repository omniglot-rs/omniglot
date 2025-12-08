// -*- fill-column: 80; -*-

pub struct HeapAllocator;

impl super::MockRtAllocator for HeapAllocator {
    unsafe fn with_alloc<R, F: FnOnce(*mut ()) -> R>(
        &self,
        layout: core::alloc::Layout,
        f: F,
    ) -> Result<R, super::MockRtAllocError> {
        if layout.size() == 0 {
            return Err(super::MockRtAllocError::InvalidLayout);
        }

        // We're not actually allocating on the stack here, but still providing
        // similar semantics by freeing allocations once we pop the current
        // stack frame:
        let ptr = unsafe { std::alloc::alloc(layout) };

        // Execute the function:
        let ret = f(ptr as *mut ());

        // We free the pointer again. There should not be any valid Rust
        // references to this memory in scope any longer, as they must have been
        // bound to the AllocScope with the anonymous lifetime as passed
        // (reborrowed) into the closure:
        unsafe {
            std::alloc::dealloc(ptr, layout);
        }

        Ok(ret)
    }
}

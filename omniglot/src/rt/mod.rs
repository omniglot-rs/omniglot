// TODO: why do we need these?
pub mod mock;
pub mod rv32i_c;
pub mod sysv_amd64;

use crate::abi::OGABI;
use crate::alloc_tracker::AllocTracker;
use crate::foreign_memory::{
    og_mut_ref::OGMutRef, og_mut_slice::OGMutSlice, og_ref::OGRef, og_slice::OGSlice,
};
use crate::id::OGID;
use crate::markers::{AccessScope, AllocScope};
use crate::{OGError, OGResult};

pub trait CallbackContext {
    fn get_argument_register(&self, reg: usize) -> Option<usize>;
}

pub trait CallbackReturn {
    fn set_return_register(&mut self, reg: usize, value: usize) -> bool;
}

pub unsafe trait OGRuntime {
    type ID: OGID;
    type AllocTracker<'a>: AllocTracker;
    type ABI: OGABI;
    type CallbackTrampolineFn;
    type CallbackContext: CallbackContext + core::fmt::Debug + Clone;
    type CallbackReturn: CallbackReturn + core::fmt::Debug + Clone;

    type SymbolTableState<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>;

    fn resolve_symbols<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        symbol_table: &'static [&'static core::ffi::CStr; SYMTAB_SIZE],
        fixed_offset_symbol_table: &'static [Option<&'static core::ffi::CStr>;
                     FIXED_OFFSET_SYMTAB_SIZE],
    ) -> Result<
        Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>,
        Option<&'static core::ffi::CStr>,
    >;

    fn lookup_symbol<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        compact_symtab_index: usize,
        fixed_offset_symtab_index: usize,
        symtabstate: &Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>,
    ) -> Option<*const ()>;

    fn setup_callback<C, F, R>(
        &self,
        callback: &mut C,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        C: FnMut(
            &Self::CallbackContext,
            &mut Self::CallbackReturn,
            &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &mut AccessScope<Self::ID>,
        ),
        F: for<'b> FnOnce(
            *const Self::CallbackTrampolineFn,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        ) -> R;

    // Can be used to set up memory protection before running the
    // invoke asm. May be implemented as a nop.
    fn execute<R, F: FnOnce() -> R>(
        &self,
        target_symbol: *const (),
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        f: F,
    ) -> OGResult<R>;

    fn allocate_stacked_untracked_mut<F, R>(
        &self,
        layout: core::alloc::Layout,
        fun: F,
    ) -> OGResult<R>
    where
        F: FnOnce(*mut ()) -> R;

    // TODO: document layout requirements!
    fn allocate_stacked_mut<F, R>(
        &self,
        layout: core::alloc::Layout,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(*mut (), &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>) -> R;

    // TODO: what about zero-sized T?
    fn allocate_stacked_t_mut<T: Sized + 'static, F, R>(
        &self,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(
            OGMutRef<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        ) -> R,
    {
        let id_imprint = alloc_scope.id_imprint();
        self.allocate_stacked_mut(
            core::alloc::Layout::new::<T>(),
            alloc_scope,
            |allocated_ptr, new_alloc_scope| {
                fun(
                    unsafe {
                        OGMutRef::upgrade_from_ptr_unchecked(allocated_ptr as *mut T, id_imprint)
                    },
                    new_alloc_scope,
                )
            },
        )
    }

    fn write_stacked_t_mut<T: Sized + 'static, F, R>(
        &self,
        t: T,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGMutRef<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.allocate_stacked_t_mut(alloc_scope, |allocation, new_alloc_scope| {
            allocation.write(t, access_scope);
            fun(allocation, new_alloc_scope, access_scope)
        })
    }

    fn write_stacked_t<T: Sized + 'static, F, R>(
        &self,
        t: T,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(
            OGRef<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.write_stacked_t_mut(
            t,
            alloc_scope,
            access_scope,
            |allocation, new_alloc_scope, new_access_scope| {
                fun(allocation.as_immut(), new_alloc_scope, new_access_scope)
            },
        )
    }

    fn write_stacked_ref_t_mut<T: Sized + Copy + 'static, F, R>(
        &self,
        t: &T,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGMutRef<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.allocate_stacked_t_mut(alloc_scope, |allocation, new_alloc_scope| {
            allocation.write_ref(t, access_scope);
            fun(allocation, new_alloc_scope, access_scope)
        })
    }

    fn write_stacked_ref_t<T: Sized + Copy + 'static, F, R>(
        &self,
        t: &T,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGRef<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.write_stacked_ref_t_mut(
            t,
            alloc_scope,
            access_scope,
            |allocation, new_alloc_scope, new_access_scope| {
                fun(allocation.as_immut(), new_alloc_scope, new_access_scope)
            },
        )
    }

    // TODO: what about zero-sized T?
    fn allocate_stacked_slice_mut<T: Sized + 'static, F, R>(
        &self,
        len: usize,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGMutSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        ) -> R,
    {
        let id_imprint = alloc_scope.id_imprint();
        self.allocate_stacked_mut(
            core::alloc::Layout::array::<T>(len).unwrap(),
            alloc_scope,
            |allocated_ptr, new_alloc_scope| {
                fun(
                    unsafe {
                        OGMutSlice::upgrade_from_ptr_unchecked(
                            allocated_ptr as *mut T,
                            len,
                            id_imprint,
                        )
                    },
                    new_alloc_scope,
                )
            },
        )
    }

    // TODO: what about an empty iterator?
    fn write_stacked_slice_from_iter_mut<T: Sized + 'static, F, R>(
        &self,
        src: impl Iterator<Item = T> + ExactSizeIterator,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(
            OGMutSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.allocate_stacked_slice_mut(src.len(), alloc_scope, |allocation, new_alloc_scope| {
            // This will panic if the iterator did not yield exactly `src.len()`
            // elements:
            allocation.write_from_iter(src, access_scope);
            fun(allocation, new_alloc_scope, access_scope)
        })
    }

    fn write_stacked_slice_mut<T: Sized + Copy + 'static, F, R>(
        &self,
        src: &[T],
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGMutSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.write_stacked_slice_from_iter_mut(src.iter().copied(), alloc_scope, access_scope, fun)
    }

    fn write_stacked_slice_from_iter<T: Sized + 'static, F, R>(
        &self,
        src: impl Iterator<Item = T> + ExactSizeIterator,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.write_stacked_slice_from_iter_mut(
            src,
            alloc_scope,
            access_scope,
            |allocation, new_alloc_scope, new_access_scope| {
                fun(allocation.as_immut(), new_alloc_scope, new_access_scope)
            },
        )
    }

    fn write_stacked_slice<T: Sized + Copy + 'static, F, R>(
        &self,
        src: &[T],
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(
            OGSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        self.write_stacked_slice_from_iter(src.iter().copied(), alloc_scope, access_scope, fun)
    }
}

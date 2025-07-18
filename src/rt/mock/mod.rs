use core::cell::UnsafeCell;
use core::ffi::{CStr, c_void};
use core::marker::PhantomData;
use core::mem::MaybeUninit;

use crate::abi::GenericABI;
use crate::alloc_tracker::AllocTracker;
use crate::foreign_memory::{og_mut_ref::OGMutRef, og_ref::OGRef, og_slice::OGSlice};
use crate::id::OGID;
use crate::markers::{AccessScope, AllocScope};
use crate::rt::{CallbackContext, CallbackReturn, OGRuntime};
use crate::{OGError, OGResult};

#[cfg_attr(feature = "nightly", doc(cfg(feature = "std")))]
#[cfg(any(feature = "std", doc))]
pub mod heap_alloc;

pub mod stack_alloc;

// Use 6 arguments, as that's how many are passed in registers on x86.
#[repr(C)]
pub struct CallbackTrampolineFnReturn {
    reg0: usize,
    reg1: usize,
}

type CallbackTrampolineFn =
    unsafe extern "C" fn(usize, usize, usize, usize, usize, usize) -> CallbackTrampolineFnReturn;

#[derive(Debug, Clone)]
pub struct MockRtCallbackContext {
    pub arg_regs: [usize; 6],
}

impl CallbackContext for MockRtCallbackContext {
    fn get_argument_register(&self, reg: usize) -> Option<usize> {
        self.arg_regs.get(reg).copied()
    }
}

#[derive(Debug, Clone)]
pub struct MockRtCallbackReturn {
    pub return_regs: [usize; 2],
}

impl CallbackReturn for MockRtCallbackReturn {
    fn set_return_register(&mut self, reg: usize, value: usize) -> bool {
        if let Some(r) = self.return_regs.get_mut(reg) {
            *r = value;
            true
        } else {
            false
        }
    }
}

// TODO: this should be a hashmap which takes a runtime ID derived from the OGID
// as key, to work with multiple mock runtimes in parallel:
static mut ACTIVE_ALLOC_CHAIN_HEAD_ROG: Option<(*const MockRtAllocChain<'static>, *const ())> =
    None;

#[inline(never)]
extern "C" fn mock_rt_callback_dispatch<ID: OGID>(
    callback_id: usize,
    callback_ctx: &MockRtCallbackContext,
    callback_ret: &mut MockRtCallbackReturn,
) {
    let alloc_chain_head_ref_opt: &Option<(*const MockRtAllocChain<'_>, *const ())> =
        unsafe { &*core::ptr::addr_of!(ACTIVE_ALLOC_CHAIN_HEAD_ROG) };

    let (alloc_chain_head_ref_ptr, id_imprint_ptr) = alloc_chain_head_ref_opt.unwrap();
    let alloc_chain_head_ref: &MockRtAllocChain<'static> = unsafe { &*alloc_chain_head_ref_ptr };
    let id_imprint: &ID::Imprint = unsafe { &*(id_imprint_ptr as *const ID::Imprint) };

    let callback_desc = alloc_chain_head_ref
        .find_callback_descriptor(callback_id)
        .expect("Callback not found!");

    let mut inner_alloc_scope: AllocScope<'_, MockRtAllocChain<'_>, ID> =
        unsafe { AllocScope::new(MockRtAllocChain::Cons(alloc_chain_head_ref), *id_imprint) };

    unsafe {
        callback_desc.invoke(
            callback_ctx,
            callback_ret,
            &mut inner_alloc_scope as *mut _ as *mut (),
            // Safe, as this should only be triggered by foreign code, when the
            // only existing AccessScope<ID> is already borrowed by the
            // trampoline.
            &mut AccessScope::<ID>::new(*id_imprint) as *mut _ as *mut (),
        )
    };
}

// TODO: reason about aliasing of the MockRtAllocChain
extern "C" fn mock_rt_callback_trampoline<const CALLBACK_ID: usize, ID: OGID>(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> CallbackTrampolineFnReturn {
    let mut callback_ret = MockRtCallbackReturn {
        return_regs: [0; 2],
    };

    mock_rt_callback_dispatch::<ID>(
        CALLBACK_ID,
        &MockRtCallbackContext {
            arg_regs: [a0, a1, a2, a3, a4, a5],
        },
        &mut callback_ret,
    );

    CallbackTrampolineFnReturn {
        reg0: callback_ret.return_regs[0],
        reg1: callback_ret.return_regs[1],
    }
}

pub enum MockRtCallbackTrampolinePool<ID: OGID> {
    _Dummy(PhantomData<ID>, core::convert::Infallible),
}

impl<ID: OGID> MockRtCallbackTrampolinePool<ID> {
    const CALLBACKS: [CallbackTrampolineFn; 512] = seq_macro::seq!(N in 0..512 { [
	    #( mock_rt_callback_trampoline::<N, ID>, )*
	] });
}

pub enum MockRtAllocError {
    InvalidLayout,
}

pub trait MockRtAllocator {
    unsafe fn with_alloc<R, F: FnOnce(*mut ()) -> R>(
        &self,
        layout: core::alloc::Layout,
        f: F,
    ) -> Result<R, MockRtAllocError>;
}

pub struct MockRt<ID: OGID, A: MockRtAllocator> {
    zero_copy_immutable: bool,
    allocator: A,
    id_imprint: ID::Imprint,
}

impl<ID: OGID, A: MockRtAllocator> MockRt<ID, A> {
    pub unsafe fn new(
        zero_copy_immutable: bool,
        all_upgrades_valid: bool,
        allocator: A,
        branding: ID,
    ) -> (
        Self,
        AllocScope<'static, MockRtAllocChain<'static>, ID>,
        AccessScope<ID>,
    ) {
        (
            MockRt {
                zero_copy_immutable,
                allocator,
                id_imprint: branding.get_imprint(),
            },
            unsafe {
                AllocScope::new(
                    MockRtAllocChain::Base(all_upgrades_valid),
                    branding.get_imprint(),
                )
            },
            unsafe { AccessScope::new(branding.get_imprint()) },
        )
    }

    fn setup_callback_int<'a, C, F, R>(
        &self,
        callback: &'a mut C,
        alloc_scope: &mut AllocScope<
            '_,
            <Self as OGRuntime>::AllocTracker<'_>,
            <Self as OGRuntime>::ID,
        >,
        fun: F,
    ) -> OGResult<R>
    where
        C: FnMut(
            &<Self as OGRuntime>::CallbackContext,
            &mut <Self as OGRuntime>::CallbackReturn,
            *mut (),
            *mut (),
        ),
        F: for<'b> FnOnce(
            *const <Self as OGRuntime>::CallbackTrampolineFn,
            &'b mut AllocScope<'_, <Self as OGRuntime>::AllocTracker<'_>, <Self as OGRuntime>::ID>,
        ) -> R,
    {
        if self.id_imprint != alloc_scope.id_imprint() {
            return Err(OGError::IDMismatch);
        }

        struct Context<'a, ClosureTy> {
            closure: &'a mut ClosureTy,
        }

        unsafe extern "C" fn callback_wrapper<
            'a,
            ClosureTy: FnMut(&MockRtCallbackContext, &mut MockRtCallbackReturn, *mut (), *mut ()) + 'a,
        >(
            ctx_ptr: *mut c_void,
            callback_ctx: &MockRtCallbackContext,
            callback_ret: &mut MockRtCallbackReturn,
            alloc_scope: *mut (),
            access_scope: *mut (),
        ) {
            let ctx: &mut Context<'a, ClosureTy> =
                unsafe { &mut *(ctx_ptr as *mut Context<'a, ClosureTy>) };

            // For now, we assume that the functoin doesn't unwind:
            (ctx.closure)(callback_ctx, callback_ret, alloc_scope, access_scope)
        }

        // Ensure that the context pointer is compatible in size and
        // layout to a c_void pointer:
        assert_eq!(
            core::mem::size_of::<*mut c_void>(),
            core::mem::size_of::<*mut Context<'a, C>>()
        );
        assert_eq!(
            core::mem::align_of::<*mut c_void>(),
            core::mem::align_of::<*mut Context<'a, C>>()
        );

        let mut ctx: Context<'a, C> = Context { closure: callback };

        let callback_id = alloc_scope.tracker().next_callback_id();

        let mut inner_alloc_scope = unsafe {
            AllocScope::new(
                MockRtAllocChain::Callback(
                    callback_id,
                    MockRtCallbackDescriptor {
                        wrapper: callback_wrapper::<C>,
                        context: &mut ctx as *mut _ as *mut c_void,
                        _lt: PhantomData::<&'a mut c_void>,
                    },
                    alloc_scope.tracker(),
                ),
                alloc_scope.id_imprint(),
            )
        };

        let alloc_chain_head_ref_opt: &mut Option<(*const MockRtAllocChain<'_>, *const ())> =
            unsafe { &mut *core::ptr::addr_of_mut!(ACTIVE_ALLOC_CHAIN_HEAD_ROG) };

        // Implement a "stack" of ACTIVE_ALLOC_CHAIN_HEAD_ROGs, keeping the
        // previous value in a local variable. We'll put it back after running
        // our closure:
        let outer_alloc_chain_head_ref = alloc_chain_head_ref_opt.clone();

        // "Push" a new stack top, consisting of the new reference to the
        // inner_alloc_scope's tracker and an ID imprint value:
        let tracker = inner_alloc_scope.tracker() as *const _;
        let id_imprint: ID::Imprint = alloc_scope.id_imprint();
        *alloc_chain_head_ref_opt = Some((
            tracker as *const MockRtAllocChain<'static>,
            &id_imprint as *const ID::Imprint as *const (),
        ));

        let callback_trampoline = MockRtCallbackTrampolinePool::<ID>::CALLBACKS[callback_id];

        let res = fun(
            callback_trampoline as *const CallbackTrampolineFn,
            &mut inner_alloc_scope,
        );

        // Reset the alloc_chain_head_ref_opt to the previous value.
        let _inner_alloc_chain_head_ref =
            unsafe { core::ptr::replace(alloc_chain_head_ref_opt, outer_alloc_chain_head_ref) };

        // All the references of `_inner_alloc_chain_head_ref` are local to our
        // stack, so there's nothing we'd need to deallocate:
        Ok(res)
    }
}

#[derive(Clone, Debug)]
pub struct MockRtAllocation {
    ptr: *mut (),
    len: usize,
    mutable: bool,
}

impl MockRtAllocation {
    fn matches(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        (ptr as usize) >= (self.ptr as usize)
            && ((ptr as usize)
                .checked_add(len)
                .map(|end| end <= (self.ptr as usize) + self.len)
                .unwrap_or(false))
            && (!mutable || self.mutable)
    }
}

#[derive(Debug)]
pub struct MockRtCallbackDescriptor<'a> {
    wrapper: unsafe extern "C" fn(
        *mut c_void,
        &MockRtCallbackContext,
        &mut MockRtCallbackReturn,
        *mut (),
        *mut (),
    ),
    context: *mut c_void,
    _lt: PhantomData<&'a mut c_void>,
}

impl MockRtCallbackDescriptor<'_> {
    unsafe fn invoke(
        &self,
        callback_ctx: &MockRtCallbackContext,
        callback_ret: &mut MockRtCallbackReturn,
        alloc_scope: *mut (),
        access_scope: *mut (),
    ) {
        unsafe {
            (self.wrapper)(
                self.context,
                callback_ctx,
                callback_ret,
                alloc_scope,
                access_scope,
            )
        }
    }
}

#[derive(Debug)]
pub enum MockRtAllocChain<'a> {
    // Because the MockRt does not have insights into or control over
    // where the foreign library allocates, we allow disabling upgrade
    // checks. Otherwise, only stacked allocations can be upgraded.
    Base(bool),
    Allocation(MockRtAllocation, &'a MockRtAllocChain<'a>),
    Callback(
        usize,
        MockRtCallbackDescriptor<'a>,
        &'a MockRtAllocChain<'a>,
    ),
    Cons(&'a MockRtAllocChain<'a>),
}

struct MockRtAllocChainIter<'a>(Option<&'a MockRtAllocChain<'a>>);

impl<'a> Iterator for MockRtAllocChainIter<'a> {
    type Item = &'a MockRtAllocChain<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.0 {
            self.0 = match cur {
                MockRtAllocChain::Base(_) => None,
                MockRtAllocChain::Allocation(_, pred) => Some(pred),
                MockRtAllocChain::Callback(_, _, pred) => Some(pred),
                MockRtAllocChain::Cons(pred) => Some(pred),
            };

            Some(cur)
        } else {
            None
        }
    }
}

impl<'a> MockRtAllocChain<'a> {
    fn iter(&'a self) -> MockRtAllocChainIter<'a> {
        MockRtAllocChainIter(Some(self))
    }

    fn is_valid_int(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        self.iter().any(|elem| match elem {
            MockRtAllocChain::Base(all_upgrades_valid) => *all_upgrades_valid,
            MockRtAllocChain::Allocation(alloc, _) => alloc.matches(ptr, len, mutable),
            MockRtAllocChain::Callback(_, _, _) => false,
            MockRtAllocChain::Cons(_) => false,
        })
    }

    fn next_callback_id(&self) -> usize {
        self.iter()
            .find_map(|elem| match elem {
                MockRtAllocChain::Base(_) => None,
                MockRtAllocChain::Allocation(_, _) => None,
                MockRtAllocChain::Callback(id, _, _) => Some(id + 1),
                MockRtAllocChain::Cons(_) => None,
            })
            .unwrap_or(0)
    }

    fn find_callback_descriptor(&self, id: usize) -> Option<&MockRtCallbackDescriptor<'_>> {
        self.iter().find_map(|elem| match elem {
            MockRtAllocChain::Base(_) => None,
            MockRtAllocChain::Allocation(_, _) => None,
            MockRtAllocChain::Callback(desc_id, desc, _) => {
                if id == *desc_id {
                    Some(desc)
                } else {
                    None
                }
            }
            MockRtAllocChain::Cons(_) => None,
        })
    }
}

unsafe impl AllocTracker for MockRtAllocChain<'_> {
    fn is_valid(&self, ptr: *const (), len: usize) -> bool {
        self.is_valid_int(ptr as *mut (), len, false)
    }

    fn is_valid_mut(&self, ptr: *mut (), len: usize) -> bool {
        self.is_valid_int(ptr, len, true)
    }
}

unsafe impl<ID: OGID, A: MockRtAllocator> OGRuntime for MockRt<ID, A> {
    type ID = ID;
    type AllocTracker<'a> = MockRtAllocChain<'a>;
    type ABI = GenericABI;
    type CallbackTrampolineFn = CallbackTrampolineFn;
    type CallbackContext = MockRtCallbackContext;
    type CallbackReturn = MockRtCallbackReturn;

    type SymbolTableState<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize> = ();

    fn resolve_symbols<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        _symbol_table: &'static [&'static CStr; SYMTAB_SIZE],
        _fixed_offset_symbol_table: &'static [Option<&'static CStr>; FIXED_OFFSET_SYMTAB_SIZE],
    ) -> Option<Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>> {
        Some(())
    }

    fn lookup_symbol<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        _compact_symtab_index: usize,
        _fixed_offset_symtab_index: usize,
        _symtabstate: &Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>,
    ) -> Option<*const ()> {
        None
    }

    fn setup_callback<'a, C, F, R>(
        &self,
        callback: &'a mut C,
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
        ) -> R,
    {
        if self.id_imprint != alloc_scope.id_imprint() {
            return Err(OGError::IDMismatch);
        }

        let typecast_callback =
            &mut |callback_ctx: &MockRtCallbackContext,
                  callback_ret: &mut MockRtCallbackReturn,
                  alloc_scope_ptr: *mut (),
                  access_scope_ptr: *mut ()| {
                let alloc_scope = unsafe {
                    &mut *(alloc_scope_ptr as *mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>)
                };

                let access_scope =
                    unsafe { &mut *(access_scope_ptr as *mut AccessScope<Self::ID>) };

                callback(callback_ctx, callback_ret, alloc_scope, access_scope);
            };

        // We need to erase the type-dependence of the closure argument on `ID`,
        // as that creates life-time issues when the `MockRtAllocChain` is
        // parameterized over it:
        self.setup_callback_int(typecast_callback, alloc_scope, fun)
    }

    fn execute<R, F: FnOnce() -> R>(
        &self,
        _target_symbol: *const (),
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        f: F,
    ) -> R {
        if self.id_imprint != alloc_scope.id_imprint()
            || self.id_imprint != access_scope.id_imprint()
        {
            panic!(
                "ID mismatch! Rt: {:?}, AllocScope: {:?}, AccessScope: {:?}",
                self.id_imprint,
                alloc_scope.id_imprint(),
                access_scope.id_imprint(),
            );
        }

        f()
    }

    fn allocate_stacked_untracked_mut<F, R>(
        &self,
        layout: core::alloc::Layout,
        fun: F,
    ) -> OGResult<R>
    where
        F: FnOnce(*mut ()) -> R,
    {
        // Simply proxy this to our underlying allocator:
        (unsafe { self.allocator.with_alloc(layout, fun) }).map_err(|e| match e {
            MockRtAllocError::InvalidLayout => OGError::AllocInvalidLayout,
        })
    }

    fn allocate_stacked_mut<F, R>(
        &self,
        layout: core::alloc::Layout,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(*mut (), &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>) -> R,
    {
        if self.id_imprint != alloc_scope.id_imprint() {
            return Err(OGError::IDMismatch);
        }

        self.allocate_stacked_untracked_mut(layout, move |ptr| {
            // Create a new AllocScope instance that wraps a new allocation
            // tracker `Cons` list element that points to this allocation, and
            // its predecessors:
            let mut inner_alloc_scope = unsafe {
                AllocScope::new(
                    MockRtAllocChain::Allocation(
                        MockRtAllocation {
                            ptr,
                            len: layout.size(),
                            mutable: true,
                        },
                        alloc_scope.tracker(),
                    ),
                    alloc_scope.id_imprint(),
                )
            };

            // Hand a temporary mutable reference to this new scope to the
            // closure.
            //
            // We thus not only allocate, but also track allocations themselves
            // on the stack, and there is nothing to clean up! The new
            // `inner_alloc_scope` will simply go out of scope at the end of
            // this closure.
            fun(ptr, &mut inner_alloc_scope)
        })
    }

    fn allocate_stacked_t_mut<T: Sized + 'static, F, R>(
        &self,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(
            OGMutRef<'b, Self::ID, T>,
            &'b mut AllocScope<'b, Self::AllocTracker<'b>, Self::ID>,
        ) -> R,
    {
        if self.id_imprint != alloc_scope.id_imprint() {
            return Err(OGError::IDMismatch);
        }

        let t = UnsafeCell::new(MaybeUninit::<T>::uninit());

        // Create a new AllocScope instance that wraps a new allocation
        // tracker `Cons` list element that points to this allocation, and
        // its predecessors:
        let mut inner_alloc_scope = unsafe {
            AllocScope::new(
                MockRtAllocChain::Allocation(
                    MockRtAllocation {
                        ptr: &t as *const _ as *const _ as *mut _,
                        len: core::mem::size_of::<T>(),
                        mutable: true,
                    },
                    alloc_scope.tracker(),
                ),
                alloc_scope.id_imprint(),
            )
        };

        // Hand a temporary mutable reference to this new scope to the
        // closure.
        //
        // We thus not only allocate, but also track allocations themselves
        // on the stack, and there is nothing to clean up! The new
        // `inner_alloc_scope` will simply go out of scope at the end of
        // this closure.
        Ok(fun(
            unsafe {
                OGMutRef::upgrade_from_ptr_unchecked(
                    &t as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T,
                    alloc_scope.id_imprint(),
                )
            },
            &mut inner_alloc_scope,
        ))
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
        if self.id_imprint != alloc_scope.id_imprint()
            || self.id_imprint != access_scope.id_imprint()
        {
            return Err(OGError::IDMismatch);
        }

        if self.zero_copy_immutable {
            // We can't wrap `write_stacked_ref_t` here, as our `T: ?Copy`.

            // While there are no guarantees that foreign code will uphold to
            // the immutability requirement with the MockRt, we still don't use
            // interior mutability here. This more closely simulates what a
            // proper runtime with memory protection would do.
            //
            // The soundness of this depends on whether the foreign code is
            // well-behaved, and whether the bindings correctly pass these
            // pointers *const arguments:
            let stored = t;

            // Create a new AllocScope instance that wraps a new allocation
            // tracker `Cons` list element that points to this allocation, and
            // its predecessors:
            let mut inner_alloc_scope = unsafe {
                AllocScope::new(
                    MockRtAllocChain::Allocation(
                        MockRtAllocation {
                            ptr: &stored as *const _ as *const _ as *mut _,
                            len: core::mem::size_of::<T>(),
                            mutable: false,
                        },
                        alloc_scope.tracker(),
                    ),
                    alloc_scope.id_imprint(),
                )
            };

            // Hand a temporary immutable reference to this new scope to the
            // closure.
            //
            // We thus not only allocate, but also track allocations themselves
            // on the stack, and there is nothing to clean up! The new
            // `inner_alloc_scope` will simply go out of scope at the end of
            // this closure.
            Ok(fun(
                unsafe {
                    OGRef::upgrade_from_ptr_unchecked(
                        &stored as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T,
                        alloc_scope.id_imprint(),
                    )
                },
                &mut inner_alloc_scope,
                access_scope,
            ))
        } else {
            // Fall back onto default behavior:
            self.write_stacked_t_mut(
                t,
                alloc_scope,
                access_scope,
                |allocation, new_alloc_scope, new_access_scope| {
                    fun(allocation.as_immut(), new_alloc_scope, new_access_scope)
                },
            )
        }
    }

    fn write_stacked_ref_t<T: Sized + Copy + 'static, F, R>(
        &self,
        t: &T,
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
        if self.id_imprint != alloc_scope.id_imprint()
            || self.id_imprint != access_scope.id_imprint()
        {
            return Err(OGError::IDMismatch);
        }

        if self.zero_copy_immutable {
            // For safety considerations, see `write_stacked_t`.

            // Create a new AllocScope instance that wraps a new allocation
            // tracker `Cons` list element that points to this allocation, and
            // its predecessors:
            let mut inner_alloc_scope = unsafe {
                AllocScope::new(
                    MockRtAllocChain::Allocation(
                        MockRtAllocation {
                            ptr: t as *const _ as *const _ as *mut _,
                            len: core::mem::size_of::<T>(),
                            mutable: false,
                        },
                        alloc_scope.tracker(),
                    ),
                    alloc_scope.id_imprint(),
                )
            };

            // Hand a temporary immutable reference to this new scope to the
            // closure.
            //
            // We thus not only allocate, but also track allocations themselves
            // on the stack, and there is nothing to clean up! The new
            // `inner_alloc_scope` will simply go out of scope at the end of
            // this closure.
            Ok(fun(
                unsafe {
                    OGRef::upgrade_from_ptr_unchecked(
                        t as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T,
                        alloc_scope.id_imprint(),
                    )
                },
                &mut inner_alloc_scope,
                access_scope,
            ))
        } else {
            // Fall back onto default behavior:
            self.write_stacked_ref_t_mut(
                t,
                alloc_scope,
                access_scope,
                |allocation, new_alloc_scope, new_access_scope| {
                    fun(allocation.as_immut(), new_alloc_scope, new_access_scope)
                },
            )
        }
    }

    fn write_stacked_slice<T: Sized + Copy + 'static, F, R>(
        &self,
        src: &[T],
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        access_scope: &mut AccessScope<Self::ID>,
        fun: F,
    ) -> OGResult<R>
    where
        F: for<'b> FnOnce(
            OGSlice<'_, Self::ID, T>,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &'b mut AccessScope<Self::ID>,
        ) -> R,
    {
        if self.id_imprint != alloc_scope.id_imprint()
            || self.id_imprint != access_scope.id_imprint()
        {
            return Err(OGError::IDMismatch);
        }

        if self.zero_copy_immutable {
            // For safety considerations, see `write_stacked_t`.

            // Create a new AllocScope instance that wraps a new allocation
            // tracker `Cons` list element that points to this allocation, and
            // its predecessors:
            let mut inner_alloc_scope = unsafe {
                AllocScope::new(
                    MockRtAllocChain::Allocation(
                        MockRtAllocation {
                            ptr: src as *const _ as *const _ as *mut _,
                            len: core::mem::size_of::<T>() * src.len(),
                            mutable: false,
                        },
                        alloc_scope.tracker(),
                    ),
                    alloc_scope.id_imprint(),
                )
            };

            // Hand a temporary immutable reference to this new scope to the
            // closure.
            //
            // We thus not only allocate, but also track allocations themselves
            // on the stack, and there is nothing to clean up! The new
            // `inner_alloc_scope` will simply go out of scope at the end of
            // this closure.
            Ok(fun(
                unsafe {
                    OGSlice::upgrade_from_ptr_unchecked(
                        src as *const _ as *mut UnsafeCell<MaybeUninit<T>> as *mut T,
                        src.len(),
                        alloc_scope.id_imprint(),
                    )
                },
                &mut inner_alloc_scope,
                access_scope,
            ))
        } else {
            // Fall back onto default behavior:
            self.write_stacked_slice_from_iter(src.iter().copied(), alloc_scope, access_scope, fun)
        }
    }
}

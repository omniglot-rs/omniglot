use crate::OGResult;
use crate::foreign_memory::og_copy::OGCopy;
use crate::rt::OGRuntime;

pub unsafe trait SysVAMD64InvokeRes<RT: SysVAMD64BaseRt, T: Sized> {
    fn new() -> Self;

    fn into_result_registers(self, rt: &RT) -> OGResult<OGCopy<T>>;
    unsafe fn into_result_stacked(self, rt: &RT, stacked_res: *mut T) -> OGResult<OGCopy<T>>;
}

pub trait SysVAMD64BaseRt: OGRuntime<ABI = crate::abi::sysv_amd64::SysVAMD64ABI> + Sized {
    type InvokeRes<T>: SysVAMD64InvokeRes<Self, T>;
}

pub trait SysVAMD64Rt<const STACK_SPILL: usize, RTLOC: crate::abi::calling_convention::ArgumentSlot>:
    SysVAMD64BaseRt
{
    unsafe extern "C" fn invoke();
}

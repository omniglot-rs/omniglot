// ABI

pub enum SysVAMD64ABI {}
impl super::OGABI for SysVAMD64ABI {}

macro_rules! sysv_amd64_areg_impl {
    ($reg:ident, $name:expr) => {
        impl super::calling_convention::ArgumentSlot
            for super::calling_convention::$reg<SysVAMD64ABI>
        {
            const IS_REG: bool = true;
            const IS_STACKED: bool = false;
            const IS_INVALID: bool = false;
            const REG_NAME: &'static str = $name;
            const STACK_OFFSET_WORDS: usize = usize::MAX;
        }
    };
}

sysv_amd64_areg_impl!(AREG0, "rdi");
sysv_amd64_areg_impl!(AREG1, "rsi");
sysv_amd64_areg_impl!(AREG2, "rdx");
sysv_amd64_areg_impl!(AREG3, "rcx");
sysv_amd64_areg_impl!(AREG4, "r8");
sysv_amd64_areg_impl!(AREG5, "r9");

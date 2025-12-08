// -*- fill-column: 80; -*-

// ABI

pub enum Rv32iCABI {}
impl super::OGABI for Rv32iCABI {}

macro_rules! rv32i_c_areg_impl {
    ($reg:ident, $name:expr) => {
        impl super::calling_convention::ArgumentSlot
            for super::calling_convention::$reg<Rv32iCABI>
        {
            const IS_REG: bool = true;
            const IS_STACKED: bool = false;
            const IS_INVALID: bool = false;
            const REG_NAME: &'static str = $name;
            const STACK_OFFSET_WORDS: usize = usize::MAX;
        }
    };
}

rv32i_c_areg_impl!(AREG0, "a0");
rv32i_c_areg_impl!(AREG1, "a1");
rv32i_c_areg_impl!(AREG2, "a2");
rv32i_c_areg_impl!(AREG3, "a3");
rv32i_c_areg_impl!(AREG4, "a4");
rv32i_c_areg_impl!(AREG5, "a5");
rv32i_c_areg_impl!(AREG6, "a6");
rv32i_c_areg_impl!(AREG7, "a7");

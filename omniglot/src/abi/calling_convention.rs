use core::convert::Infallible;
use core::marker::PhantomData;

pub trait ArgumentSlot {
    const IS_REG: bool;
    const IS_STACKED: bool;
    const IS_INVALID: bool;
    const REG_NAME: &'static str;
    const STACK_OFFSET_WORDS: usize;
}

pub enum Invalid {}
impl ArgumentSlot for Invalid {
    const IS_REG: bool = false;
    const IS_STACKED: bool = false;
    const IS_INVALID: bool = true;
    const REG_NAME: &'static str = "invalid";
    const STACK_OFFSET_WORDS: usize = usize::MAX;
}

pub enum Stacked<const OFFSET: usize, ABI: super::OGABI> {
    _Impossible(Infallible, PhantomData<ABI>),
}
impl<const OFFSET: usize, ABI: super::OGABI> ArgumentSlot for Stacked<OFFSET, ABI> {
    const IS_REG: bool = false;
    const IS_STACKED: bool = true;
    const IS_INVALID: bool = false;
    const REG_NAME: &'static str = "stacked";
    const STACK_OFFSET_WORDS: usize = OFFSET;
}

// ---------- Register types ---------------------------------------------------

macro_rules! register_type_def {
    ($name:ident) => {
	pub enum $name<ABI: super::OGABI> {
	    _Impossible(Infallible, PhantomData<ABI>)
	}
    };

    ($name:ident, $($rest:ident),* $(,)?) => {
	register_type_def!($name);
	register_type_def!($($rest),*);
    };
}

#[rustfmt::skip]
register_type_def![
    AREG0, AREG1, AREG2, AREG3, AREG4, AREG5, AREG6, AREG7,
    AREG8, AREG9, AREG10, AREG11, AREG12, AREG13, AREG14, AREG15,
    AREG16, AREG17, AREG18, AREG19, AREG20, AREG21, AREG22, AREG23,
    AREG24, AREG25, AREG26, AREG27, AREG28, AREG29, AREG30, AREG31,
];

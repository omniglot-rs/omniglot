//! Definition of the [`BitPatternValidate`] trait and implementations
//! for primitive Rust types.

/// A trait for validating types based on their underlying bit-pattern.
///
/// When this trait is implemented for a type, it indicates that there exists a
/// way to validate that arbitrary, initialized, and stable (meaning it is not
/// volatile, and will not be modified for the duration of the call to
/// [`validate`](BitPatternValidate::validate)) memory holds a valid instance of
/// that type.
///
/// Importantly, this trait must not be implemented when there exist any
/// higher-level language-level or correctness invariants imposed on a type. For
/// instance, a Rust reference type can never be validated solely by examining
/// its underlying pointer-value. Similarly, a typestate type may not be
/// validated if it cannot be constructed in safe Rust, out of a vaccum, without
/// any side effects and with any value that is deemed valid by
/// [`validate`](BitPatternValidate::validate).
///
/// Examples of types that can be validated are:
///
/// - `()` (the unit type): can be freely constructed, without any side-effects,
///   in a vaccum in safe Rust. There are no high-level language or correctness
///   invariants imposed on this type. It is zero-sized and does not occupy any
///   memory, and hence is unconditionally valid.
///
/// - `usize`: similar to `()`, any `usize` value can be freely constructed,
///   without any side-effects, in a vaccum in safe Rust. Furthermore, any value
///   that occupies a `core::mem::size_of::<usize>()` bytes region in memory is
///   a valid value of the `usize` type. Thus is it unconditionally valid.
///
/// - `bool`: can be freely constructed, without any side-effects, in a vaccum
///   in safe Rust. However, its only valid members are `true` (represented as a
///   one-byte numeric value `1`) and `false` (represented as a one-byte numeric
///   value `0`). Thus its validity depends on whether the underlying memory
///   location contains a value in `[0; 1]` or not.
pub unsafe trait BitPatternValidate {
    /// Validate that the memory behind the pointer `*const Self` contains a
    /// valid instance of `Self`.
    ///
    /// # Safety
    ///
    /// Callers must guarantee that `t: *const Self` points to a readable,
    /// initialized (i.e., stable) memory allocation with size and alignment
    /// matching that of `Self`. This memory allocation must remain readable and
    /// stable over the duration of this call to `validate`; concurrent
    /// mutations of the memory are not permitted.
    ///
    /// As per the [trait documentation](BitPatternValidate), this trait must
    /// not be implemented for any types that have language-level soundness
    /// requirements or higher-level correctness guarantees.
    unsafe fn validate(t: *const Self) -> bool;
}

/// Validating an array requires validating of every element.
unsafe impl<const N: usize, T: BitPatternValidate> BitPatternValidate for [T; N] {
    unsafe fn validate(array: *const Self) -> bool {
        // The array must have been validated to be well-aligned and
        // accessible. It must further be smaller than or equal to
        // isize::MAX in size, or otherwise the `.add` method invocation may
        // be unsound.
        //
        // This cast is important here. Otherwise we'd be stepping over the
        // array in its entirety and recursively calling this validate
        // function.
        let mut elem = array as *const T;

        // We'd like to use a Range<*mut T>.all() here, but `*mut T` does
        // not implement `core::iter::Step`...
        for _i in 0..N {
            // # Safety
            //
            // TODO
            if !(unsafe { BitPatternValidate::validate(elem) }) {
                // Abort on the first invalid element.
                return false;
            }

            // # Safety
            //
            // TODO
            elem = unsafe { elem.add(1) };
        }

        // We iterated over the entire array and validated every single
        // element, to the entire array is valid:
        true
    }
}

macro_rules! unconditionally_valid {
    // Attempt to try to support generic arguments, does not work:
    //
    // ($( #[ $attrs:tt ] )* for<$( $generics:ty ),*> $( $target:tt )*) => {
    //     $( #[ $attrs ] )*
    //     unsafe impl<$( $generics ),*> ::encapfn::types::BitPatternValidate for $( $target )* {
    // 	fn validate(_t: *const Self) -> bool {
    // 	    // Unconditionally valid:
    // 	    true
    // 	}
    //     }
    // }

    // Non-generic:
    ($( #[ $( $attrs:tt )* ] )* $target:ty) => {
	/// Unconditionally valid type.
	///
	/// As long as the memory backing this type is accessible,
	/// well-aligned and conforms to Rust's aliasing requirements, we
	/// can assume it to be valid without reading back its memory.
	$( #[ $( $attrs )* ] )*
	unsafe impl crate::bit_pattern_validate::BitPatternValidate for $target {
	    unsafe fn validate(_t: *const Self) -> bool {
		// Unconditionally valid:
		true
	    }
	}
    }
}

/// Accessing a raw pointer only requires that the pointer's numeric value
/// is itself readable. Rust places no other restrictions on references to
/// raw pointers. This does not mean that the resulting pointer is
/// well-aligned, or safely dereferencable.
unsafe impl<T> BitPatternValidate for *mut T {
    unsafe fn validate(_t: *const Self) -> bool {
        // Well-aligned and accessible pointer values are unconditionally
        // valid:
        true
    }
}

/// See the documentation for [`*mut T as BitPatternValidate`].
unsafe impl<T> BitPatternValidate for *const T {
    unsafe fn validate(_t: *const Self) -> bool {
        // Well-aligned and accessible pointer values are unconditionally
        // valid:
        true
    }
}

// Implementations for primitives. We would like to implement these on the
// `std::ffi::c_*` type aliases instead, but those are platform dependent
// and may produce conflicting implementations. Hence we use Rust's
// primitives, which the `std::ffi::c_*` type aliases point to, but for
// which we can guarantee uniqueness:
unconditionally_valid!(u8);
unconditionally_valid!(u16);
unconditionally_valid!(u32);
unconditionally_valid!(u64);
unconditionally_valid!(u128);
unconditionally_valid!(usize);

unconditionally_valid!(i8);
unconditionally_valid!(i16);
unconditionally_valid!(i32);
unconditionally_valid!(i64);
unconditionally_valid!(i128);
unconditionally_valid!(isize);

unconditionally_valid!(f32);
unconditionally_valid!(f64);

unconditionally_valid!(());

unsafe impl BitPatternValidate for bool {
    unsafe fn validate(t: *const Self) -> bool {
        // Ensure that the integer type we load instead has an
        // equivalent layout:
        assert!(core::mem::size_of::<bool>() == core::mem::size_of::<u8>());
        assert!(core::mem::align_of::<bool>() == core::mem::align_of::<u8>());

        // Load the value as an integer and check that it is
        // within the range of valid boolean values:
        //
        // # Safety
        //
        // TODO!
        (unsafe { core::ptr::read(t as *const u8) }) < 2
    }
}

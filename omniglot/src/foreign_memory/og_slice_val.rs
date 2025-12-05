use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::Deref;

use crate::id::OGID;

pub struct OGSliceVal<'alloc, 'access, ID: OGID, T: 'static> {
    r: &'access [MaybeUninit<T>],
    _id_imprint: ID::Imprint,
    _alloc_lt: PhantomData<&'alloc [T]>,
}

impl<'alloc, 'access, ID: OGID, T: 'static> OGSliceVal<'alloc, 'access, ID, T> {
    pub(crate) unsafe fn new(r: &'access [MaybeUninit<T>], id_imprint: ID::Imprint) -> Self {
        OGSliceVal {
            r,
            _id_imprint: id_imprint,
            _alloc_lt: PhantomData,
        }
    }
}

impl<'alloc, 'access, ID: OGID, T: 'static> Deref
    for OGSliceVal<'alloc, 'access, ID, T>
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute::<&[MaybeUninit<T>], &[T]>(&self.r) }
    }
}

// -*- fill-column: 80; -*-

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::Deref;

use crate::id::OGID;

use super::og_mut_ref::OGMutRef;
use super::og_ref::OGRef;

pub struct OGVal<'alloc, 'access, ID: OGID, T: ?Sized> {
    r: &'access T,
    id_imprint: ID::Imprint,
    _alloc_lt: PhantomData<&'alloc T>,
}

impl<'alloc, 'access, ID: OGID, T: ?Sized> OGVal<'alloc, 'access, ID, T> {
    pub(crate) unsafe fn new(r: &'access T, id_imprint: ID::Imprint) -> Self {
        OGVal {
            r,
            id_imprint,
            _alloc_lt: PhantomData,
        }
    }
}

impl<'alloc, 'access, ID: OGID, T> OGVal<'alloc, 'access, ID, T> {
    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }

    pub fn as_ref(&self) -> OGRef<'alloc, ID, T> {
        unsafe {
            OGRef::new(
                &*(self.r as *const _ as *const UnsafeCell<MaybeUninit<T>>),
                self.id_imprint(),
            )
        }
    }

    pub fn as_mut(&self) -> OGMutRef<'alloc, ID, T> {
        unsafe {
            OGMutRef::new(
                &*(self.r as *const _ as *const UnsafeCell<MaybeUninit<T>>),
                self.id_imprint(),
            )
        }
    }
}

impl<'alloc, 'access, ID: OGID, T: ?Sized> Deref for OGVal<'alloc, 'access, ID, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.r
    }
}

impl<'alloc, 'access, ID: OGID, T> Clone for OGVal<'alloc, 'access, ID, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'alloc, 'access, ID: OGID, T> Copy for OGVal<'alloc, 'access, ID, T> {}

impl<'alloc, 'access, const N: usize, ID: OGID, T> OGVal<'alloc, 'access, ID, [T; N]> {
    pub fn as_array(&self) -> &[OGVal<'alloc, 'access, ID, T>; N] {
        unsafe {
            core::mem::transmute::<
                &OGVal<'alloc, 'access, ID, [T; N]>,
                &[OGVal<'alloc, 'access, ID, T>; N],
            >(&self)
        }
    }
}

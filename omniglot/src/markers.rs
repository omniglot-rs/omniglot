use crate::alloc_tracker::AllocTracker;
use crate::id::OGID;

use core::marker::PhantomData;

pub struct AllocScope<'a, T: AllocTracker, ID: OGID> {
    tracker: T,
    id_imprint: ID::Imprint,
    _lt: PhantomData<&'a ()>,
}

impl<'a, T: AllocTracker, ID: OGID> AllocScope<'a, T, ID> {
    pub unsafe fn new(tracker: T, id_imprint: ID::Imprint) -> Self {
        AllocScope {
            tracker,
            id_imprint,
            _lt: PhantomData,
        }
    }

    pub fn tracker(&self) -> &T {
        &self.tracker
    }

    pub fn tracker_mut(&mut self) -> &mut T {
        &mut self.tracker
    }

    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }
}

pub struct AccessScope<ID: OGID> {
    id_imprint: ID::Imprint,
}

impl<ID: OGID> AccessScope<ID> {
    pub unsafe fn new(id_imprint: ID::Imprint) -> Self {
        AccessScope { id_imprint }
    }

    pub fn id_imprint(&self) -> ID::Imprint {
        self.id_imprint
    }
}

use std::marker::PhantomData;
use web_sys::IdbIndex;

pub struct Index<Err> {
    sys: IdbIndex,
    _phantom: PhantomData<Err>,
}

impl<Err> Index<Err> {
    pub(crate) fn from_sys(sys: IdbIndex) -> Index<Err> {
        Index {
            sys,
            _phantom: PhantomData,
        }
    }
}

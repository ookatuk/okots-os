use num_traits::{Unsigned, PrimInt};

#[derive(Debug, Clone)]
pub struct MemMap<T: Unsigned + PrimInt = u64> {
    pub start: T,
    pub end: T,
}

impl<T: Unsigned + PrimInt> From<MemData<T>> for MemMap<T> {
    fn from(value: MemData<T>) -> Self {
        Self {
            start: value.start,
            end: value.start + value.len,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemData<T: Unsigned + PrimInt = u64> {
    pub start: T,
    pub len: T,
}

impl<T: Unsigned + PrimInt> From<MemMap<T>> for MemData<T> {
    fn from(value: MemMap<T>) -> Self {
        Self {
            start: value.start,
            len: value.end - value.start,
        }
    }
}
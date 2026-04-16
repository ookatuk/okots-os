use core::alloc::{GlobalAlloc, Layout};
use core::hint::unlikely;
use core::ops::{Add, Div, Rem, Sub};
use num_traits::{FromPrimitive, ToPrimitive, Unsigned, Zero};

pub trait CanRangeData:
Add<Output = Self> +
Sub<Output = Self> +
Ord +
PartialOrd +
Copy +
ToPrimitive +
FromPrimitive +
Unsigned
{}

impl CanRangeData for usize {}
impl CanRangeData for u8 {}
impl CanRangeData for u16 {}
impl CanRangeData for u32 {}
impl CanRangeData for u64 {}
impl CanRangeData for u128 {}

#[derive(PartialEq, Eq)]
pub struct SmartPtr<'a, DT, GA>
where
    DT: CanRangeData + Rem<Output = DT>,
    GA: GlobalAlloc + 'a,
{
    pub range: MemRangeData<DT>,
    pub alloc: &'a GA,
    align: usize,
}

impl<'a, DT, GA> SmartPtr<'a, DT, GA> where
    DT: CanRangeData + Div<Output = DT> + Rem<Output = DT>,
    GA: GlobalAlloc,
{
    pub fn new(ptr: usize, layout: Layout, alloc: &'a GA) -> Option<Self> {
        if unlikely(ptr.is_zero() || layout.size().is_zero()) {
            return None;
        }

        let range = MemRangeData {
            start: DT::from_usize(ptr)?,
            len: DT::from_usize(layout.size())?,
        };

        Some(Self {
            range,
            alloc,
            align: layout.align(),
        })
    }

    #[inline]
    pub const fn get_addr(&self) -> DT {
        self.range.start()
    }

    #[inline]
    pub fn get_ptr<TY>(&self) -> Option<*const TY> {
        Some(self.range.start().to_usize()? as *const TY)
    }

    pub fn get_slice<TY>(&self) -> Option<&[TY]> {
        let tsiz_raw = size_of::<TY>();
        if tsiz_raw == 0 {
            return None;
        }

        let size = DT::from_usize(tsiz_raw)?;
        let zero = DT::from_u64(0).unwrap();

        if self.range.len() % size != zero {
            return None;
        }

        let count = self.range.len() / size;

        unsafe {
            Some(core::slice::from_raw_parts(
                self.range.start().to_usize()? as *const TY,
                count.to_usize()?
            ))
        }
    }

    #[inline]
    pub fn get_mut_ptr<TY>(&mut self) -> *mut TY {
        self.range.start().to_usize().unwrap() as *mut TY
    }

    pub fn get_mut_slice<TY>(&mut self) -> Option<&mut [TY]> {
        let tsiz_raw = size_of::<TY>();
        if tsiz_raw == 0 {
            return None;
        }

        let size = DT::from_usize(tsiz_raw)?;
        let zero = DT::from_u64(0).unwrap();

        if self.range.len() % size != zero {
            return None;
        }

        let count = self.range.len() / size;

        unsafe {
            Some(core::slice::from_raw_parts_mut(
                self.range.start().to_usize()? as *mut TY,
                count.to_usize()?
            ))
        }
    }
}

impl<DT, GA> Drop for SmartPtr<'_, DT, GA> where
    DT: CanRangeData + Div<Output = DT> + Rem<Output = DT>,
    GA: GlobalAlloc,
{
    fn drop(&mut self) {
        let size = self.range.len().to_usize().unwrap();
        let align = self.align;

        unsafe {
            self.alloc.dealloc(
                self.get_mut_ptr::<u8>(),
                Layout::from_size_align_unchecked(size, align)
            );
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MemRangeData<T> where
    T: CanRangeData
{
    start: T,
    len: T
}

impl<T> MemRangeData<T> where
    T: CanRangeData
{
    #[inline]
    pub const fn new(start: T, len: T) -> MemRangeData<T> {
        MemRangeData {
            start,
            len
        }
    }

    #[inline]
    pub fn new_start_end(start: T, end: T) -> Option<MemRangeData<T>> {
        if unlikely(end < start) {
            return None;
        }

        Some(MemRangeData {
            start,
            len: end - start
        })
    }

    #[inline]
    pub const fn len(&self) -> T {
        self.len
    }

    #[inline]
    pub const fn start(&self) -> T {
        self.start
    }

    #[inline]
    pub fn end(&self) -> T {
        self.start + self.len
    }

    #[inline]
    pub fn set_start(&mut self, start: T) {
        self.start = start;
    }

    #[inline]
    pub fn set_end(&mut self, end: T) -> bool {
        if unlikely(end < self.start) {
            return false;
        }
        self.len = end - self.start;
        true
    }

    #[inline]
    pub fn set_len(&mut self, len: T) {
        self.len = len;
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicIsize, Ordering};
    use super::*;

    #[test]
    fn mem_range_data_test_1() {
        let data: MemRangeData<u64> = MemRangeData::new(50, 60);
        let data2: MemRangeData<u64> = MemRangeData::new_start_end(50, 110).expect("data is good. but returned none.");

        assert_eq!(data.len(), 60, "basic fails(1)");
        assert_eq!(data.start(), 50, "basic fails(2)");
        assert_eq!(data.end(), 110, "basic fails(3)");

        assert_eq!(data2, data, "this is eq, but shown not eq");

        let fail: Option<MemRangeData<u64>> = MemRangeData::new_start_end(50, 40);
        assert!(fail.is_none(), "data is invalid, but not error raised.");
    }

    #[derive(Default)]
    struct TmpAlloc {
        pub remaining_count: AtomicIsize,
    }

    unsafe impl GlobalAlloc for TmpAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.remaining_count.fetch_add(1, Ordering::SeqCst);
            unsafe{alloc::alloc::alloc(layout)}
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            self.remaining_count.fetch_sub(1, Ordering::SeqCst);
            unsafe{alloc::alloc::dealloc(ptr, layout)}
        }
    }

    #[test]
    fn smart_ptr() {
        let alloc = TmpAlloc::default();

        {
            let layout = Layout::from_size_align(64, 16).unwrap();
            let allocated = unsafe{alloc.alloc(layout)};
            {
                let tmp = allocated as *mut u64;
                unsafe {
                    for i in 0..(64 / 8) {
                        tmp.add(i).write(i as u64);
                    }
                }
            }

            assert!(!allocated.is_null(), "failed to allocate memory");

            let smart_ptr = SmartPtr::<u64, TmpAlloc>::new(allocated.addr(), layout, &alloc);

            assert!(smart_ptr.is_some(), "SmartPtr::new() broken.");
            let smart_ptr = smart_ptr.unwrap();

            let slice = smart_ptr.get_slice::<u64>();

            assert!(slice.is_some(), "slice system is broken.");

            let slice = slice.unwrap();

            for (l, i) in slice.iter().enumerate() {
                assert_eq!(*i, l as u64, "data broken/invalid ptr");
            }
        }

        assert_eq!(alloc.remaining_count.load(Ordering::SeqCst), 0, "memory leak detected.");
    }
}
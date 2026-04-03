use proc_bitfield::bitfield;
use alloc::borrow::Cow;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::hint::{unlikely};
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::null_mut;

const STRUCT_VER: u16 = 1;

bitfield! {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct DataTag(u8) {
        pub borrow: bool @ 0,
        pub is_static: bool @ 1,
        pub is_mut: bool @ 2,
    }
}

impl DataTag {
    pub const fn empty() -> Self { Self(0) }
    pub const fn as_static_borrow(self) -> Self {
        Self(self.0 | (1 << 1) | (1 << 0))
    }
    pub const fn as_borrow(self) -> Self {
        Self(self.0 | (1 << 0))
    }
    pub const fn as_mut(self) -> Self {
        Self(self.0 | (1 << 2))
    }
}

#[repr(C)]
/// 構造体情報
/// 関数の実行中のみ生きてる想定
/// あと普通に破壊可能っていう想定
pub struct Data<'a, T> {
    pub ver: u16,
    pub ptr: *mut T,
    pub len: u64,
    pub cap: u64,
    pub tag: DataTag,
    _marker: core::marker::PhantomData<&'a T>,

    // ライフタイムはこの構造体にあるため、
    // この構造体の移動をできるだけ禁止することで
    // 実質的にライフタイムを制限させる
    _pin: PhantomPinned,
}

impl<'a, T> Data<'a, T> {
    pub const fn new_static(data: &'static [T]) -> Data<'a, T> {
        Data {
            ver: STRUCT_VER,
            ptr: data.as_ptr() as *mut T,
            len: data.len() as u64,
            cap: data.len() as u64,
            tag: DataTag::empty().as_static_borrow(),
            _marker: core::marker::PhantomData,
            _pin: PhantomPinned,
        }
    }

    pub const fn new_static_mut(data: &'static mut [T]) -> Data<'a, T> {
        Data {
            ver: STRUCT_VER,
            ptr: data.as_ptr() as *mut T,
            len: data.len() as u64,
            cap: data.len() as u64,
            tag: DataTag::empty().as_static_borrow().as_mut(),
            _marker: core::marker::PhantomData,
            _pin: PhantomPinned,
        }
    }


    pub fn new_owned(mut v: Vec<T>) -> Data<'a, T> {
        let mut v = core::mem::ManuallyDrop::new(v);
        Data {
            ver: STRUCT_VER,
            ptr: v.as_mut_ptr(),
            len: v.len() as u64,
            cap: v.capacity() as u64,
            tag: DataTag::empty(),
            _marker: core::marker::PhantomData,
            _pin: PhantomPinned,
        }
    }

    pub const fn new_borrow(data: &'a [T]) -> Data<'a, T> {
        Data {
            ver: STRUCT_VER,
            ptr: data.as_ptr() as *mut T,
            len: data.len() as u64,
            cap: data.len() as u64,
            tag: DataTag(1 << 0),
            _marker: core::marker::PhantomData,
            _pin: PhantomPinned,
        }
    }

    fn get_raw_ptr(self: Pin<&mut Self>) -> Option<*mut T> {
        if unlikely(self.ptr.is_null() || self.len > isize::MAX as u64 || !self.ptr.is_aligned()) {
            return None;
        }

        let ptr = self.ptr;

        unsafe {
            let this = self.get_unchecked_mut();
            this.ptr = null_mut();
        }

        Some(ptr)
    }

    pub unsafe fn get_slice_static(mut self: Pin<&mut Self>) -> Option<&'static [T]> where T: 'static {
        if unlikely(!self.tag.is_static()) {
            return None;
        }

        Self::get_raw_ptr(self.as_mut()).map(|p| unsafe { core::slice::from_raw_parts(p, self.len as usize) })
    }

    pub unsafe fn get_slice_static_mut(mut self: Pin<&mut Self>) -> Option<&'static mut [T]> where T: 'static {
        if unlikely(!self.tag.is_static() || !self.tag.is_mut()) {
            return None;
        }

        Self::get_raw_ptr(self.as_mut()).map(|p| unsafe { core::slice::from_raw_parts_mut(p, self.len as usize) })
    }

    pub unsafe fn get_slice(mut self: Pin<&mut Self>) -> Option<&'a [T]> {
        Self::get_raw_ptr(self.as_mut()).map(|p| unsafe { core::slice::from_raw_parts(p, self.len as usize) })
    }

    pub unsafe fn get_mut_slice(mut self: Pin<&mut Self>) -> Option<&'a mut [T]> {
        if unlikely(!self.tag.is_mut()) {
            return None;
        }

        Self::get_raw_ptr(self.as_mut()).map(|p| unsafe { core::slice::from_raw_parts_mut(p, self.len as usize) })
    }

    pub unsafe fn take_vec(mut self: Pin<&mut Self>) -> Option<Vec<T>> {
        if self.tag.borrow() || self.tag.is_static() {
            return None;
        }

        let ptr = Self::get_raw_ptr(self.as_mut())?;
        Some(unsafe { Vec::from_raw_parts(ptr, self.len as usize, self.cap as usize) })
    }
}

impl<'a, T> Drop for Data<'a, T> {
    fn drop(&mut self) {
        if !self.tag.borrow() && !self.ptr.is_null() {
            unsafe {
                let _ = Vec::from_raw_parts(self.ptr, self.len as usize, self.cap as usize);
            }
        }
    }
}
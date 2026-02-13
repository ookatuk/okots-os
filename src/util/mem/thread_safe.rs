use core::arch::asm;
use core::ops::{Deref, DerefMut};

#[repr(Rust)]
#[derive(Debug, Default)]
pub struct GsMainData {

}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Gs {
    pub app_gs: u64,
    pub kernel_gs: u64,
    main_data: GsMainData,  // カプセル化
}

impl Deref for Gs {
    type Target = GsMainData;
    
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.main_data
    }
}

impl DerefMut for Gs {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.main_data
    }
}


#[inline]
pub fn get_mut() -> Option<&'static mut Gs> {
    let value: u64;
    unsafe {
        asm!("mov {}, gs:", out(reg) value, options(nostack, readonly, preserves_flags));
        (value as *mut Gs).as_mut()
    }
}
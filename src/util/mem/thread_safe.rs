use alloc::boxed::Box;
use core::arch::asm;
use core::ops::{Deref, DerefMut};
use crate::cpu;

#[repr(Rust)]
#[derive(Debug, Default)]
pub struct GsMainData {

}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Gs {
    pub self_ptr: u64,
    pub app_stack: u64,
    pub kernel_stack: u64,
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
    let ptr: *mut Gs;
    unsafe {
        asm!(
        "mov {}, gs:[0]",
        out(reg) ptr,
        options(nostack, readonly, preserves_flags)
        );
        ptr.as_mut()
    }
}

pub unsafe fn init_gs(app_stack: *const u8, kernel_stack: *const u8) {
    let mut gs = Box::new(Gs {
        self_ptr: 0, // 後で入れる
        app_stack: app_stack as u64,
        kernel_stack: kernel_stack as u64,
        main_data: Default::default(),
    });

    // Heap上の実体のアドレスを取得
    let ptr = Box::into_raw(gs);
    // 自己参照ポインタを書き込む
    (*ptr).self_ptr = ptr as u64;

    // GS_BASE MSR (0xC0000101) にアドレスを書き込む
    cpu::utils::write_msr(
        cpu::utils::msr::common::GS_BASE,
        ptr as u64
    );
}
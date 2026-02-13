#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

const VERSION: &str = "1.0.0";

/// OSプロトコルバージョン.
const DEBUG_PROTOCOL_VERSION: &str = "1.0";


const LINE_SPACING: f32 = 1.5;
const ENABLE_LIGATURES: bool = true;


const ALLOW_RATIOS: &[(usize, usize)] = &[
    (16, 9),
    (16, 10),
    (4, 3)
];

unsafe extern "C" {
    static __ImageBase: u8;
}

extern crate alloc;

use alloc::boxed::Box;
use core::arch::x86_64;
use core::panic::PanicInfo;
use core::ptr::{null, NonNull};
use uefi::{entry, Identify};
use uefi::boot::SearchType;
use uefi_raw::Status;
use spin::RwLock;
use bitflags::bitflags;
use spin::mutex::Mutex;
use util::result;

mod fonts;
mod cpu;
mod io;
mod rng;
mod fs;
mod util;

#[global_allocator]
/// 物理/仮想アロケーター.
pub static ALLOC: uefi::allocator::Allocator = uefi::allocator::Allocator;

bitflags!{
    pub struct State: u8 {
        const DEST = 1 << 0;
        const RUNNING = 1 << 1;
        const ERR = 1 << 2;
    }
}

struct WatchingItem {
    pub ptr: *const fn(&Main) -> result::Result,
    pub reversed_ptr: u64,
    pub level: u8,
    pub state: State,
}

impl WatchingItem {
    #[inline]
    pub fn new(ptr: *const fn(&Main) -> result::Result, level: u8) -> Self {
        Self { ptr, level, state: State::empty() }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.ptr.is_null()
    }

    #[inline]
    pub fn is_dest(&self) -> bool {
        self.state.contains(State::DEST)
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.state.contains(State::RUNNING)
    }

    #[inline]
    pub fn is_err(&self) -> bool {
        self.state.contains(State::ERR)
    }
}

#[derive(Default, Debug)]
struct Main<'a> {
    gop_data: RwLock<Option<&'a mut io::console::gop::GopData>>,
    watching_list: &'a [Mutex<WatchingItem>],
}


impl Main<'_> {
    fn init_dep(&self) {
        uefi::helpers::init().expect("Failed to init uefi helpers");
    }

    fn init_gop(&self) {
        let handle = uefi::boot::
        locate_handle_buffer(SearchType::ByProtocol(&uefi::proto::console::gop::GraphicsOutput::GUID))
            .expect("Failed to find GOP handles");

        let mut gop =
            uefi::boot::open_protocol_exclusive::<uefi::proto::console::gop::GraphicsOutput>(handle[0])
                .expect("Failed to open GOP");

        // いい感じのを選ぶ
        // w, レベル, index
        let mut target: Option<(usize, usize, uefi::proto::console::gop::Mode)> = None;

        for mode in gop.modes() {
            let info = mode.info();
            let (w, h) = info.resolution();

            if let Some((level, _)) = ALLOW_RATIOS.iter().enumerate().find(|&(_, &(rw, rh))| w * rh == h * rw) {

                let is_better = if let Some((best_w, best_level, _)) = target {
                    // レベルが低い（優先度が高い）か、同じレベルで幅が広い場合
                    level < best_level || (level == best_level && w > best_w)
                } else {
                    true
                };

                if is_better {
                    target = Some((w, level, mode));
                }
            }
        }

        if let Some((_, _, mode)) = target {
            gop.set_mode(&mode).expect("Failed to set video mode");
        }

        let info = gop.current_mode_info();
        let (w, h) = info.resolution();

        let fb_addr = gop.frame_buffer().as_mut_ptr() as *mut u32;

        let gop_data = Box::leak(Box::new(io::console::gop::GopData{
            ptr: NonNull::new(fb_addr).unwrap(),
            w,
            h,
            stride: info.stride(),
        }));

        let mut data = self.gop_data.write();

        if let Some(old_ref) = data.take() {
            unsafe {
                let _ = Box::from_raw(old_ref as *mut _);
            }
        }

        *data = Some(gop_data);
    }

    fn frist_init(&self) {
        self.init_dep();
        let res = cpu::mitigation::ucode::load();
        
        self.init_gop();
    }

    fn a_run_watching(&self, is_bsp: bool) -> u8 { //! TODO (同権限内の)Spectre及びBHIの大部分の系列, Rowhammer脆弱性の踏み台になる可能性の対策
        for (index, i) in self.watching_list.iter().enumerate() {
            let mut data = i.lock();

            x86_64::_mm_lfence();
            if data.is_none() || data.is_running() || (data.is_dest() && !is_bsp) {
                core::hint::spin_loop();
                continue;
            }

            x86_64::_mm_lfence(); // BHIとかSpectreやRowhammer対策
            let de_ptr = data.ptr ^ index ^ 7;

            x86_64::_mm_lfence();
            if de_ptr as u64 != (data.reversed_ptr.reverse_bits()) {
                data.state |= State::ERR;  // ぶっこわれてるのであうと
                return 2;  // 何なら攻撃の可能性もある
            }

            // 軽減策終了

            data.state |= State::RUNNING;

            let mut func = unsafe{*de_ptr};
            let result = func(self);

            data.state &= !State::RUNNING;
            data.ptr = null();

            if result.is_err() {
                data.state |= State::ERR;
                return 1;
            }
            return 0;
        }
        0
    }

    pub fn main(&self) -> ! {
        self.frist_init();

        loop {}
    }
}

#[entry]
fn main() -> Status {
    Status::ABORTED
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop()
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {  // アロケーターエラー関係
    panic!("alloc failed: {:?}", layout);
}

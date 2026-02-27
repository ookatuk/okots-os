#![no_std]
#![no_main]

const VERSION: &str = "1.0.0";

/// OSプロトコルバージョン.
const DEBUG_PROTOCOL_VERSION: &str = "1.0";

const ENABLE_DEBUG: bool = true;

const LINE_SPACING: f32 = 1.5;
const ENABLE_LIGATURES: bool = true;

const MAX_DO_ITEM: usize = 1000;

const BAR_HEIGHT: usize = 20;
const BAR_MARGIN: usize = 50;

const GUI_WAIT: usize = 2_000_000;

const PANICED_TO_RESTART_TIME: usize = 20;

const ALLOW_RATIOS: &[(usize, usize)] = &[
    (21, 9),
    (32, 9),
    (16, 9),
    (16, 10),
    (4, 3),
    (3, 2),
    (5, 4),
];

const MAIN_FONT: &'static[u8]  = include_bytes!("../assets/ZeroveItalic.ttf");

unsafe extern "C" {
    static __ImageBase: u8;
}

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::arch::{naked_asm};
use core::cmp::min;
use core::ffi::c_void;
use core::hint::spin_loop;
use core::panic::PanicInfo;
use core::ptr;
use core::ptr::{addr_of, NonNull};
use core::sync::atomic::{AtomicU8, AtomicUsize};
use core::sync::atomic::Ordering::SeqCst;
use spin::{Once, RwLock};
use bitflags::bitflags;
use fontdue::Font;
use num_traits::Zero;
use spin::mutex::Mutex;
use uefi::boot::{TimerTrigger};
use uefi::{entry, runtime};
use uefi::mem::memory_map::{MemoryMap, MemoryMapOwned};
use uefi::proto::console::gop;
use uefi_raw::Status;
use uefi_raw::table::boot::{EventType, MemoryDescriptor, MemoryType, Tpl};
use uefi_raw::table::runtime::ResetType;
use util::result;
use crate::util::result::{Error, ErrorType};
use x86_64::instructions::interrupts;
use util::mem::allocator;
use crate::cpu::utils;
use crate::fonts::Text;
use crate::io::console::gop::Color;
use crate::manager::display_manager::DisplayManager;
use crate::manager::load_task_manager::LoadTaskManager;
use crate::manager::memory_manager::MemoryManager;
use crate::util::mem::types::MemMap;
use crate::util::timer::TSC;

extern crate alloc;

mod fonts;
mod cpu;
mod io;
mod rng;
mod fs;
mod util;
mod manager;

#[global_allocator]
/// 物理/仮想アロケーター.
pub static ALLOC: allocator::uefi_allocator::LockedAllocator = allocator::uefi_allocator::LockedAllocator::new();

bitflags!{
    #[derive(Debug)]
    pub struct State: u8 {
        const DEST = 1 << 0;
        const RUNNING = 1 << 1;
        const ERR = 1 << 2;
    }
}

#[derive(Debug)]
struct WatchingItem {
    pub ptr: u64,
    pub reversed_ptr: u64,
    pub level: u8,
    pub state: State,
}

impl WatchingItem {
    #[inline]
    pub fn new<T>(ptr: fn(&T) -> result::Result, level: u8) -> Self {
        let addr = ptr as usize; // ここで数値化
        Self {
            ptr: (addr ^ level as usize ^ 7) as u64,
            reversed_ptr: (addr.reverse_bits()) as u64,
            level,
            state: State::empty(),
        }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.ptr.is_zero()
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

#[derive(Default)]
#[repr(align(16))]
struct Main {
    did_it_item: AtomicUsize,
    global_font: Arc<RwLock<Option<Font>>>,
    tsc_timer: Arc<RwLock<TSC>>,

    do_fn: Once<Arc<dyn Fn()>>,

    load_task_manager: Arc<LoadTaskManager>,

    display_manager: DisplayManager,
    memory_manager: MemoryManager,
}


impl Main {
    fn init_font(&self) -> result::Result<()> {
        let new_font = Box::new(fonts::load_font(MAIN_FONT));
        interrupts::without_interrupts(|| {
            let mut a = self.global_font.write();
            *a = Some(*new_font);
        });

        Ok(())
    }

    fn frist_init(&self) -> Vec<result::Result> {
        let mut ret = vec![];

        self.display_manager.global_font.call_once(||{self.global_font.clone()});
        self.load_task_manager.do_parent.call_once(|| {self.display_manager.do_parent.clone()});

        let ltm = Arc::clone(&self.load_task_manager);

        self.do_fn.call_once(||{ltm.get_add_func()});

        self.memory_manager.do_fn.call_once(||{self.do_fn.get().unwrap().clone()});

        ret.push(|| -> result::Result {
            interrupts::without_interrupts(|| {
                self.tsc_timer.write().init(None)?;
                util::logger::init_timer(self.tsc_timer.clone());
                Ok(())
            })
        }());

        ret.push(cpu::mitigation::ucode::load());

        ret.push(self.init_font());
        
        ret.push(self.display_manager.init_gop());

        ret.push(self.display_manager.start_load_grap());

        ret
    }

    pub unsafe extern "C" fn main(&self) -> ! {
        let res = self.frist_init();

        log_info!("kernel", "main", "checking results");

        for (i, ret) in res.iter().enumerate() {
            if !ret.is_err() {
                continue
            }

            if i == 3{
                ret.clone().expect("Failed to get GOP data");
            } else if i == 1 {
                log_warn!("kernel", "security", "failed to attach micro code: {}", ret.clone().unwrap_err().to_string());
            } else {
                log_warn!("kernel", "kernel", "any failed(number: {}): {}", i, ret.clone().unwrap_err().to_string());
            }
        }

        self.do_fn.get().unwrap()();

        self.memory_manager.init_memory().expect("failed to init memory system.");

        loop {
            spin_loop();
        }
    }

    pub fn test(&self) -> result::Result {
        log_info!("kernel", "main", "testaaa");
        Ok(())
    }
}

mod _internal_init {
    use core::alloc::Layout;
    use core::ptr;
    use uefi::runtime;
    use crate::{cpu, io, log_custom, log_debug, util, Main, DEBUG_PROTOCOL_VERSION, ENABLE_DEBUG, VERSION};
    use crate::cpu::utils;

    pub unsafe extern "C" fn init_dep() {
        io::console::serial::init_serial();
        uefi::helpers::init().expect("Failed to init uefi helpers");
    }

    pub fn get_boot_entropy() -> usize {
        let mut entropy: usize = 0;

        if let Ok(mut rng_proto) = util::proto::open::<uefi::proto::rng::Rng>(None) {
            let mut buf = [0u8; size_of::<usize>()];
            if rng_proto.get_rng(None, &mut buf).is_ok() {
                entropy = usize::from_le_bytes(buf);
            }
        }

        let tsc = unsafe { core::arch::x86_64::_rdtsc() as usize };

        let time_val = runtime::get_time().map(|t| t.nanosecond() as usize).unwrap_or(0);

        entropy ^ tsc ^ time_val
    }

    pub unsafe extern "C" fn debug_hand() {
        log_custom!("s", "ds", "a", "");
        log_custom!("s", "ds", "d", "{}", if ENABLE_DEBUG {1} else {0});
        log_custom!("s", "ds", "v", "{}", VERSION);
        log_custom!("s", "ds", "pv", "{}", DEBUG_PROTOCOL_VERSION);

        if ENABLE_DEBUG {
            log_debug!("debug", "cpu vendor", "{}, 0x{:x}", unsafe{cpu::utils::get_vendor_name()}, unsafe { utils::cpuid(cpu::utils::cpuid::common::PIAFB, None) }.eax);
        }
    }

    pub unsafe extern "C" fn allocate() -> *mut u64 {
        let entropy = (get_boot_entropy() % 65536) & !0xf;
        let stack_size = 1024;
        let main_size = size_of::<Main>();
        let main_align = align_of::<Main>();

        let total_size = stack_size + entropy + main_size + main_align;
        let layout = Layout::from_size_align(total_size, 4096).unwrap();
        let allocated = unsafe { alloc::alloc::alloc_zeroed(layout) };

        if allocated.is_null() {
            panic!("Allocation failed");
        }

        let stack_top = unsafe { allocated.add(stack_size) as usize } & !0xf;

        let struct_addr = (stack_top + entropy + main_align) & !(main_align - 1);
        let struct_ptr = struct_addr as *mut Main;

        unsafe {
            ptr::write(struct_ptr, Main::default());
        }

        let ctx_ptr = unsafe { alloc::alloc::alloc(Layout::new::<[u64; 2]>()) as *mut u64 };
        unsafe {
            *ctx_ptr.offset(0) = stack_top as u64;
            *ctx_ptr.offset(1) = struct_ptr as u64;
        }

        ctx_ptr
    }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn real_main() -> ! {
    naked_asm!(
        "endbr64",
        "sub rsp, 32",
        "call {init_dep}",
        "add rsp, 32",

        "sub rsp, 32",
        "call {debug_hand_shake}",
        "add rsp, 32",

        "sub rsp, 32",
        "call {allocate}",
        "add rsp, 32",

        "mov rcx, [rax + 8]",
        "mov rsp, [rax]",

        "sub rsp, 32",
        "jmp {main}",
        init_dep = sym _internal_init::init_dep,
        debug_hand_shake = sym _internal_init::debug_hand,
        allocate = sym _internal_init::allocate,
        main = sym Main::main,
    )
}

#[entry]
fn main() -> Status {
    unsafe {
        core::arch::asm!(
            "jmp {target}",
            target = sym real_main,
            options(noreturn)
        )
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let message = info.to_string();
    let loc = info.location().unwrap().to_string();

    log_last!("kernel", "panic", "{}\n{}", loc, message);
    log_last!("kernel", "panic", "A critical system error has occurred. System will restart in {} seconds. for system admin: (info: {}, by: {})", PANICED_TO_RESTART_TIME, info.message(), info.location().unwrap());

    loop {
        spin_loop()
    }
}
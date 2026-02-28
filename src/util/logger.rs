use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Display};
use core::panic::Location;
use core::sync::atomic::{AtomicUsize, Ordering};
use serde::Serialize;
use spin::{Lazy, Once, RwLock};
use x86_64::instructions::interrupts;
use crate::io::console::serial::SERIAL1;
use crate::util::timer::TSC;

const LOG_CAPACITY: usize = 5000;

pub(crate) static LOG_BUF: Lazy<RwLock<Vec<Arc<OsLog>>>> = Lazy::new(|| {
    RwLock::new(Vec::with_capacity(LOG_CAPACITY))
});

static LOG_TIMER: Once<Arc<RwLock<TSC>>> = Once::new();

pub fn init_timer(timer: Arc<RwLock<TSC>>) {
    LOG_TIMER.call_once(|| {timer});
}

static LOG_HEAD_ID: AtomicUsize = AtomicUsize::new(0); // 0番目の要素の通算ID

pub fn add_log(data: OsLog) {
    interrupts::without_interrupts(|| {
        let shared = Arc::new(data);

        let mut lock = LOG_BUF.write();

        if lock.len() >= LOG_CAPACITY {
            lock.remove(0);
            LOG_HEAD_ID.fetch_add(1, Ordering::SeqCst);
        }

        lock.push(shared);

    })
}

pub fn read_log(target_id: usize) -> Option<Arc<OsLog>> {
    interrupts::without_interrupts(|| {
        let head = LOG_HEAD_ID.load(Ordering::SeqCst);
        let lock = LOG_BUF.read();
        let current_len = lock.len();

        if target_id < head {
            return None;
        }

        if target_id >= head + current_len {
            return None;
        }

        let index = target_id - head;
        lock.get(index).cloned()
    })
}

pub struct LogIterator {
    next_id: usize,
    include_system: bool, // "s" を含むかどうか
}

impl LogIterator {
    pub fn new(start_id: usize, include_system: bool) -> Self {
        Self {
            next_id: start_id,
            include_system,
        }
    }
}

impl Iterator for LogIterator {
    type Item = Arc<OsLog>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let log = read_log(self.next_id)?;
            self.next_id += 1;

            let is_system = log.level == "s";

            if self.include_system == is_system {
                return Some(log);
            }
        }
    }
}

#[inline]
pub fn get_log_min_id() -> usize {
    LOG_HEAD_ID.load(Ordering::SeqCst)
}

#[macro_export]
macro_rules! log_custom {
    ($level:expr,$by:expr,$tag:expr,$($text:tt)*) => { $crate::util::logger::_custom($level, $by, $tag, format_args!($($text)*)) };
}

#[macro_export]
macro_rules! log_trace {
    ($by:expr,$tag:expr,$($text:tt)*) => { if cfg!(feature = "debug-mode") {$crate::log_custom!("trace", $by, $tag, $($text)*)} };
}

#[macro_export]
macro_rules! log_debug {
    ($by:expr,$tag:expr,$($text:tt)*) => { if cfg!(feature = "debug-mode") {$crate::log_custom!("debug", $by, $tag, $($text)*)} };
}

#[macro_export]
macro_rules! log_info {
    ($by:expr,$tag:expr,$($text:tt)*) => { $crate::log_custom!("info", $by, $tag, $($text)*) };
}

#[macro_export]
macro_rules! log_warn {
    ($by:expr,$tag:expr,$($text:tt)*) => { $crate::log_custom!("warn", $by, $tag, $($text)*) };
}

#[macro_export]
macro_rules! log_error {
    ($by:expr,$tag:expr,$($text:tt)*) => { $crate::log_custom!("error", $by, $tag, $($text)*) };
}

#[macro_export]
macro_rules! log_last {
    ($by:expr,$tag:expr,$($text:tt)*) => { $crate::log_custom!("last", $by, $tag, $($text)*) };
}

#[track_caller]
pub fn _custom(level: &'static str, by: &'static str, tag: &'static str, text: core::fmt::Arguments) {
    let location = Location::caller();
    custom_internal(level, by, tag, text, location);
}

#[derive(Serialize, Clone)]
#[repr(C)]
pub struct OsLog {
    pub level: Cow<'static, str>,
    pub by: Cow<'static, str>,
    pub tag: Cow<'static, str>,
    pub data: String,
    pub file: &'static str,
    pub time: u64,
    pub line: u32,
    pub column: u32,
    pub cpu_acpi_id: u32,
}

impl Display for OsLog {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "({}), [{:<5}] [{:<5}] {} (at {}:{}:{})",
            self.cpu_acpi_id,
            self.level,
            self.tag,
            self.data,
            self.file,
            self.line,
            self.column,
        )
    }
}

impl OsLog {
    pub fn to_short_string(&self) -> String {
        alloc::format!("({}) [{}] {}: {}", self.cpu_acpi_id, self.level, self.tag, self.data)
    }
}

pub fn custom_internal(level: &'static str, by: &'static str, tag: &'static str, text: core::fmt::Arguments, loc: &'static Location) {
    _real_custom_internal(level, by, tag, text, loc);
}

pub fn _real_custom_internal(level: &'static str, by: &'static str, tag: &'static str, text: core::fmt::Arguments, loc: &'static Location) {
    let mut time = 0;

    if let Some(a) = LOG_TIMER.get() {
        if let Some(tsc) = a.try_read() {
            if let Some(clock) = tsc.clock_in_100ms.as_ref() {
                let tsc_per_ms = clock.get() / 100;

                time = tsc.now_clock() / tsc_per_ms;
            }
        }
    }

    let data = OsLog {
        time,
        level: Cow::Borrowed(level),
        by: Cow::Borrowed(by),
        tag: Cow::Borrowed(tag),
        data: text.to_string(),

        file: loc.file(),
        line: loc.line(),
        column: loc.column(),

        cpu_acpi_id: crate::cpu::utils::who_am_i(),
    };
    let a = bincode::serde::encode_to_vec(
        data.clone(),
        bincode::config::standard()
    );

    if let Ok(data) = a {
        interrupts::without_interrupts(|| {
            let mut lk = SERIAL1.lock();
            lk.send_raw(0xAA);
            lk.send_raw(0xBB);
            lk.send_raw(0xCC);
            lk.send_raw(0xEE);
            for i in ((data.len()+0) as u32).to_le_bytes() {
                lk.send_raw(i);
            }
            for i in data {
                lk.send_raw(i);
            }
        })
    }

    add_log(data);
}

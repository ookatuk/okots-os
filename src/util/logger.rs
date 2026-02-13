use alloc::string::ToString;
use core::panic::Location;
use core::sync::atomic::Ordering;
use bincode::config;

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
    ($by:expr,$tag:expr,$($text:tt)*) => { $crate::log_custom!("debug", $by, $tag, $($text)*) };
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
pub fn _custom(level: &str, by: &str, tag: &str, text: core::fmt::Arguments) {
    let location = core::panic::Location::caller();
    text.to_string().split('\n').for_each(|line| _custom_internal(level, by, tag, line, location));
}

#[derive(Serialize)]
pub struct OsLog<'a> {
    pub time: f64,      // タイムスタンプ（数値のまま！）
    pub file: &'a str,
    pub line: u32,
    pub column: u32,
    pub level: &'a str,  // LogLevel (カスタム文字列)
    pub by: &'a str,  // by (コンポーネント名)
    pub tag: &'a str,  // tag (サブカテゴリ)
    pub data: &'a str,  // data/text (メッセージ本体)
}

#[track_caller]
pub fn _custom_internal(level: &str, by: &str, tag: &str, text: &str, loc: &Location) {
    let loop_d = match level {
        "s" => SEND_MAX_LOOP_FOR_CRITICAL.load(Ordering::Relaxed),
        "error" => SEND_MAX_LOOP_FOR_CRITICAL.load(Ordering::Relaxed),
        "last" => SEND_MAX_LOOP_FOR_CRITICAL.load(Ordering::Relaxed),
        "warn" => SEND_MAX_LOOP_FOR_CRITICAL.load(Ordering::Relaxed),
        _ => crate::SEND_MAX_LOOP.load(Ordering::Relaxed),
    };

    if loop_d == 0 {
        return;
    }

    if !crate::time::get_time::TSC.loaded() {
        return;
    }

    let tsc = &crate::time::get_time::TSC;
    let freq = tsc.clock.load(Ordering::Relaxed);
    let now_tsc = tsc.get_tsc_fenced();

    let total_secs = (now_tsc as f64) / (freq as f64);

    let data = OsLog {
        time: total_secs,
        level,
        by,
        tag,
        data: text,

        file: loc.file(),
        line: loc.line(),
        column: loc.column(),
    };

    x86_64::instructions::interrupts::without_interrupts(|| {
        if loop_d == SEND_MAX_LOOP_FOR_CRITICAL.load(Ordering::Relaxed) {
            // unsafe { crate::io::uart::UART_WRITER.force_unlock() };
        }

        let config = config::standard();
        let mut buffer = [0u8; 1024];
        let encoded_len = bincode::serde::encode_into_slice(&data, &mut buffer, config).unwrap();

        let len = encoded_len as u32;

        let mut writer = crate::io::uart::UART_WRITER.lock();

        for _ in 0..loop_d {
            writer.write_bytes(&[0xAA, 0xBB, 0xCC, 0xEE]);
            writer.write_bytes(&len.to_le_bytes());
            writer.write_bytes(&buffer[..encoded_len]);
        }
    });
}

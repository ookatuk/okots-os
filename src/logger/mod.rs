pub mod types;
pub mod core;
pub mod macros;
pub mod utils;

pub mod items {
    #![allow(unused_imports)]

    pub use super::core::{read_log, get_log_min_id};
    pub use super::types::{OsLog, LogIterator};
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use core::sync::atomic::Ordering;
    use crate::logger::core::{add_log, get_log_min_id, LOG_BUF, LOG_CAPACITY, LOG_HEAD_ID};
    use crate::logger::items::OsLog;

    fn reset_logger(cap: usize) {
        let mut lock = LOG_BUF.write();
        lock.clear();
        lock.reserve(cap);
        LOG_CAPACITY.store(cap, Ordering::SeqCst);
        LOG_HEAD_ID.store(0, Ordering::SeqCst);
    }

    #[test]
    fn test_log_rotation() {
        reset_logger(3);

        for i in 0..4 {
            add_log(&OsLog {
                level: "info",
                by: "test",
                tag: "core",
                data: i.to_string(),
                file: "test.rs",
                time: 0,
                line: 0,
                column: 0,
                cpu_acpi_id: 0,
            });
        }

        // 今度は 1 になっているはず
        assert_eq!(get_log_min_id(), 1);
        assert_eq!(LOG_BUF.read().len(), 3);
    }
}
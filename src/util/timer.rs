use core::num::{NonZeroU64};
use core::time::Duration;
use x86_64::instructions::interrupts::without_interrupts;
use crate::util::result;
use crate::util::result::{Error, ErrorType};

#[derive(Debug, Default)]
pub struct TSC {
    pub clock_in_100ms: Option<NonZeroU64>,
}

impl TSC {
    pub fn new() -> Self {
        TSC { clock_in_100ms: None }
    }

    #[inline]
    pub fn now_clock(&self) -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    pub fn init(&mut self, timer_100ms: Option<fn()>) -> result::Result {
        let (start, end) = without_interrupts(|| {
            let start = unsafe { core::arch::x86_64::_rdtsc() };

            match timer_100ms {
                Some(timer) => timer(),
                None => uefi::boot::stall(Duration::from_millis(100)),
            }

            let end = unsafe { core::arch::x86_64::_rdtsc() };
            (start, end)
        });

        if let Some(clock) = NonZeroU64::new(end - start) {
            self.clock_in_100ms = Some(clock);
            Ok(())
        } else {
            Error::new(
                ErrorType::DeviceError,
                Some("100ms clock is zero")
            ).raise()
        }
    }
}
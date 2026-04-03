use core::hint::spin_loop;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use spin::{Lazy, RwLock};
use x86::time::rdtsc;
use x86_64::instructions::interrupts::without_interrupts;
use crate::{cpu_info};
use crate::thread_local::read_gs;
use crate::timer::{Timer, TimerConst, TimerConstTimeStampInfo};

pub static TSC: Lazy<Tsc> = Lazy::new(|| {
    Tsc::new()
});

#[derive(Debug, Default)]
pub struct TscGsData {
    pub par_100ns: u64,
    pub adjust: u64,
}

#[derive(Debug)]
pub struct Tsc {
    pub is_invariant: bool,
    pub par_100ns: AtomicU64,
    pub utc_offset: RwLock<Duration>,
}

impl Tsc {
    pub fn new() -> Self {
        let is_invariant = cpu_info!(environment::tsc::InvariantTsc);
        Self {
            is_invariant,
            par_100ns: AtomicU64::new(0),
            utc_offset: RwLock::new(Duration::default()),
        }
    }

    pub fn init_for_ap(&self, timer: fn(Duration) -> (), wait: Duration) {
        if without_interrupts(|| -> bool {
            let gs = read_gs().unwrap();
            if gs.tsc_init {
                return true;
            }
            false
        }) {
            return;
        }

        if self.is_invariant && self.par_100ns.load(Ordering::SeqCst) > 0 {
            return;
        }

        let (start, end) = without_interrupts(|| {
            let start = Self::get();
            timer(wait);
            let end = Self::get();
            (start, end)
        });

        let count = end.wrapping_sub(start);
        let units = (wait.as_nanos() / 100) as u64;

        if units == 0 { return; }
        let par_100ns_value = count / units;

        without_interrupts(|| {
            let gs = read_gs().unwrap();
            gs.tsc_init = true;
        });

        if self.is_invariant {
            self.par_100ns.store(par_100ns_value, Ordering::SeqCst);
        } else {
            without_interrupts(|| {
                let gs = read_gs().unwrap();
                gs.tsc_data.par_100ns = par_100ns_value;
            });
        }
    }

    pub fn get_val(&self) -> u64 {
        if self.is_invariant {
            self.par_100ns.load(Ordering::SeqCst)
        } else {
            without_interrupts(|| {
                let gs = read_gs().unwrap();
                gs.tsc_data.par_100ns + gs.tsc_data.adjust
            })
        }
    }

    #[inline]
    pub fn get() -> u64 {
        unsafe{rdtsc()}
    }
}

impl const TimerConst for Tsc {
    fn accuracy(&self) -> Duration {
        Duration::from_nanos(100)
    }

    fn utc_supported(&self) -> TimerConstTimeStampInfo {
        TimerConstTimeStampInfo::NeedInit
    }

    fn lts_supported(&self) -> TimerConstTimeStampInfo {
        TimerConstTimeStampInfo::NeedInit
    }
}

impl Timer for Tsc {
    fn get_time(&self) -> Duration {
        let current_tsc = Self::get() as u128;
        let counts_per_100ns = self.get_val() as u128;

        if counts_per_100ns == 0 { return Duration::ZERO; }

        let nanos = (current_tsc * 100) / counts_per_100ns;
        Duration::from_nanos(nanos as u64)
    }


    fn spin(&self, wait: Duration) {
        let start_tsc = Self::get();
        let counts_per_100ns = self.get_val();

        let wait_counts = (wait.as_nanos() * counts_per_100ns as u128) / 100;
        let target_tsc = start_tsc as u128 + wait_counts;

        while (Self::get() as u128) < target_tsc {
            spin_loop();
        }
    }

    fn option_init_time_stamp(&self, utc: Duration) {
        without_interrupts(|| {
            let current_time = self.get_time();
            let target = utc - current_time;

            let mut lock = self.utc_offset.write();
            *lock = target;
        })
    }

    fn get_world_time_utc(&self) -> Option<Duration> {
        let offset = without_interrupts(|| *self.utc_offset.read());

        if offset.is_zero() {
            return None;
        }

        Some(offset + self.get_time())
    }
}
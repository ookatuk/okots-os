use core::time::Duration;

pub mod tsc;
pub mod rtc;

pub enum TimerConstTimeStampInfo {
    Supported,
    NeedInit,
    NotSupported,
}

pub const trait TimerConst {
    fn accuracy(&self) -> Duration;
    fn utc_supported(&self) -> TimerConstTimeStampInfo;
    fn lts_supported(&self) -> TimerConstTimeStampInfo;
}

pub trait Timer: TimerConst {
    fn get_time(&self) -> Duration;
    fn spin(&self, wait: Duration);
    fn option_init_time_stamp(&self, utc: Duration);
    fn get_world_time_utc(&self) -> Option<Duration>;
}
use core::hint::spin_loop;
use core::time::Duration;
use spin::Lazy;
use x86_64::instructions::port::Port;
use crate::timer::{Timer, TimerConst, TimerConstTimeStampInfo};

pub static RTC: Lazy<Rtc> = Lazy::new(|| {
    Rtc::new()
});

const RTC_ADDR: u16 = 0x70;
const RTC_DATA: u16 = 0x71;
const REG_SEC: u8 = 0x00;
const REG_MIN: u8 = 0x02;
const REG_HOUR: u8 = 0x04;
const REG_DAY: u8 = 0x07;
const REG_MONTH: u8 = 0x08;
const REG_YEAR: u8 = 0x09;
const REG_STAT_A: u8 = 0x0A;
const REG_STAT_B: u8 = 0x0B;
const REG_CENTURY: u8 = 0x32;

fn read_rtc_reg(reg: u8) -> u8 {
    let mut addr_port = Port::new(RTC_ADDR);
    let mut data_port = Port::new(RTC_DATA);
    unsafe {
        addr_port.write(reg | 0x80);
        data_port.read()
    }
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}


fn bin_to_bcd(bin: u8) -> u8 {
    ((bin / 10) << 4) | (bin % 10)
}

fn write_rtc_reg(reg: u8, value: u8) {
    let mut addr_port = Port::new(RTC_ADDR);
    let mut data_port = Port::new(RTC_DATA);

    unsafe {
        addr_port.write(reg | 0x80);
        data_port.write(value);
    }
}

fn is_updating() -> bool {
    (read_rtc_reg(REG_STAT_A) & 0x80) != 0
}

#[derive(Default)]
pub struct Rtc {

}

impl Rtc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_time(&self, year: u64, month: u8, day: u8, hour: u8, min: u8, sec: u8) {
        let status_b = read_rtc_reg(REG_STAT_B);
        write_rtc_reg(REG_STAT_B, status_b | 0x80);

        write_rtc_reg(REG_SEC, bin_to_bcd(sec));
        write_rtc_reg(REG_MIN, bin_to_bcd(min));
        write_rtc_reg(REG_HOUR, bin_to_bcd(hour));
        write_rtc_reg(REG_DAY, bin_to_bcd(day));
        write_rtc_reg(REG_MONTH, bin_to_bcd(month));

        write_rtc_reg(REG_YEAR, bin_to_bcd((year % 100) as u8));
        write_rtc_reg(REG_CENTURY, bin_to_bcd((year / 100) as u8));

        write_rtc_reg(REG_STAT_B, status_b & 0x7F);

        let mut addr_port = Port::new(RTC_ADDR);
        unsafe { addr_port.write(REG_SEC & 0x7F); }
    }

    pub fn sync_and_get_time(&self) -> Duration {
        while is_updating() { spin_loop(); }
        let initial_sec = read_rtc_reg(REG_SEC);

        loop {
            while is_updating() { spin_loop(); }
            let current_sec = read_rtc_reg(REG_SEC);

            if current_sec != initial_sec {
                break;
            }
            spin_loop();
        }

        self.get_time()
    }
}

impl TimerConst for Rtc {
    fn accuracy(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn utc_supported(&self) -> TimerConstTimeStampInfo {
        TimerConstTimeStampInfo::NotSupported
    }

    fn lts_supported(&self) -> TimerConstTimeStampInfo {
        TimerConstTimeStampInfo::NotSupported
    }
}

impl Timer for Rtc {
    fn get_time(&self) -> Duration {
        while is_updating() {spin_loop()}

        let sec = bcd_to_bin(read_rtc_reg(REG_SEC));
        let min = bcd_to_bin(read_rtc_reg(REG_MIN));
        let day = bcd_to_bin(read_rtc_reg(REG_DAY));
        let month = bcd_to_bin(read_rtc_reg(REG_MONTH));

        let mut hour = read_rtc_reg(REG_HOUR);

        let is_pm = hour & 0x80 != 0;
        hour = bcd_to_bin(hour & 0x7F);
        if is_pm { hour += 12; }

        let century_bcd = read_rtc_reg(REG_CENTURY);
        let century = bcd_to_bin(century_bcd) as u64;

        let year_short = bcd_to_bin(read_rtc_reg(REG_YEAR)) as u64;

        let mut addr_port = Port::new(RTC_ADDR);
        unsafe { addr_port.write(REG_SEC & 0x7F); }

        let year = (century * 100) + year_short;

        let mut total_days = (year - 1970) * 365
            + (year - 1969) / 4
            - (year - 1901) / 100
            + (year - 1601) / 400;

        let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for i in 1..month as usize {
            total_days += month_days[i];
        }

        if month > 2 && is_leap_year(year) {
            total_days += 1;
        }

        total_days += (day - 1) as u64;

        let total_secs = total_days * 86400
            + (hour as u64) * 3600
            + (min as u64) * 60
            + (sec as u64);

        Duration::from_secs(total_secs)
    }

    fn spin(&self, wait: Duration) {
        let target = self.get_time().as_secs() + wait.as_secs();
        while self.get_time().as_secs() < target {
            spin_loop();
        }
    }

    fn option_init_time_stamp(&self, _: Duration) {}

    fn get_world_time_utc(&self) -> Option<Duration> {
        None
    }
}
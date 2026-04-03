use x86::msr::{rdmsr, wrmsr};

pub mod msr_address {
    pub use x86::msr::*;
}

#[inline]
pub unsafe fn write(addr: u32, value: u64) {
    unsafe{wrmsr(
        addr,
        value
    )}
}

#[inline]
pub unsafe fn read(addr: u32) -> u64 {
    unsafe{rdmsr(addr)}
}
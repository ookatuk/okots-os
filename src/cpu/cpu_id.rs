use x86::cpuid::CpuIdResult;
use x86::cpuid::native_cpuid::cpuid_count;

#[inline]
pub unsafe fn read(a: u32, b: Option<u32>) -> CpuIdResult {
    cpuid_count(
        a,
        b.unwrap_or(0),
    )
}
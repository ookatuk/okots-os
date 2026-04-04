#[cfg(not(test))]
use x86_64::instructions::interrupts::without_interrupts;

#[inline(always)]
pub fn with_interr<F, R>(f: F) -> R where
    F: FnOnce() -> R
{
    #[cfg(test)]
    return f();
    #[cfg(not(test))]
    return without_interrupts(f);
}

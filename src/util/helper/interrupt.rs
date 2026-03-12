use crate::util::result;
use alloc::boxed::Box;
use spin::Once;
use x86_64::{
    VirtAddr,
    instructions::interrupts::without_interrupts,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame},
};

static IDT: Once<&'static InterruptDescriptorTable> = Once::new();
static HELPER_CREATED: Once = Once::new();

extern "x86-interrupt" fn timer_interrupt(_stack: InterruptStackFrame) {
    unsafe {
        crate::util::lapic::LapicOffset::Eoi.write(0).unwrap();
    }
}

pub struct InterruptHelper;

impl InterruptHelper {
    pub fn init() -> result::Result<Self> {
        if HELPER_CREATED.is_completed() {
            return result::Error::new(
                result::ErrorType::AlreadyUsed,
                Some("interrupt helper already created."),
            )
            .raise();
        }
        HELPER_CREATED.call_once(|| unsafe {
            without_interrupts(|| {
                let idt = Box::leak(Box::new(InterruptDescriptorTable::new()));
                idt[32].set_handler_fn(timer_interrupt);

                idt.load();

                IDT.call_once(|| idt);
            });
        });

        Ok(Self)
    }
}

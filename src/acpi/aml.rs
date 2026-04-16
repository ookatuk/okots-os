use alloc::sync::Arc;
use core::hint::{cold_path, likely, unlikely};
use core::ops::Deref;
use acpi::{Handler};
use acpi::aml::Interpreter;
use spin::{Mutex, Once};
use crate::acpi::handler::TmpHandler;
use crate::memory::paging::PHY_OFFSET;
use crate::{deb, log_error, result};
use crate::result::{ErrorType, Wirt};

static CACHE: Once<Mutex<Interpreter<TmpHandler>>> = Once::new();

pub fn get_dsdt() -> Wirt<&'static Mutex<Interpreter<TmpHandler>>> {
    if likely(CACHE.is_completed()) {
        return Wirt::Ok(unsafe{CACHE.get().unwrap_unchecked()});
    }

    let table = Wirt::from_option(
        super::core::ACPI_TABLE_TMP_HANDLER.get(),
        ErrorType::NotInitialized,
        Some("acpi table is not initialized.")
    )?;

    for i in table.find_tables::<acpi::sdt::fadt::Fadt>() {
        let dsdt_addr = Wirt::try_raise_acpi(
            i.get().dsdt_address(),
            Some("dsdt address is invalid")
        )?;

        let dsdt_header = unsafe { *((dsdt_addr + PHY_OFFSET) as *const acpi::sdt::SdtHeader) };
        let dsdt_revision = dsdt_header.revision;

        let reg = acpi::registers::FixedRegisters::new(
            i.deref(),
            i.handler.clone()
        );

        let reg = Wirt::try_raise_acpi(
            reg,
            Some("failed to create fixed registers")
        )?;

        let addr = Wirt::try_raise_acpi(
            i.facs_address(),
            Some("facs_address is invalid")
        )?;

        let ptr = unsafe{i.handler.map_physical_region(
            addr,
            size_of::<acpi::sdt::facs::Facs>()
        )};

        let interr = Interpreter::new(
            i.handler.clone(),
            dsdt_revision,
            Arc::new(reg),
            Some(ptr)
        );

        let data = unsafe {
            core::slice::from_raw_parts(
                (dsdt_addr + PHY_OFFSET) as *const u8,
                dsdt_header.length as usize,
            )
        };

        let aml_data = &data[36..];

        Wirt::try_raise_aml(
            interr.load_table(aml_data),
            Some("failed to load dsdt")
        )?;

        for i in table.ssdts() {
            let data = unsafe {
                core::slice::from_raw_parts(
                    (i.phys_address + PHY_OFFSET) as *const u8,
                    i.length as usize,
                )
            };

            let data = &data[36..];

            if interr.load_table(data).is_err() {
                log_error!("kernel", "aml", "failed to load ssdt. continue.");
            }
        }

        interr.initialize_namespace();

        let val = CACHE.call_once(|| Mutex::new(interr));

        return Wirt::Ok(val);
    }

    Wirt::Err(
        ErrorType::NotFound,
        Some("fadt not found"),
    )
}
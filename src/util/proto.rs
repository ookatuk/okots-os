use uefi::boot;
use uefi::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, SearchType};
use uefi::proto::ProtocolPointer;
use crate::deb;
use crate::result::{ErrorType, Wirt};

pub fn open<P: ProtocolPointer + ?Sized>(index: Option<usize>) -> Wirt<ScopedProtocol<P>> {

    let handles = Wirt::try_raise_uefi(
        boot::locate_handle_buffer(SearchType::ByProtocol(&P::GUID)),
        Some("Failed to get handle buffer")
    )?;


    let target_index = index.unwrap_or(0);
    let target_handle = *handles.get(target_index).ok_or_else(|| Wirt::<ScopedProtocol<P>>::Err(
        ErrorType::NotFound,
        Some("The requested protocol handle index is out of bounds"),
    ))?;

    unsafe {
        Wirt::try_raise_uefi(
            boot::open_protocol::<P>(
                OpenProtocolParams {
                    handle: target_handle,
                    agent: boot::image_handle(),
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            ),
            Some("Failed to open protocol")
        )
    }
}
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use core::any::Any;
use core::fmt::{Debug, Display, Formatter};
use core::panic::Location;

pub type Result<Output = ()> = core::result::Result<Output, Error>;

#[derive(Debug, Clone)]
pub enum ErrorType {
    InternalError,
    DeviceError,
    ReadError,

    NotSupported,

    AllocationFailed,

    FileNotFound,

    InvalidData,
    InvalidArgument,

    IndexMax,

    NotAFile,

    InvalidFileType,

    AlreadyUsed,
    AlreadyInitialized,

    NotFound,
    OverFlow,

    NotInitialized,

    UefiBroken,

    ReturnIsNone,
    OtherError,

    UefiError(uefi::Error),
    AcpiError(acpi::AcpiError),
    ErrorRaised(Box<Error>),

    Other(Arc<dyn Debug>),
}

impl Display for ErrorType {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

unsafe impl Send for ErrorType {}
unsafe impl Sync for ErrorType {}

#[derive(Debug, Clone)]
pub struct Error {
    pub error_type: ErrorType,
    pub message: Option<Cow<'static, str>>,
    pub caller: Option<&'static Location<'static>>
}

impl Error {
    #[inline]
    #[track_caller]
    pub const fn new(error_type: ErrorType, message: Option<&'static str>) -> Self {
        let message = match message {
            Some(s) => Some(Cow::Borrowed(s)),
            None => None,
        };
        Self {
            error_type,
            message,
            #[cfg(feature = "enable_error_location_caller")]
            caller: Some(Location::caller()),
            #[cfg(not(feature = "enable_error_location_caller"))]
            caller: None,
        }
    }

    #[inline]
    #[track_caller]
    pub fn new_string(error_type: ErrorType, message: Option<String>) -> Self {
        Self {
            error_type,
            message: message.map(Cow::Owned),
            #[cfg(feature = "enable_error_location_caller")]
            caller: Some(Location::caller()),
            #[cfg(not(feature = "enable_error_location_caller"))]
            caller: None,
        }
    }

    #[inline]
    pub const fn raise<T>(self) -> Result<T> {
        Err(self)
    }

    #[inline]
    #[track_caller]
    pub const fn from_uefi(status: uefi::Error, desc: Option<&'static str>) -> Self {
        Error::new(ErrorType::UefiError(status), desc)
    }

    #[inline]
    #[track_caller]
    pub const fn from_acpi(status: acpi::AcpiError, desc: Option<&'static str>) -> Self {
        Error::new(ErrorType::AcpiError(status), desc)
    }

    #[inline]
    #[track_caller]
    pub fn from_self(me: Self, desc: Option<&'static str>) -> Self {
        Error::new(ErrorType::ErrorRaised(Box::new(me)), desc)
    }

    #[track_caller]
    pub fn try_raise<T, E: 'static + Debug>(
        status: core::result::Result<T, E>,
        desc: Option<&'static str>,
    ) -> Result<T> {
        match status {
            Ok(val) => Ok(val),
            Err(error) => {
                let any_err = &error as &dyn Any;

                if let Some(acpi_err) = any_err.downcast_ref::<acpi::AcpiError>() {
                    Self::from_acpi(acpi_err.clone(), desc).raise()
                } else if let Some(uefi_err) = any_err.downcast_ref::<uefi::Error>() {
                    Self::from_uefi(uefi_err.clone(), desc).raise()
                } else if let Some(me) = any_err.downcast_ref::<Error>() {
                    Self::from_self(me.clone(), desc).raise()
                } else {
                    let error: Arc<dyn Debug> = Arc::new(error);

                    Err(Self::new(ErrorType::Other(error), desc))
                }
            }
        }
    }

    #[inline]
    #[track_caller]
    pub fn from_option<T>(
        data: Option<T>,
        e_type: Option<ErrorType>,
        desc: Option<&'static str>,
    ) -> Result<T> {
        data.ok_or_else(|| Error::new(e_type.unwrap_or(ErrorType::ReturnIsNone), desc))
    }
}

impl From<uefi::Error> for Error {
    #[inline]
    #[track_caller]
    fn from(status: uefi::Error) -> Self {
        Self::from_uefi(status, None)
    }
}

impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self.message {
            Some(msg) => write!(f, "[{:?}] {}", self.error_type, msg),
            None => write!(f, "[{:?}] (no message)", self.error_type),
        }
    }
}

impl From<Error> for Box<rhai::EvalAltResult> {
    #[track_caller]
    fn from(err: Error) -> Self {
        Box::new(rhai::EvalAltResult::ErrorSystem(
            format!("{}", err),
            Box::new(err),
        ))
    }
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl core_error::Error for Error {}
impl core::error::Error for Error {}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::new(ErrorType::InternalError, Some("static message"));
        assert_eq!(format!("{}", err), "[InternalError] static message");

        let err_owned = Error::new_string(ErrorType::DeviceError, Some(String::from("owned message")));
        assert!(matches!(err_owned.error_type, ErrorType::DeviceError));
        assert_eq!(err_owned.message.unwrap(), "owned message");
    }

    #[test]
    #[cfg(feature = "enable_error_location_caller")]
    fn test_error_location() {
        let err = Error::new(ErrorType::FileNotFound, None);
        assert!(err.caller.is_some());
    }

    #[test]
    fn test_try_raise_downcast() {
        let inner_err = Error::new(ErrorType::IndexMax, None);
        let result: Result<u32> = Error::try_raise(Err(inner_err), Some("raising again"));

        match result {
            Err(e) => {
                if let ErrorType::ErrorRaised(boxed_err) = e.error_type {
                    assert!(matches!(boxed_err.error_type, ErrorType::IndexMax));
                } else {
                    panic!("Should be ErrorRaised");
                }
            }
            _ => panic!("Should be Err"),
        }
    }
}
/// 結構高度なResultを実装
/// [`Result`]と[`Wirt`]の違い
/// 1. `no-std`でも使えるトレース付き
/// 2. stdでも記録されないエラーの移動の記録を記録
/// 3. 警告を入れれる
/// 4. 結果的な未使用を警告
/// ぐらい
/// ただ、メモリ消費はその代わり多いからそこは注意
/// [`alloc`]前提
/// 注意点
/// [`Result`]との互換性は少しはあるけどきたいしないほうがいい
/// ## 使用するnighilty features
/// 1. `try_trait_v2`
/// 2. `likely_unlikely`
/// 3. `core_intrinsics`
/// ## 使用するfeatures
/// `enable_overprotective_safety_checks`
/// ## 必要な依存関係
///  1. [`alloc`]
///  2. 適切なログシステム([`log_warn`], [`log_error`]を置き換えで解決)

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::{String};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Display, Formatter};
use core::hint::{cold_path, unlikely};
use core::ops::{ControlFlow, FromResidual, Try};
use core::panic::Location;
use acpi::sdt::madt;
use crate::{log_error, log_warn};

const BACK_TRACE_MAX: usize = 1024;

/// last: A system where new data overwrites old data.
/// first: When a new one arrives, if the limit is reached, it will skip over the old one instead of overwriting it.
/// good: A system that caused the error is not deleted; instead, the old version that was used as an intermediary is deleted and a new version is installed.
const TRACE_REMOVE_TYPE: &str = "good";

/// If the data is consecutive and originates from the same location, do not record it.
const TRACE_UNIQUE: bool = true;

/// WIRT (WarnIng Result Trace)
#[must_use]
#[derive(Clone, Debug)]
pub struct Wirt<Item = ()> {
    data: Result<Item, Box<ErrorDetail>>,
    warns: Vec<WarnItem>,
    handled: bool,
}

#[must_use]
#[derive(Clone, Debug, Default)]
struct ErrorDetail {
    ty: ErrorType,
    desc: Option<Cow<'static, str>>,
    #[cfg(feature = "enable_overprotective_safety_checks")]
    log: VecDeque<&'static Location<'static>>,
}

impl<T> Drop for Wirt<T> {
    fn drop(&mut self) {
        #[cfg(all(feature = "enable_overprotective_safety_checks", debug_assertions))]
        if !self.handled && self.data.is_err() {
            log_error!("kernel", "wirt", "non-checking dropping found.");

            if let Err(detail) = &self.data {
                log_error!("kernel", "wirt", "Error: {:?}", detail.ty);

                if let Some(d) = &detail.desc {
                    log_error!("kernel", "wirt", "Description: {}", d);
                }

                #[cfg(feature = "enable_overprotective_safety_checks")]
                if !detail.log.is_empty() {
                    log_error!("kernel", "wirt", "Trace:");
                    for loc in &detail.log {
                        log_error!("kernel", "wirt", "  at {}", loc);
                    }
                }
            }

            self.say_warns();
        }
    }
}

impl ErrorDetail {
    #[track_caller]
    pub fn new(ty: ErrorType, desc: Option<Cow<'static, str>>) -> Self {
        #[cfg(feature = "enable_overprotective_safety_checks")]
        let res = {
            let mut log = VecDeque::new();
            log.push_back(Location::caller());
            Self {
                ty,
                desc,
                log,
            }
        };
        #[cfg(not(feature = "enable_overprotective_safety_checks"))]
        let res = Self {
            ty,
            desc,
        };
        res
    }
}

impl<T> Default for Wirt<T> {
    fn default() -> Self {
        #[cfg(feature = "enable_overprotective_safety_checks")]
        let res = Self {
            data: Err(Box::new(ErrorDetail::default())),
            warns: Vec::new(),
            handled: false,
        };
        res
    }
}

impl<Item> Wirt<Item> {
    #[inline]
    #[allow(non_snake_case)]
    pub fn Ok(item: Item) -> Self {
        Self {
            data: Ok(item),
            handled: false,
            warns: Vec::new(),
        }
    }

    #[inline]
    #[allow(non_snake_case)]
    #[track_caller]
    #[cold]
    pub fn Err(error: ErrorType, desc: Option<&'static str>) -> Self {
        Self {
            data: Err(Box::new(ErrorDetail::new(error, desc.map(|x| {Cow::Borrowed(x)})))),
            handled: false,
            warns: Vec::new(),
        }
    }

    #[inline]
    #[allow(non_snake_case)]
    #[track_caller]
    #[cold]
    pub fn Err_string(error: ErrorType, desc: Option<String>) -> Self {
        Self {
            data: Err(Box::new(ErrorDetail::new(error, desc.map(|x| {Cow::Owned(x)})))),
            handled: false,
            warns: Vec::new(),
        }
    }

    #[inline]
    pub fn add_warn(&mut self, warn: WarnItem) {
        self.warns.push(warn);
    }

    #[inline]
    pub fn add_warns(&mut self, warns: Vec<WarnItem>) {
        self.warns.extend(warns);
    }

    #[inline]
    #[must_use]
    pub const fn is_err(&self) -> bool {
        self.data.is_err()
    }

    #[inline]
    #[must_use]
    pub const fn have_warn(&self) -> bool {
        self.warns.len() != 0
    }

    #[inline]
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.data.is_ok()
    }

    #[inline]
    #[must_use]
    pub fn err(&self) -> Option<&Self> {
        if self.is_err() {
            Some(self)
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub fn get_error_desc(mut self) -> Option<Box<ErrorDetail>> {
        self.handled = true;
        self.data.as_ref().err().cloned()
    }

    #[inline]
    #[must_use]
    pub fn ok(mut self) -> Option<Item> {
        self.handled = true;
        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));

        let data = core::mem::replace(&mut self.data, dummy_data);

        data.ok()
    }

    #[inline]
    #[must_use]
    pub fn warn(&self) -> &[WarnItem] {
        self.warns.as_ref()
    }

    #[inline]
    pub fn unwrap(mut self) -> Item {
        self.handled = true;

        if self.data.is_err() {
            self.expect("failed to unwrap Wirt");
            unreachable!();
        }

        let dummy = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));

        let data = core::mem::replace(&mut self.data, dummy);
        data.unwrap()
    }

    #[track_caller]
    pub fn unwrap_err(mut self) -> Box<ErrorDetail>
    where
        Item: Debug,
    {
        self.handled = true;

        if unlikely(self.is_ok()) {
            panic!("called `Wirt::unwrap_err()` on an `Ok` value");
        }

        self.say_warns();

        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));

        let data = core::mem::replace(&mut self.data, dummy_data);

        data.unwrap_err()
    }

    #[inline]
    #[must_use]
    pub unsafe fn unwrap_unchecked(mut self) -> Item {
        self.handled = true;
        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));

        let data = core::mem::replace(&mut self.data, dummy_data);

        unsafe{data.unwrap_unchecked()}
    }

    #[track_caller]
    pub fn log(&mut self) {
        #[cfg(feature = "enable_overprotective_safety_checks")]
        if let Err(detail) = self.data.as_mut() {
            let loc = Location::caller();

            if TRACE_UNIQUE {
                if let Some(back) = detail.log.back() {
                    if back.file() == loc.file() &&
                        back.line() == loc.line() &&
                        back.column() == loc.column()
                    {
                        return;
                    }
                }
            }

            if detail.log.len() >= BACK_TRACE_MAX {
                if TRACE_REMOVE_TYPE == "first" {
                    return;
                } else if TRACE_REMOVE_TYPE == "last" {
                    detail.log.pop_front();
                } else if TRACE_REMOVE_TYPE == "good" {
                    if detail.log.len() > 2 {
                        detail.log.remove(1);
                    } else {
                        detail.log.pop_front();
                    }
                }
            }
            detail.log.push_back(loc);
        }
        #[cfg(not(feature = "enable_overprotective_safety_checks"))]
        let _ = 5;
    }

    pub fn for_each_log<F>(&self, mut f: F)
    where
        F: FnMut(&'static Location<'static>) -> bool
    {
        #[cfg(feature = "enable_overprotective_safety_checks")]
        if let Err(detail) = self.data.as_ref() {
            for &loc in &detail.log {
                let res = f(loc);
                if res {
                    break;
                }
            }
        }
    }

    #[inline]
    #[track_caller]
    pub fn try_raise_uefi(err: uefi::Result<Item>, detail: Option<&'static str>) -> Self {
        if let Ok(x) = err {
            Self::Ok(x)
        } else {
            cold_path();
            let err = unsafe{err.unwrap_err_unchecked()};

            let err_type = ErrorType::UefiError(Box::new(err));

            Self::Err(err_type, detail)
        }
    }

    #[inline]
    #[track_caller]
    pub fn try_raise_acpi(err: Result<Item, acpi::AcpiError>, detail: Option<&'static str>) -> Self {
        if let Ok(x) = err {
            Self::Ok(x)
        } else {
            cold_path();
            let err = unsafe{err.unwrap_err_unchecked()};
            let err_type = ErrorType::AcpiError(Box::new(err));
            Self::Err(err_type, detail)
        }
    }

    #[inline]
    #[track_caller]
    pub fn try_raise_aml(err: Result<Item, acpi::aml::AmlError>, detail: Option<&'static str>) -> Self {
        if let Ok(x) = err {
            Self::Ok(x)
        } else {
            cold_path();
            let err = unsafe{err.unwrap_err_unchecked()};
            let err_type = ErrorType::AmlError(Box::new(err));
            Self::Err(err_type, detail)
        }
    }

    #[inline]
    #[track_caller]
    pub fn from_option(data: Option<Item>, e_type: ErrorType, detail: Option<&'static str>) -> Self {
        if let Some(item) = data {
            Self::Ok(item)
        } else {
            cold_path();
            Self::Err(e_type, detail)
        }
    }

    #[track_caller]
    pub fn expect(mut self, msg: &str) -> Item {
        self.handled = true;

        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));

        let data = core::mem::replace(&mut self.data, dummy_data);

        match data {
            Ok(t) => {
                self.say_warns();
                t
            },
            Err(e) => panic!("{}: {:?}", msg, e),
        }
    }

    #[inline]
    pub fn say_warns(&self) {
        if self.have_warn() {
            log_warn!("kernel", "wirt", "warn found: {:?}", self.warns);
        }
    }
}

impl<Item> Try for Wirt<Item> {
    type Output = Item;
    type Residual = Self;

    #[inline]
    fn from_output(output: Self::Output) -> Self {
        Self::Ok(output)
    }

    #[track_caller]
    fn branch(mut self) -> ControlFlow<Self::Residual, Self::Output> {
        if self.data.is_ok() {
            self.handled = true;
            self.say_warns();

            let dummy_data = Err(Box::new(ErrorDetail {
                ty: ErrorType::Success,
                desc: None,
                log: VecDeque::new(),
            }));

            let data = core::mem::replace(&mut self.data, dummy_data);

            ControlFlow::Continue(data.unwrap())
        } else {
            self.say_warns();

            ControlFlow::Break(self)
        }
    }
}

impl<Item, T> FromResidual<Result<core::convert::Infallible, Wirt<T>>> for Wirt<Item> {
    #[track_caller]
    fn from_residual(residual: Result<core::convert::Infallible, Wirt<T>>) -> Self {
        let mut wirt = unsafe { residual.unwrap_err_unchecked() };

        wirt.log();

        wirt.handled = true;

        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));
        let dummy_warns = Vec::new();

        let data = core::mem::replace(&mut wirt.data, dummy_data);
        let warns = core::mem::replace(&mut wirt.warns, dummy_warns);

        Self {
            data: Err(unsafe { data.unwrap_err_unchecked() }),
            warns,
            handled: false,
        }
    }
}

impl<Item, T> FromResidual<Wirt<T>> for Wirt<Item> {
    #[track_caller]
    fn from_residual(mut residual: Wirt<T>) -> Self {
        residual.log();
        residual.handled = true;

        let dummy_data = Err(Box::new(ErrorDetail {
            ty: ErrorType::Success,
            desc: None,
            log: VecDeque::new(),
        }));
        let dummy_warns = Vec::new();

        let data = core::mem::replace(&mut residual.data, dummy_data);
        let warns = core::mem::replace(&mut residual.warns, dummy_warns);

        Self {
            data: Err(unsafe { data.unwrap_err_unchecked() }),
            warns,
            handled: false,
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C, i32)]
#[non_exhaustive]
pub enum ErrorType {
    Success = 0x0,
    InternalError = 0x1,
    IoError = 0x2,

    DeviceError = 0x3,
    DriverError = 0x4,
    FirmwareBug = 0x5,

    NotSupported = 0x6,

    AllocationFailed = 0x7,

    InvalidData = 0x8,
    InvalidArgument = 0x9,

    OutOfBounds = 0xA,

    InvalidFileType = 0xB,

    AlreadyUsed = 0xC,
    AlreadyInitialized = 0xD,

    NotFound = 0xE,

    NotInitialized = 0xF,

    Timeout = 0x10,

    UefiError(Box<uefi::Error>) = -0x1,
    AcpiError(Box<acpi::AcpiError>) = -0x2,
    MadtError(Box<madt::MadtError>) = -0x3,
    AmlError(Box<acpi::aml::AmlError>) = -0x4,

    Other(Arc<dyn Debug>) = -0x5,
}

impl Default for ErrorType {
    fn default() -> Self {
        ErrorType::Success
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone)]
#[repr(u32)]
#[non_exhaustive]
#[must_use]
pub enum WarnItem {
    None = 0x0,
}

impl Display for ErrorType {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

unsafe impl Send for ErrorType {}
unsafe impl Sync for ErrorType {}

unsafe impl<T: Send> Send for Wirt<T> {}
unsafe impl<T: Sync> Sync for Wirt<T> {}

impl core_error::Error for Wirt {}

impl core::error::Error for Wirt {}

impl<T: Debug> Display for Wirt<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match &self.data {
            Err(detail) => {
                writeln!(f, "Error: {:?}", detail.ty)?;
                if let Some(d) = &detail.desc {
                    writeln!(f, "Description: {}", d)?;
                }
                #[cfg(feature = "enable_overprotective_safety_checks")]
                if !detail.log.is_empty() {
                    writeln!(f, "Trace:")?;
                    for loc in &detail.log {
                        writeln!(f, "  at {}", loc)?;
                    }
                }
            }
            Ok(data) => writeln!(f, "Ok: {:?}", data)?,
        }

        if !self.warns.is_empty() {
            writeln!(f, "Warnings: {:?}", self.warns)?;
        }
        Ok(())
    }
}
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::Directory;
use uefi::proto::media::fs::SimpleFileSystem;
use crate::util::result;

/// ルートを取得する
/// # Returns
/// 1. [`Directory`]
/// # Errors
/// * [`result::ErrorType::UefiError`]
/// 1. [`LoadedImage`]プロトコルを開けなかった場合
/// 2. ボリュームを開けなかった場合
/// 3 [`SimpleFileSystem`]プロトコルを開けなかった場合
/// * [`result::ErrorType::FileNotFound`]
/// 1. 自身のボリュームが存在しない場合
pub fn get_root() -> result::Result<Directory> {
    let my_handle = uefi::boot::image_handle();

    let my_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(my_handle)?;

    let drive = my_image.device();
    if drive.is_none() {
        return result::Error::new(
            result::ErrorType::FileNotFound,
            Some("Could not open my image drive")
        ).raise();
    }

    Ok(uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(unsafe{drive.unwrap_unchecked()})?.open_volume()?)  // noneじゃないからチェックいらん
}
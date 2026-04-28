use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DICOM error: {0}")]
    Dicom(String),

    #[error("Image decode error: {0}")]
    Image(#[from] image::ImageError),

    #[error("UI error: {0}")]
    Ui(#[from] slint::PlatformError),
}

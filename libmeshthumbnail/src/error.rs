use thiserror::Error;

#[derive(Error, Debug)]
pub enum MeshThumbnailError {
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Failed to read zip file")]
    ZipError(#[from] zip::result::ZipError),
    #[error("Failed to open or read file")]
    FileSystemFault(#[from] std::io::Error),
    #[error("Failed to parse OBJ file: {0}")]
    ObjParseError(#[from] wavefront_obj::ParseError),
    #[error("Failed to parse 3mf file: {0}")]
    ThreemfParseError(#[from] threemf::Error),
    #[error("Gcode number parsing error: {0}")]
    GcodeNumberParseError(#[from] std::num::ParseFloatError),
    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),
    #[cfg(feature = "step")]
    #[error("STEP parsing error: {0}")]
    StepParseError(#[from] opencascade::Error),
}
use std::path::PathBuf;

use image::DynamicImage;

mod threemf;
mod gcode;

pub fn handle_extract_image(
    input_path : &PathBuf
) -> Result<Option<DynamicImage>, crate::error::MeshThumbnailError>
{
    if let Some(result) = threemf::handle_threemf(input_path)? {
        return Ok(Some(result));
    }

    if let Some(result) = gcode::handle_gcode(input_path)? {
        return Ok(Some(result));
    }

    Ok(None)
}
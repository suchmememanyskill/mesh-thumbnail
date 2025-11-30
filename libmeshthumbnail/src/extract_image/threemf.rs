use std::{fs::File, io::{self, Cursor}, path::PathBuf};

use image::{DynamicImage, ImageReader};
use zip::ZipArchive;

use crate::error::MeshThumbnailError;

pub fn handle_threemf(input_path : &PathBuf) -> Result<Option<DynamicImage>, MeshThumbnailError>
{
    let input_path_str = input_path.to_string_lossy().to_lowercase();

    if input_path_str.ends_with(".3mf") {
        Ok(Some(extract_image_from_3mf(input_path)?))
    } else {
        Ok(None)
    }
}

fn extract_image_from_3mf(
    input_path : &PathBuf
) -> Result<DynamicImage, MeshThumbnailError> {
    // Open 3mf path as zip file
    let file = File::open(input_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with("thumbnail_middle.png") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            io::copy(&mut file, &mut buffer)?;

            let step1 = ImageReader::new(Cursor::new(buffer)).with_guessed_format()?.decode()?;

            return Ok(step1);
        }
    }

    Err(MeshThumbnailError::InternalError(String::from("thumbnail_middle.png not found in 3mf file")))
}
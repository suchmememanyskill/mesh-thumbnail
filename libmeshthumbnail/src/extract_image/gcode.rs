use std::{fs::File, io::{BufRead, BufReader, Cursor, Read}, path::PathBuf};

use base64::{Engine, prelude::BASE64_STANDARD};
use image::{DynamicImage, ImageReader};
use zip::ZipArchive;

use crate::error::MeshThumbnailError;

pub fn handle_gcode(
    input_path : &PathBuf, 
) -> Result<Option<DynamicImage>, MeshThumbnailError>
{
    let input_path_str = input_path.to_string_lossy().to_lowercase();

    if input_path_str.ends_with(".gcode") {
        Ok(Some(extract_image_from_gcode_file(input_path)?))
    } else if input_path_str.ends_with(".gcode.zip") {
        Ok(Some(extract_iamge_from_gcode_zip(input_path)?))
    } else {
        Ok(None)
    }
}

struct GcodeImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl GcodeImage {
    fn area(&self) -> u32 {
        self.width * self.height
    }
}

fn extract_image_from_gcode_file(
    gcode_path : &PathBuf,
) -> Result<DynamicImage, MeshThumbnailError> {
    let mut file = File::open(gcode_path)?;
    extract_image_from_gcode(&mut file)
}

fn extract_iamge_from_gcode_zip(
    gcode_zip_path : &PathBuf,
) -> Result<DynamicImage, MeshThumbnailError> {
    let file = File::open(gcode_zip_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".gcode") {
            return extract_image_from_gcode(&mut file);
        }
    }

    Err(MeshThumbnailError::InternalError(String::from("No gcode file found in zip archive")))
}

fn extract_image_from_gcode<W>(
    reader : &mut W,
) -> Result<DynamicImage, MeshThumbnailError> where W: Read {
    let buffered_reader = BufReader::new(reader);
    let mut gcode_images : Vec<GcodeImage> = Vec::new();
    let mut in_gcode_section = false;
    let mut gcode_img_width = 0;
    let mut gcode_img_height = 0;
    let mut image = String::from("");

    for line in buffered_reader.lines().map_while(Result::ok) {
        if line.starts_with("; thumbnail begin") {
            let pixel_format = match line.split(" ").skip(3).next() {
                Some(s) => s,
                None => continue
            };

            let pixel_format_unpacked: Vec<u32> = pixel_format.split("x").map(|f| f.parse().unwrap_or_default()).collect();

            gcode_img_width = pixel_format_unpacked.get(0).unwrap_or(&0).clone();
            gcode_img_height = pixel_format_unpacked.get(1).unwrap_or(&0).clone();
            image = String::from("");

            in_gcode_section = gcode_img_width > 0 && gcode_img_height > 0;
        }
        else if line.starts_with("; thumbnail end") {
            in_gcode_section = false;
            let image = match BASE64_STANDARD.decode(&image)  {
                Ok(data) => data,
                Err(e) => {
                    println!("Error decoding base64 image data: {}", e);
                    continue;
                },
            };

            gcode_images.push(GcodeImage { width: gcode_img_width, height: gcode_img_height, data: image  });
            
        }
        else if in_gcode_section {
            image.push_str(line[2..].trim());
        }
        else if line.starts_with("; EXECUTABLE_BLOCK_START") {
            break;
        }
    }

    gcode_images.sort_by(|a, b| b.area().cmp(&a.area()));
    println!("Found {} thumbnails in gcode file", gcode_images.len());

    let largest_image = match gcode_images.first() {
        Some(x) => x,
        None => return Err(MeshThumbnailError::InternalError(String::from("No thumbnail found in gcode file"))),
    };

    println!("Using gcode image {}x{}", largest_image.width, largest_image.height);

    let step1 = ImageReader::new(Cursor::new(&largest_image.data)).with_guessed_format()?.decode()?;

    return Ok(step1);
}
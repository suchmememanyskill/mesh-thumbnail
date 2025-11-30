
use clap::{Parser, ValueEnum};
use image::imageops::FilterType::Triangle;
use libmeshthumbnail::{extract_image, parse_model, render};
use std::path::PathBuf;
use std::path;
use std::num::ParseIntError;
use std::time::Instant;
use vek::*;

#[derive(Parser, Debug)]
#[command(name = "mesh-thumbnail", about = "3D file thumbnail generator", version = "0.1")]
struct Args {
    /// Rotation around the X-axis
    #[arg(long, default_value_t = 0.0)]
    #[clap(allow_hyphen_values = true)]
    rotatex: f32,

    /// Rotation around the Y-axis
    #[arg(long, default_value_t = 0.0)]
    #[clap(allow_hyphen_values = true)]
    rotatey: f32,

    /// Output directory (default: current folder)
    #[arg(long, default_value = ".")]
    outdir: String,

    /// Image width
    #[arg(long, default_value_t = 512)]
    width: u32,

    /// Image height
    #[arg(long, default_value_t = 512)]
    height: u32,

    /// Output image format
    #[arg(long, default_value_t = Format::Png, value_enum)]
    format: Format,

    /// Background color in hex format (default: Grey)
    #[arg(long, default_value = "DDDDDD")]
    color: String,

    /// Overwrite existing output files
    #[arg(long, default_value_t = false)]
    overwrite: bool,

    /// Input files (at least one required)
    #[arg(required = true)]
    files: Vec<String>,

    /// Fallback on thumbnail inside 3mf files
    #[arg(long, default_value_t = false)]
    fallback_3mf_thumbnail: bool,

    /// Prefer 3mf thumbnail over 3mf model
    #[arg(long, default_value_t = false)]
    prefer_3mf_thumbnail: bool,

    // Prefer gcode thumbnail over gcode model
    #[arg(long, default_value_t = false)]
    prefer_gcode_thumbnail: bool,

    #[arg(long, default_value_t = 1)]
    /// Amount of images to generate per file
    images_per_file: u32,

    #[arg(long, default_value_t = 1.0)]
    zoom: f32,
}

fn parse_hex_color(s: &str) -> Result<u32, ParseIntError> {
    u32::from_str_radix(s, 16)
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
enum Format {
    Jpg,
    Png,
}

impl ToString for Format {
    fn to_string(&self) -> String {
      match self {
        Format::Jpg => String::from("jpg"),
        Format::Png => String::from("png"),  
      }
    }
  }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();

    if args.prefer_3mf_thumbnail
    {
        args.fallback_3mf_thumbnail = false;
    }

    if args.images_per_file < 1
    {
        args.images_per_file = 1;
    }

    if args.images_per_file > 1 && args.rotatex != 0.0
    {
        eprintln!("Warning: rotatex is ignored when generating multiple images per file.");
        args.rotatex = 0.0;
    }

    let color_u32 = parse_hex_color(&args.color).unwrap_or(0xDDDDDD);
    let color = Vec3::new(
        ((color_u32 >> 16) & 0xFF) as u8,
        ((color_u32 >> 8) & 0xFF) as u8,
        (color_u32 & 0xFF) as u8,
    );

    println!("Parsed arguments: {:#?}", args);
    let instant = Instant::now();

    for file in args.files {
        let absolute_path = path::absolute(&file).unwrap();
        let mut extension = absolute_path.extension().take().unwrap().to_str().take().unwrap();
        let filename = absolute_path.file_name().take().unwrap().to_str().take().unwrap();

        if filename.ends_with(".stl.zip")
        {
            extension = "stl.zip";
        }

        if filename.ends_with(".obj.zip")
        {
            extension = "obj.zip";
        }

        if filename.ends_with(".gcode.zip")
        {
            extension = "gcode.zip";
        }

        let filename_image = format!("{}{}", &filename[..filename.len() - extension.len()] ,args.format.to_string());
        let image_path = PathBuf::from(args.outdir.clone()).join(filename_image);

        if !args.overwrite && image_path.exists()
        {
            println!("Path {:?} already exists, skipping {}...", image_path, filename);
            continue;
        }

        if (args.prefer_3mf_thumbnail && filename.ends_with(".3mf"))
            || (args.prefer_gcode_thumbnail && (filename.ends_with(".gcode") || filename.ends_with(".gcode.zip"))) {
                if let Ok(Some(mut image)) = extract_image::handle_extract_image(&absolute_path) {
                    println!("Extracted thumbnail from {}, saving to {:?}...", filename, image_path);
                    image = image.resize_to_fill(args.width, args.height, Triangle);
                    image.save(&image_path)?;
                    continue;
                }
            }

        let x_coords = if args.images_per_file > 1 {
            (0..args.images_per_file).map(|i| i as f32 * 360.0 / args.images_per_file as f32).collect::<Vec<f32>>()
        } else {
            vec![args.rotatex]
        };

        let mesh = match parse_model::handle_parse(&absolute_path) {
            Ok(Some(mesh)) => Some(mesh),
            Ok(None) => {
                println!("No parsers could handle file {}", filename);
                None
            },
            Err(e) => {
                println!("Error parsing file {}: {}", filename, e);
                None
            }
        };

        if let Some(mesh) = mesh {
            for (i, x) in x_coords.iter().enumerate() {
                let mut image_path = image_path.clone();
                if args.images_per_file > 1 {
                    let new_name = format!("{}-{:02}", image_path.file_stem().unwrap().to_str().unwrap(), i);
                    replace_file_stem(&mut image_path, &new_name);
                }

                let render = render::render(&mesh, Vec2::new(args.width as usize, args.height as usize), 
                    Vec3::new(*x, args.rotatey, 0.0), 
                    color, 
                    args.zoom);

                render.save(&image_path)?;
                println!("Rendered {} to {:?}...", filename, image_path);
            }

            continue;
        }


        if args.fallback_3mf_thumbnail && filename.ends_with(".3mf") {
            if let Ok(Some(mut image)) = extract_image::handle_extract_image(&absolute_path) {
                println!("Extracted thumbnail from {}, saving to {:?}...", filename, image_path);
                image = image.resize_to_fill(args.width, args.height, Triangle);
                image.save(&image_path)?;
                continue;
            }
        }

        println!("Failed to generate thumbnail for {}.", filename);
    }

    println!("Fully done in {:.2?}!", instant.elapsed());

    Ok(())
}

fn replace_file_stem(path: &mut PathBuf, new_stem: &str) {
    if let Some(ext) = path.extension() {
        path.set_file_name(format!("{}.{}", new_stem, ext.to_string_lossy()));
    } else {
        path.set_file_name(new_stem);
    }
}
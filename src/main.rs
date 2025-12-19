use clap::Parser;
use std::path::PathBuf;

use mesh_thumbnail::{Format, ThumbnailOptions, generate_thumbnail_for_file};

#[derive(Parser, Debug)]
#[command(
    name = "mesh-thumbnail",
    about = "3D file thumbnail generator",
    version = "0.1"
)]
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

    /// Prefer gcode thumbnail over gcode model
    #[arg(long, default_value_t = false)]
    prefer_gcode_thumbnail: bool,

    #[arg(long, default_value_t = 1)]
    /// Amount of images to generate per file
    images_per_file: u32,

    #[arg(long, default_value_t = 1.0)]
    /// Scale factor for the camera
    inverse_zoom: f32,
}

fn main() {
    let mut args = Args::parse();

    if args.prefer_3mf_thumbnail {
        args.fallback_3mf_thumbnail = false;
    }

    if args.images_per_file < 1 {
        args.images_per_file = 1;
    }

    if args.images_per_file > 1 && args.rotatex != 0.0 {
        eprintln!("Warning: rotatex is ignored when generating multiple images per file.");
        args.rotatex = 0.0;
    }

    println!("Parsed arguments: {:#?}", args);

    let options = ThumbnailOptions {
        rotatex: args.rotatex,
        rotatey: args.rotatey,
        width: args.width,
        height: args.height,
        format: args.format,
        color: args.color,
        overwrite: args.overwrite,
        fallback_3mf_thumbnail: args.fallback_3mf_thumbnail,
        prefer_3mf_thumbnail: args.prefer_3mf_thumbnail,
        prefer_gcode_thumbnail: args.prefer_gcode_thumbnail,
        images_per_file: args.images_per_file,
        inverse_zoom: args.inverse_zoom,
    };

    let outdir = PathBuf::from(&args.outdir);

    for file in args.files {
        let path = PathBuf::from(&file);
        if let Err(e) = generate_thumbnail_for_file(&path, &outdir, &options) {
            eprintln!("Error while converting {}: {:?}.", file, e);
        }
    }
}

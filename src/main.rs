use clap::Parser;
use image::{imageops::FilterType::Triangle, ImageReader};
use std::{io::Read, num::ParseIntError, path::PathBuf};
use clap::ValueEnum;
use std::path;
use three_d::*;
use three_d_asset::io::Serialize;
use std::fs::File;
use zip::ZipArchive;
use std::io::Cursor;

mod parse_mesh;
mod solid_material;

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

fn main() {
    let mut args = Args::parse();

    if args.prefer_3mf_thumbnail
    {
        args.fallback_3mf_thumbnail = false;
    }

    println!("Parsed arguments: {:#?}", args);

    let viewport = Viewport::new_at_origo(args.width, args.height);
    let context = HeadlessContext::new().unwrap();
    let alpha = if args.format == Format::Jpg { 0.8 } else { 0.0 };

    // Create a color texture to render into
    let mut texture = Texture2D::new_empty::<[u8; 4]>(
        &context,
        viewport.width,
        viewport.height,
        Interpolation::Nearest,
        Interpolation::Nearest,
        None,
        Wrapping::ClampToEdge,
        Wrapping::ClampToEdge,
    );
        
    // Also create a depth texture to support depth testing
    let mut depth_texture = DepthTexture2D::new::<f32>(
        &context,
        viewport.width,
        viewport.height,
        Wrapping::ClampToEdge,
        Wrapping::ClampToEdge,
    );

    for file in args.files
    {
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

        let filename_image = format!("{}{}", &filename[..filename.len() - extension.len()] ,args.format.to_string());
        let image_path = PathBuf::from(args.outdir.clone()).join(filename_image);
        let image_path_str = image_path.to_str().take().unwrap();

        if !args.overwrite && path::Path::new(image_path_str).exists()
        {
            println!("Path {} already exists, skipping {}...", image_path_str, filename);
            continue;
        }

        if args.prefer_3mf_thumbnail && filename.ends_with(".3mf")
        {
            if extract_image_from_3mf(&absolute_path, args.width, args.height, &image_path).is_ok()
            {
                continue;
            }
        }

        let possible_mesh = parse_mesh::parse_file((&absolute_path).to_str().take().unwrap());

        if let Ok(mesh) = possible_mesh {
            render_model(&context, &viewport, &mesh, alpha, &file, &image_path, &args.color, args.rotatex, args.rotatey, &mut texture, &mut depth_texture);
        } else if let Err(e) = possible_mesh {
            println!("Error while converting {}: {}.", filename, e.to_string());

            if args.fallback_3mf_thumbnail && filename.ends_with(".3mf") && !args.prefer_3mf_thumbnail
            {
                if extract_image_from_3mf(&absolute_path, args.width, args.height, &image_path).is_err()
                {
                    println!("Fallback of extracting image also failed...");
                }
            }
        }
    }
}

fn render_model(
    context: &HeadlessContext,
    viewport: &Viewport,
    mesh: &CpuMesh,
    alpha: f32,
    file: &str,
    image_path: &PathBuf,
    color : &str,
    rotatex: f32,
    rotatey: f32,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
) {
    let color = parse_hex_color(color).unwrap();
    let mut model = Gm::new(
        Mesh::new(&context, &mesh),
        solid_material::SolidMaterial::new_opaque(&context,
            &CpuMaterial {
                albedo: Srgba::new_opaque((color >> 16 & 0xFF) as u8, (color >> 8 & 0xFF) as u8, (color & 0xFF) as u8),
                ..Default::default()
            }),
        );

    let mut offset = Mat4::from_translation(model.aabb().min() * -1.0) * Mat4::from_translation((model.aabb().min() - model.aabb().max()) / 2f32);
    
    if file.ends_with(".stl") 
        || file.ends_with(".stl.zip")
        || file.ends_with(".3mf")
        || file.ends_with(".obj")
        || file.ends_with(".obj.zip")
    {
        offset = Mat4::from_angle_x(Deg(270.0)) * offset;
    }
    
    model.set_transformation(offset);

    let magnitude = (model.aabb().min() - model.aabb().max()).magnitude();

    let mut camera = Camera::new_perspective(
        viewport.clone(),
        vec3(0.0, 0.0, magnitude),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        magnitude * 0.01,
        1000.0,
    );
    let target = camera.target();
    camera.rotate_around_with_fixed_up(target, (3.14 * 2.0) * (rotatex / 360.0), (3.14 * 2.0) * (rotatey / 360.0));

    let pixels : Vec<[u8; 4]> = RenderTarget::new(
        texture.as_color_target(None),
        depth_texture.as_depth_target(),
    )
    // Clear color and depth of the render target
    .clear(ClearState::color_and_depth(0.2, 0.2, 0.2, alpha, 1.0))
    // Render the triangle with the per vertex colors defined at construction
    .render(&camera, &model, &[])
    .read_color();

    three_d_asset::io::save(
        &CpuTexture {
            data: TextureData::RgbaU8(pixels),
            width: texture.width(),
            height: texture.height(),
            ..Default::default()
        }
        .serialize(image_path)
        .unwrap(),
    )
    .unwrap();
}

fn extract_image_from_3mf(
    threemf_path : &PathBuf,
    width : u32,
    height : u32,
    image_path : &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open 3mf path as zip file
    let file = File::open(threemf_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with("thumbnail_middle.png") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;

            let step1 = ImageReader::new(Cursor::new(buffer)).with_guessed_format()?.decode()?;
            let step2 = step1.resize_to_fill(width, height, Triangle);

            step2.save(image_path)?;
            return Ok(());
        }
    }

    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "thumbnail_middle.png not found in 3mf file",
    )))
}
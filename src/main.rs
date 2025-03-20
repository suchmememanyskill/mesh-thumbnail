use clap::Parser;
use std::{num::ParseIntError, path::PathBuf};
use clap::ValueEnum;
use std::path;
use three_d::*;
use three_d_asset::io::Serialize;

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
    let args = Args::parse();
    println!("Parsed arguments: {:#?}", args);

    let viewport = Viewport::new_at_origo(args.width, args.height);
    let context = HeadlessContext::new().unwrap();
    let alpha = if args.format == Format::Jpg { 0.0 } else { 0.0 };

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
        let absolute_path = path::absolute(file).unwrap();
        let mut extension = absolute_path.extension().take().unwrap().to_str().take().unwrap();
        let filename = absolute_path.file_name().take().unwrap().to_str().take().unwrap();

        if filename.ends_with(".stl.zip")
        {
            extension = "stl.zip";
        }

        let filename_image = format!("{}{}", &filename[..filename.len() - extension.len()] ,args.format.to_string());
        let image_path = PathBuf::from(args.outdir.clone()).join(filename_image);
        let image_path_str = image_path.to_str().take().unwrap();

        if !args.overwrite && path::Path::new(image_path_str).exists()
        {
            println!("Path {} already exists, skipping {}...", image_path_str, filename);
            continue;
        }

        let mesh = match parse_mesh::parse_file(absolute_path.to_str().take().unwrap()) 
        {
            Ok(v) => v,
            Err(e) => {
                println!("Error while converting {}: {}.", filename, e.to_string());
                continue;
            }
        };

        let color = parse_hex_color(&args.color).unwrap();
        let mut model = Gm::new(
            Mesh::new(&context, &mesh),
            solid_material::SolidMaterial::new_opaque(&context,
                &CpuMaterial {
                    albedo: Srgba::new_opaque((color >> 16 & 0xFF) as u8, (color >> 8 & 0xFF) as u8, (color & 0xFF) as u8),
                    ..Default::default()
                }),
            );

        let offset = Mat4::from_translation(model.aabb().min() * -1.0) * Mat4::from_translation((model.aabb().min() - model.aabb().max()) / 2f32);
        model.set_transformation(Mat4::from_angle_x(Deg(270.0)) * offset);

        let magnitude = (model.aabb().min() - model.aabb().max()).magnitude();

        let mut camera = Camera::new_perspective(
            viewport,
            vec3(0.0, 0.0, magnitude),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            degrees(45.0),
            magnitude * 0.01,
            1000.0,
        );
        let target = camera.target();
        camera.rotate_around_with_fixed_up(target, (3.14 * 2.0) * (args.rotatex / 360.0), (3.14 * 2.0) * (args.rotatey / 360.0));


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
}
use base64::{Engine, prelude::BASE64_STANDARD};
use clap::ValueEnum;
use image::{
    DynamicImage, ImageFormat, ImageReader, RgbaImage, imageops::FilterType::Triangle,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::{self, Path, PathBuf};
use three_d::*;
use three_d_asset::io::Serialize;
use zip::{ZipArchive, result::ZipError};

pub mod parse_mesh;
pub mod solid_material;

#[cfg(feature = "python")]
mod python;

pub use parse_mesh::{MeshWithTransform, ParseError, ParseResult};
pub use solid_material::SolidMaterial;

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub enum Format {
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

#[derive(Debug, Clone)]
pub struct ThumbnailOptions {
    pub rotatex: f32,
    pub rotatey: f32,
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub color: String,
    pub overwrite: bool,
    pub fallback_3mf_thumbnail: bool,
    pub prefer_3mf_thumbnail: bool,
    pub prefer_gcode_thumbnail: bool,
    pub images_per_file: u32,
    pub inverse_zoom: f32,
}

impl Default for ThumbnailOptions {
    fn default() -> Self {
        Self {
            rotatex: 0.0,
            rotatey: 0.0,
            width: 512,
            height: 512,
            format: Format::Png,
            color: String::from("DDDDDD"),
            overwrite: false,
            fallback_3mf_thumbnail: false,
            prefer_3mf_thumbnail: false,
            prefer_gcode_thumbnail: false,
            images_per_file: 1,
            inverse_zoom: 1.0,
        }
    }
}

#[derive(Debug)]
pub enum ThumbnailError {
    Io(String),
    Parse(String),
    Other(String),
}

impl std::fmt::Display for ThumbnailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThumbnailError::Io(e) => write!(f, "I/O error: {}", e),
            ThumbnailError::Parse(e) => write!(f, "Parse error: {}", e),
            ThumbnailError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ThumbnailError {}

impl From<std::io::Error> for ThumbnailError {
    fn from(e: std::io::Error) -> Self {
        ThumbnailError::Io(e.to_string())
    }
}

impl From<ZipError> for ThumbnailError {
    fn from(e: ZipError) -> Self {
        ThumbnailError::Io(e.to_string())
    }
}

impl From<parse_mesh::ParseError> for ThumbnailError {
    fn from(e: parse_mesh::ParseError) -> Self {
        ThumbnailError::Parse(e.to_string())
    }
}

pub fn generate_thumbnail_for_file(
    file: &Path,
    outdir: &Path,
    options: &ThumbnailOptions,
) -> Result<(), ThumbnailError> {
    let mut options = options.clone();

    if options.images_per_file < 1 {
        options.images_per_file = 1;
    }

    if options.images_per_file > 1 && options.rotatex != 0.0 {
        options.rotatex = 0.0;
    }

    let viewport = Viewport::new_at_origo(options.width, options.height);
    let context = HeadlessContext::new().map_err(|e| ThumbnailError::Other(e.to_string()))?;
    let alpha = if options.format == Format::Jpg {
        0.8
    } else {
        0.0
    };

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

    let mut depth_texture = DepthTexture2D::new::<f32>(
        &context,
        viewport.width,
        viewport.height,
        Wrapping::ClampToEdge,
        Wrapping::ClampToEdge,
    );

    generate_thumbnail_for_file_with_context(
        &context,
        &viewport,
        file,
        outdir,
        alpha,
        &mut texture,
        &mut depth_texture,
        &options,
    )
}

fn generate_thumbnail_bytes_for_file_with_context(
    context: &HeadlessContext,
    viewport: &Viewport,
    file: &Path,
    alpha: f32,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
    options: &ThumbnailOptions,
) -> Result<Vec<u8>, ThumbnailError> {
    let absolute_path = path::absolute(file)?;
    let filename = absolute_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ThumbnailError::Other(String::from("Invalid filename")))?;

    if options.prefer_3mf_thumbnail && filename.ends_with(".3mf") {
        if let Ok(bytes) = extract_image_from_3mf_to_bytes(
            &absolute_path,
            options.width,
            options.height,
            &options.format,
        ) {
            return Ok(bytes);
        }
    }

    if options.prefer_gcode_thumbnail {
        if filename.ends_with(".gcode") {
            if let Ok(bytes) = extract_image_from_gcode_file_to_bytes(
                &absolute_path,
                options.width,
                options.height,
                &options.format,
            ) {
                return Ok(bytes);
            }
        } else if filename.ends_with(".gcode.zip") {
            if let Ok(bytes) = extract_image_from_gcode_zip_to_bytes(
                &absolute_path,
                options.width,
                options.height,
                &options.format,
            ) {
                return Ok(bytes);
            }
        }
    }

    let possible_mesh = parse_mesh::parse_file(
        absolute_path
            .to_str()
            .ok_or_else(|| ThumbnailError::Other(String::from("Invalid path encoding")))?,
    );

    match possible_mesh {
        Ok(parse_result) => render_parse_result_to_bytes(
            context,
            viewport,
            texture,
            depth_texture,
            &parse_result,
            alpha,
            filename,
            &options.color,
            options.rotatex,
            options.rotatey,
            options.inverse_zoom,
            &options.format,
        ),
        Err(e) => {
            if options.fallback_3mf_thumbnail
                && filename.ends_with(".3mf")
                && !options.prefer_3mf_thumbnail
            {
                if let Ok(bytes) = extract_image_from_3mf_to_bytes(
                    &absolute_path,
                    options.width,
                    options.height,
                    &options.format,
                ) {
                    return Ok(bytes);
                }
            }
            Err(ThumbnailError::Parse(e.to_string()))
        }
    }
}

pub fn generate_thumbnail_bytes_for_file(
    file: &Path,
    options: &ThumbnailOptions,
) -> Result<Vec<u8>, ThumbnailError> {
    let mut options = options.clone();
    options.images_per_file = 1;

    let viewport = Viewport::new_at_origo(options.width, options.height);
    let context = HeadlessContext::new().map_err(|e| ThumbnailError::Other(e.to_string()))?;
    let alpha = if options.format == Format::Jpg {
        0.8
    } else {
        0.0
    };

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

    let mut depth_texture = DepthTexture2D::new::<f32>(
        &context,
        viewport.width,
        viewport.height,
        Wrapping::ClampToEdge,
        Wrapping::ClampToEdge,
    );

    generate_thumbnail_bytes_for_file_with_context(
        &context,
        &viewport,
        file,
        alpha,
        &mut texture,
        &mut depth_texture,
        &options,
    )
}

fn generate_thumbnail_for_file_with_context(
    context: &HeadlessContext,
    viewport: &Viewport,
    file: &Path,
    outdir: &Path,
    alpha: f32,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
    options: &ThumbnailOptions,
) -> Result<(), ThumbnailError> {
    let absolute_path = path::absolute(file)?;
    let filename = absolute_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ThumbnailError::Other(String::from("Invalid filename")))?;

    let mut extension = absolute_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if filename.ends_with(".stl.zip") {
        extension = "stl.zip";
    }

    if filename.ends_with(".obj.zip") {
        extension = "obj.zip";
    }

    if filename.ends_with(".gcode.zip") {
        extension = "gcode.zip";
    }

    let filename_image = format!(
        "{}{}",
        &filename[..filename.len() - extension.len()],
        options.format.to_string()
    );

    let image_path = outdir.join(filename_image);

    if !options.overwrite && image_path.exists() {
        return Ok(());
    }

    if options.prefer_3mf_thumbnail && filename.ends_with(".3mf") {
        if extract_image_from_3mf(&absolute_path, options.width, options.height, &image_path)
            .is_ok()
        {
            return Ok(());
        }
    }

    if options.prefer_gcode_thumbnail {
        if filename.ends_with(".gcode") {
            if extract_image_from_gcode_file(
                &absolute_path,
                options.width,
                options.height,
                &image_path,
            )
            .is_ok()
            {
                return Ok(());
            }
        } else if filename.ends_with(".gcode.zip") {
            if extract_image_from_gcode_zip(
                &absolute_path,
                options.width,
                options.height,
                &image_path,
            )
            .is_ok()
            {
                return Ok(());
            }
        }
    }

    let possible_mesh = parse_mesh::parse_file(
        absolute_path
            .to_str()
            .ok_or_else(|| ThumbnailError::Other(String::from("Invalid path encoding")))?,
    );

    match possible_mesh {
        Ok(parse_result) => {
            render_model(
                context,
                viewport,
                texture,
                depth_texture,
                &parse_result,
                alpha,
                filename,
                &image_path,
                &options.color,
                options.rotatex,
                options.rotatey,
                options.images_per_file,
                options.inverse_zoom,
            );
            Ok(())
        }
        Err(e) => {
            if options.fallback_3mf_thumbnail
                && filename.ends_with(".3mf")
                && !options.prefer_3mf_thumbnail
            {
                if extract_image_from_3mf(
                    &absolute_path,
                    options.width,
                    options.height,
                    &image_path,
                )
                .is_err()
                {
                    return Err(ThumbnailError::Parse(e.to_string()));
                }
                Ok(())
            } else {
                Err(ThumbnailError::Parse(e.to_string()))
            }
        }
    }
}

fn viewport_from_texture(texture: &Texture2D) -> Viewport {
    Viewport::new_at_origo(texture.width(), texture.height())
}

fn parse_hex_color(s: &str) -> Result<u32, std::num::ParseIntError> {
    u32::from_str_radix(s, 16)
}

fn render_model(
    context: &HeadlessContext,
    viewport: &Viewport,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
    parse_result: &parse_mesh::ParseResult,
    alpha: f32,
    file: &str,
    image_path: &PathBuf,
    color: &str,
    rotatex: f32,
    rotatey: f32,
    count: u32,
    scale: f32,
) {
    let mut models = build_models(context, parse_result, color);
    let width = texture.width();
    let height = texture.height();

    for iter in 0..count {
        let mut iter_file_path = image_path.clone();
        let mut local_rotatex = rotatex;

        if count > 1 {
            let new_name = format!(
                "{}-{:02}",
                iter_file_path.file_stem().unwrap().to_str().unwrap(),
                iter
            );
            replace_file_stem(&mut iter_file_path, &new_name);
        }

        if iter > 0 {
            local_rotatex += (360.0 / count as f32) * iter as f32;
        }

        let pixels = render_pixels_for_view(
            &mut models,
            parse_result,
            file,
            viewport,
            texture,
            depth_texture,
            alpha,
            local_rotatex,
            rotatey,
            scale,
        );

        save_pixels_to_path(pixels, width, height, &iter_file_path);
    }
}

fn build_models(
    context: &HeadlessContext,
    parse_result: &parse_mesh::ParseResult,
    color: &str,
) -> Vec<Gm<Mesh, solid_material::SolidMaterial>> {
    let default_color = parse_hex_color(color).unwrap();
    let default_srgba = Srgba::new_opaque(
        (default_color >> 16 & 0xFF) as u8,
        (default_color >> 8 & 0xFF) as u8,
        (default_color & 0xFF) as u8,
    );

    parse_result
        .meshes
        .iter()
        .map(|mesh_with_transform| {
            let albedo = mesh_with_transform.color.unwrap_or(default_srgba);

            Gm::new(
                Mesh::new(&context, &mesh_with_transform.mesh),
                solid_material::SolidMaterial::new_opaque(
                    &context,
                    &CpuMaterial {
                        albedo,
                        ..Default::default()
                    },
                ),
            )
        })
        .collect()
}

fn render_pixels_for_view(
    models: &mut [Gm<Mesh, solid_material::SolidMaterial>],
    parse_result: &parse_mesh::ParseResult,
    file: &str,
    viewport: &Viewport,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
    alpha: f32,
    rotatex: f32,
    rotatey: f32,
    scale: f32,
) -> Vec<[u8; 4]> {
    let mut combined_min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut combined_max = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for (idx, model) in models.iter_mut().enumerate() {
        let mesh_transform = parse_result.meshes[idx].transform;
        model.set_transformation(mesh_transform);

        let aabb = model.aabb();
        combined_min = vec3(
            combined_min.x.min(aabb.min().x),
            combined_min.y.min(aabb.min().y),
            combined_min.z.min(aabb.min().z),
        );
        combined_max = vec3(
            combined_max.x.max(aabb.max().x),
            combined_max.y.max(aabb.max().y),
            combined_max.z.max(aabb.max().z),
        );
    }

    let mut offset = Mat4::from_translation(combined_min * -1.0)
        * Mat4::from_translation((combined_min - combined_max) / 2f32);

    if file.ends_with(".stl")
        || file.ends_with(".stl.zip")
        || file.ends_with(".3mf")
        || file.ends_with(".obj")
        || file.ends_with(".obj.zip")
    {
        offset = Mat4::from_angle_x(Deg(270.0)) * offset;
    } else if file.ends_with("gcode") || file.ends_with("gcode.zip") {
        offset = Mat4::from_angle_y(Deg(180.0)) * offset;
    }

    for (idx, model) in models.iter_mut().enumerate() {
        let mesh_transform = parse_result.meshes[idx].transform;
        model.set_transformation(offset * mesh_transform);
    }

    let magnitude = (combined_min - combined_max).magnitude() * scale;

    let pitch = rotatey.clamp(-90.0, 90.0).to_radians();
    let yaw = rotatex.to_radians();

    let x = magnitude * pitch.cos() * yaw.sin();
    let y = magnitude * pitch.sin();
    let z = magnitude * pitch.cos() * yaw.cos();

    let camera = Camera::new_perspective(
        viewport.clone(),
        vec3(x, y, z),
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        degrees(45.0),
        magnitude * 0.01,
        1000.0,
    );

    let model_refs: Vec<&dyn Object> = models.iter().map(|m| m as &dyn Object).collect();

    RenderTarget::new(
        texture.as_color_target(None),
        depth_texture.as_depth_target(),
    )
    .clear(ClearState::color_and_depth(0.2, 0.2, 0.2, alpha, 1.0))
    .render(&camera, &model_refs, &[])
    .read_color()
}

fn save_pixels_to_path(pixels: Vec<[u8; 4]>, width: u32, height: u32, path: &Path) {
    three_d_asset::io::save(
        &CpuTexture {
            data: TextureData::RgbaU8(pixels),
            width,
            height,
            ..Default::default()
        }
        .serialize(path)
        .unwrap(),
    )
    .unwrap();
}

fn render_parse_result_to_bytes(
    context: &HeadlessContext,
    viewport: &Viewport,
    texture: &mut Texture2D,
    depth_texture: &mut DepthTexture2D,
    parse_result: &parse_mesh::ParseResult,
    alpha: f32,
    file: &str,
    color: &str,
    rotatex: f32,
    rotatey: f32,
    scale: f32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError> {
    let mut models = build_models(context, parse_result, color);
    let pixels = render_pixels_for_view(
        &mut models,
        parse_result,
        file,
        viewport,
        texture,
        depth_texture,
        alpha,
        rotatex,
        rotatey,
        scale,
    );

    encode_pixels(pixels, texture.width(), texture.height(), format)
}

fn encode_pixels(
    pixels: Vec<[u8; 4]>,
    width: u32,
    height: u32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError> {
    let mut raw = Vec::with_capacity((width * height * 4) as usize);
    for pixel in pixels {
        raw.extend_from_slice(&pixel);
    }

    let image = RgbaImage::from_vec(width, height, raw)
        .ok_or_else(|| ThumbnailError::Other(String::from("Failed to create image buffer")))?;
    encode_dynamic_image(DynamicImage::ImageRgba8(image), format)
}

fn encode_dynamic_image(image: DynamicImage, format: &Format) -> Result<Vec<u8>, ThumbnailError> {
    let mut buffer = Vec::new();
    let output_format = match format {
        Format::Png => ImageFormat::Png,
        Format::Jpg => ImageFormat::Jpeg,
    };

    image
        .write_to(&mut Cursor::new(&mut buffer), output_format)
        .map_err(|e| ThumbnailError::Other(e.to_string()))?;

    Ok(buffer)
}

fn extract_image_from_3mf(
    threemf_path: &PathBuf,
    width: u32,
    height: u32,
    image_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let image = load_thumbnail_from_3mf(threemf_path)?;
    resize_dynamic_image(image, width, height).save(image_path)?;
    Ok(())
}

fn load_thumbnail_from_3mf(
    threemf_path: &PathBuf,
) -> Result<DynamicImage, Box<dyn std::error::Error>> {
    let file = File::open(threemf_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with("thumbnail_middle.png") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;

            return Ok(ImageReader::new(Cursor::new(buffer))
                .with_guessed_format()?
                .decode()?);
        }
    }

    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "thumbnail_middle.png not found in 3mf file",
    )))
}

fn extract_image_from_3mf_to_bytes(
    threemf_path: &PathBuf,
    width: u32,
    height: u32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError> {
    let image =
        load_thumbnail_from_3mf(threemf_path).map_err(|e| ThumbnailError::Other(e.to_string()))?;
    let resized = resize_dynamic_image(image, width, height);
    encode_dynamic_image(resized, format)
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
    gcode_path: &PathBuf,
    width: u32,
    height: u32,
    image_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(gcode_path)?;
    extract_image_from_gcode(&mut file, width, height, image_path)
}

fn extract_image_from_gcode_zip(
    gcode_zip_path: &PathBuf,
    width: u32,
    height: u32,
    image_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(gcode_zip_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".gcode") {
            return extract_image_from_gcode(&mut file, width, height, image_path);
        }
    }

    Err("No gcode file found in zip archive".into())
}

fn extract_image_from_gcode<W>(
    reader: &mut W,
    width: u32,
    height: u32,
    image_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: Read,
{
    let image = load_thumbnail_from_gcode_reader(reader)?;
    resize_dynamic_image(image, width, height).save(image_path)?;
    Ok(())
}

fn load_thumbnail_from_gcode_reader<W>(
    reader: &mut W,
) -> Result<DynamicImage, Box<dyn std::error::Error>>
where
    W: Read,
{
    let buffered_reader = BufReader::new(reader);
    let mut gcode_images: Vec<GcodeImage> = Vec::new();
    let mut in_gcode_section = false;
    let mut gcode_img_width = 0;
    let mut gcode_img_height = 0;
    let mut image = String::from("");

    for line in buffered_reader.lines().map_while(Result::ok) {
        if line.starts_with("; thumbnail begin") {
            let pixel_format = match line.split(' ').skip(3).next() {
                Some(s) => s,
                None => continue,
            };

            let pixel_format_unpacked: Vec<u32> = pixel_format
                .split('x')
                .map(|f| f.parse().unwrap_or_default())
                .collect();

            gcode_img_width = *pixel_format_unpacked.get(0).unwrap_or(&0);
            gcode_img_height = *pixel_format_unpacked.get(1).unwrap_or(&0);
            image = String::from("");

            in_gcode_section = gcode_img_width > 0 && gcode_img_height > 0;
        } else if line.starts_with("; thumbnail end") {
            in_gcode_section = false;
            let image = match BASE64_STANDARD.decode(&image) {
                Ok(data) => data,
                Err(e) => {
                    println!("Error decoding base64 image data: {}", e);
                    continue;
                }
            };

            gcode_images.push(GcodeImage {
                width: gcode_img_width,
                height: gcode_img_height,
                data: image,
            });
        } else if in_gcode_section {
            image.push_str(line[2..].trim());
        } else if line.starts_with("; EXECUTABLE_BLOCK_START") {
            break;
        }
    }

    gcode_images.sort_by(|a, b| b.area().cmp(&a.area()));

    let largest_image = match gcode_images.first() {
        Some(x) => x,
        None => return Err("No thumbnail found in gcode file".into()),
    };

    Ok(ImageReader::new(Cursor::new(&largest_image.data))
        .with_guessed_format()?
        .decode()?)
}

fn extract_image_from_gcode_reader_to_bytes<W>(
    reader: &mut W,
    width: u32,
    height: u32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError>
where
    W: Read,
{
    let image = load_thumbnail_from_gcode_reader(reader)
        .map_err(|e| ThumbnailError::Other(e.to_string()))?;
    let resized = resize_dynamic_image(image, width, height);
    encode_dynamic_image(resized, format)
}

fn extract_image_from_gcode_file_to_bytes(
    gcode_path: &PathBuf,
    width: u32,
    height: u32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError> {
    let mut file = File::open(gcode_path)?;
    extract_image_from_gcode_reader_to_bytes(&mut file, width, height, format)
}

fn extract_image_from_gcode_zip_to_bytes(
    gcode_zip_path: &PathBuf,
    width: u32,
    height: u32,
    format: &Format,
) -> Result<Vec<u8>, ThumbnailError> {
    let file = File::open(gcode_zip_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".gcode") {
            return extract_image_from_gcode_reader_to_bytes(&mut file, width, height, format);
        }
    }

    Err(ThumbnailError::Other(String::from(
        "No gcode file found in zip archive",
    )))
}

fn replace_file_stem(path: &mut PathBuf, new_stem: &str) {
    if let Some(ext) = path.extension() {
        path.set_file_name(format!("{}.{}", new_stem, ext.to_string_lossy()));
    } else {
        path.set_file_name(new_stem);
    }
}

fn resize_dynamic_image(image: DynamicImage, width: u32, height: u32) -> DynamicImage {
    image.resize_to_fill(width, height, Triangle)
}

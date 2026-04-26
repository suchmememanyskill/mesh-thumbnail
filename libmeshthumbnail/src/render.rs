use euc::{Buffer2d, IndexedVertices, Pipeline};
use image::RgbaImage;
use vek::{Mat4, Rgba, Vec2, Vec3, Vec4};

use crate::{mesh::{MeshAxisAlignedBoundingBox, ParseResult}, scene::Scene};

pub fn render(
    parse_result: &ParseResult,
    image_size: Vec2<usize>,
    rotation: Vec3<f32>,
    color: Vec3<u8>,
    zoom: f32,
) -> RgbaImage {
    let mut color_buffer = Buffer2d::fill([image_size.x, image_size.y], [0, 0, 0, 0]); // Transparent background
    let mut depth_buffer = Buffer2d::fill([image_size.x, image_size.y], 1.0);
    
    // Calculate combined bounding box for all meshes with their transforms
    let mut combined_min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut combined_max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    
    for mesh_with_transform in &parse_result.meshes {
        let aabb = mesh_with_transform.mesh.aabb();
        // Transform the AABB corners
        for corner in aabb_corners(&aabb) {
            let transformed = (mesh_with_transform.transform * Vec4::from_point(corner)).xyz();
            combined_min = Vec3::new(
                combined_min.x.min(transformed.x),
                combined_min.y.min(transformed.y),
                combined_min.z.min(transformed.z),
            );
            combined_max = Vec3::new(
                combined_max.x.max(transformed.x),
                combined_max.y.max(transformed.y),
                combined_max.z.max(transformed.z),
            );
        }
    }
    
    let center = (combined_min + combined_max) * 0.5;
    let size = combined_max - combined_min;
    let magnitude = size.magnitude();
    let scale = (2.4 / magnitude) * zoom;

    let camera_position = Vec3::new(-2.0, 0.0, 0.0);
    let view = Mat4::<f32>::look_at_lh(
        camera_position, 
        Vec3::new(0.0, 0.0, 0.0), 
        Vec3::new(0.0, 1.0, 0.0) 
    );
    
    let projection = Mat4::perspective_fov_lh_zo(
        1.0,
        image_size.x as f32,
        image_size.y as f32,
        0.1,
        100.0,
    );

    let default_color = Rgba::new(
        color.x as f32 / 255.0,
        color.y as f32 / 255.0,
        color.z as f32 / 255.0,
        1.0,
    );

    // Render each mesh with its own transform and color
    for mesh_with_transform in &parse_result.meshes {
        let model_matrix = Mat4::<f32>::rotation_x(270f32.to_radians()) *
                            Mat4::<f32>::rotation_z(90f32.to_radians()) *
                            Mat4::<f32>::rotation_x(rotation.y.to_radians() * -1.0) *
                            Mat4::<f32>::rotation_y(rotation.z.to_radians()) *
                            Mat4::<f32>::rotation_z(rotation.x.to_radians()) *
                            Mat4::<f32>::scaling_3d(Vec3::new(1.0, -1.0, 1.0)) *
                            Mat4::<f32>::scaling_3d(Vec3::broadcast(scale)) *
                            Mat4::<f32>::translation_3d(-center) *
                            mesh_with_transform.transform;

        let mvp = projection * view * model_matrix;

        let surface_color = mesh_with_transform.color
            .map(|c| Rgba::new(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, 1.0))
            .unwrap_or(default_color);

        let scene = Scene::new(
            mvp,
            model_matrix,
            camera_position,
            surface_color,
        );

        scene.render(
            IndexedVertices::new(
                mesh_with_transform.mesh.indices.iter().map(|&x| x as usize),
                mesh_with_transform.mesh.vertices.as_slice(),
            ),
            &mut color_buffer,
            &mut depth_buffer,
        );
    }

    let img = image::RgbaImage::from_fn(image_size.x as u32, image_size.y as u32, |x, y| {
        let pixel = color_buffer.raw()[y as usize * image_size.x + x as usize];
        image::Rgba(pixel)
    });

    img
}

fn aabb_corners(aabb: &MeshAxisAlignedBoundingBox) -> [Vec3<f32>; 8] {
    let min = aabb.min;
    let max = aabb.max;
    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ]
}
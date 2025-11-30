use euc::{Buffer2d, IndexedVertices, Pipeline};
use image::RgbaImage;
use vek::{Mat4, Rgba, Vec2, Vec3};

use crate::{mesh::Mesh, scene::Scene};

pub fn render(
    mesh: &Mesh,
    image_size: Vec2<usize>,
    rotation: Vec3<f32>,
    color: Vec3<u8>,
    zoom: f32,
) -> RgbaImage {
    let mut color_buffer = Buffer2d::fill([image_size.x, image_size.y], [0, 0, 0, 0]); // Transparent background
    let mut depth_buffer = Buffer2d::fill([image_size.x, image_size.y], 1.0);
    
    let aabb = mesh.aabb();
    let center = aabb.center();
    let magnitude = aabb.size().magnitude();
    let scale = (2.4 / magnitude) * zoom;

    // Set up camera and view matrices
    // First scale, then translate to origin
    let model =  Mat4::<f32>::rotation_x(270f32.to_radians()) *  // Y rotation - base value
                            Mat4::<f32>::rotation_z(90f32.to_radians()) * // X rotation - base value
                            Mat4::<f32>::rotation_x(rotation.y.to_radians() * -1.0) *
                            Mat4::<f32>::rotation_y(rotation.z.to_radians()) *
                            Mat4::<f32>::rotation_z(rotation.x.to_radians()) *
                            Mat4::<f32>::scaling_3d(Vec3::new(1.0, -1.0, 1.0)) *
                            Mat4::<f32>::scaling_3d(Vec3::broadcast(scale)) *
                            Mat4::<f32>::translation_3d(-center);
    

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
    
    let mvp = projection * view * model;

    let scene = Scene::new(
        mvp,
        model,
        camera_position,
        Rgba::new(
            color.x as f32 / 255.0,
            color.y as f32 / 255.0,
            color.z as f32 / 255.0,
            1.0,
        ),
    );
    

    scene.render(
        IndexedVertices::new(mesh.indices.iter().map(|&x| x as usize), &mesh.vertices.as_slice()),
        &mut color_buffer,
        &mut depth_buffer
    );

    let img = image::RgbaImage::from_fn(image_size.x as u32, image_size.y as u32, |x, y| {
        let pixel = color_buffer.raw()[y as usize * image_size.x + x as usize];
        image::Rgba(pixel)
    });

    img
}
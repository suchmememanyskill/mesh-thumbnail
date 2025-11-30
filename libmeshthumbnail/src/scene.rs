use euc::{CullMode, DepthMode, Pipeline, TriangleList};
use vek::{Mat4, Rgba, Vec3, Vec4};

// Vertex data that will be interpolated across the triangle
#[derive(Clone, Copy, Debug)]
pub struct VertexData {
    world_pos: Vec3<f32>,
    normal: Vec3<f32>, // Computed per-triangle, will be flat across the triangle
}

impl euc::math::WeightedSum for VertexData {
    fn weighted_sum<const N: usize>(values: [Self; N], weights: [f32; N]) -> Self {
        let mut world_pos = Vec3::zero();
        
        for i in 0..N {
            world_pos += values[i].world_pos * weights[i];
        }
        
        // Normal should be the same for all vertices in a triangle (flat shading)
        // Just take the first one
        VertexData { 
            world_pos,
            normal: values[0].normal,
        }
    }
}

// Rendering pipeline with derivative-based normal calculation and rim lighting
pub struct Scene {
    model_view_projection: Mat4<f32>,
    model: Mat4<f32>,
    camera_position: Vec3<f32>,
    surface_color: Rgba<f32>,
}

impl Scene {
    pub fn new(
        model_view_projection: Mat4<f32>,
        model: Mat4<f32>,
        camera_position: Vec3<f32>,
        surface_color: Rgba<f32>,
    ) -> Self {
        Scene {
            model_view_projection,
            model,
            camera_position,
            surface_color,
        }
    }
}

impl<'r> Pipeline<'r> for Scene {
    type Vertex = Vec3<f32>; // Just position
    type VertexData = VertexData;
    type Primitives = TriangleList;
    type Fragment = Rgba<f32>;
    type Pixel = [u8; 4];

    fn depth_mode(&self) -> DepthMode {
        DepthMode::LESS_WRITE
    }

    fn rasterizer_config(&self) -> CullMode {
        CullMode::Back
    }

    fn vertex(&self, vertex: &Self::Vertex) -> ([f32; 4], Self::VertexData) {
        let pos4 = Vec4::from_point(*vertex);
        let world_pos = (self.model * pos4).xyz();
        
        (
            (self.model_view_projection * pos4).into_array(),
            VertexData { 
                world_pos,
                normal: Vec3::zero(), // Will be computed in geometry shader
            },
        )
    }
    
    fn geometry<O>(
        &self,
        primitive: <Self::Primitives as euc::primitives::PrimitiveKind<Self::VertexData>>::Primitive,
        mut output: O,
    ) where
        O: FnMut(<Self::Primitives as euc::primitives::PrimitiveKind<Self::VertexData>>::Primitive),
    {
        // Compute the face normal from the triangle vertices in world space
        let [v0, v1, v2] = primitive;
        
        // Calculate edges
        let edge1 = v1.1.world_pos - v0.1.world_pos;
        let edge2 = v2.1.world_pos - v0.1.world_pos;
        
        // Compute normal via cross product
        let cross = edge1.cross(edge2);
        let normal = cross.normalized();
        
        // Create new vertices with the computed normal
        let v0_new = (v0.0, VertexData { world_pos: v0.1.world_pos, normal });
        let v1_new = (v1.0, VertexData { world_pos: v1.1.world_pos, normal });
        let v2_new = (v2.0, VertexData { world_pos: v2.1.world_pos, normal });
        
        output([v0_new, v1_new, v2_new]);
    }

    fn fragment(&self, data: Self::VertexData) -> Self::Fragment {
        // Normalize the interpolated normal (simulates the derivative-based normal computation)
        let normal = data.normal.try_normalized().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        
        // View direction calculation
        let view_dir = (self.camera_position - data.world_pos).try_normalized()
            .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        
        // Light direction follows camera (as in the original shader)
        let light_dir = view_dir;
        
        // Compute diffuse lighting - use absolute value to handle both front and back faces
        let diffuse = normal.dot(light_dir).abs().max(0.1); // Add ambient minimum
        
        // Soft rim light effect
        // rim = pow(1.0 - max(dot(viewDir, normal), 0.0), 3.0)
        let rim = (1.0 - view_dir.dot(normal).abs()).powf(3.0);
        
        // Merge colors
        let base_color = Vec3::new(self.surface_color.r, self.surface_color.g, self.surface_color.b);
        let shaded_color = base_color * diffuse + Vec3::broadcast(rim * 0.2);
        
        Rgba::new(
            shaded_color.x.clamp(0.0, 1.0),
            shaded_color.y.clamp(0.0, 1.0),
            shaded_color.z.clamp(0.0, 1.0),
            1.0
        )
    }

    fn blend(&self, _old: Self::Pixel, new: Self::Fragment) -> Self::Pixel {
        let rgba = new.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
        [rgba.r, rgba.g, rgba.b, rgba.a]
    }
}
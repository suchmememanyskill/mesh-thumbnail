use vek::Vec3;

pub struct Mesh {
    // Positions in space
    pub vertices : Vec<Vec3<f32>>,
    // Indices into the vertex array, 3 per triangle
    pub indices : Vec<u32>
}

impl Mesh {
    pub fn aabb(&self) -> MeshAxisAlignedBoundingBox {
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        
        for v in &self.vertices {
            min.x = min.x.min(v.x);
            min.y = min.y.min(v.y);
            min.z = min.z.min(v.z);
            
            max.x = max.x.max(v.x);
            max.y = max.y.max(v.y);
            max.z = max.z.max(v.z);
        }
        
        MeshAxisAlignedBoundingBox { min, max }
    }
}

pub struct MeshAxisAlignedBoundingBox {
    pub min: Vec3<f32>,
    pub max: Vec3<f32>,
}

impl MeshAxisAlignedBoundingBox {
    pub fn center(&self) -> Vec3<f32> {
        (self.min + self.max) * 0.5
    }
    
    pub fn size(&self) -> Vec3<f32> {
        self.max - self.min
    }
}


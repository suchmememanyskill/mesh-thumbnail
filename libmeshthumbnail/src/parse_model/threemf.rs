use std::{fs::File, path::PathBuf};

use vek::Vec3;

use crate::{error::MeshThumbnailError, mesh::Mesh};


pub fn handle_threemf(path : &PathBuf) -> Result<Option<Mesh>, MeshThumbnailError>
{
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".3mf") {
        Ok(Some(parse_3mf(path)?))
    } else {
        Ok(None)
    }
}

fn parse_3mf(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let handle = File::open(path)?;
    let mfmodel = threemf::read(handle)?;

    let mut all_meshes : Vec<&threemf::Mesh> = mfmodel
        .iter()
        .map(|f| f.resources.object.iter())
        .flat_map(|f| f)
        .filter(|predicate| predicate.mesh.is_some())
        .map(|f| f.mesh.as_ref().unwrap())
        .collect();

    all_meshes.sort_by(|a, b| a.triangles.triangle.len().cmp(&b.triangles.triangle.len()).reverse());

    if all_meshes.len() <= 0
    {
        return Err(MeshThumbnailError::InternalError(String::from("No meshes found in 3mf model")));
    }

    let mesh = all_meshes[0];

    let positions = mesh.vertices
        .vertex
            .iter()
            .map(|a| Vec3 {
                x: a.x as f32,
                y: a.y as f32,
                z: a.z as f32
            }).collect();

    let indices = mesh.triangles.triangle
        .iter()
        .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter())
        .collect();

    Ok(
        Mesh {
            vertices: positions,
            indices: indices
        }
    )
}
use std::{fs::File, io::{self, Cursor}, path::PathBuf};

use stl_io::IndexedMesh;
use vek::Vec3;
use zip::ZipArchive;

use crate::{error::MeshThumbnailError, mesh::Mesh};

pub fn handle_stl(path : &PathBuf) -> Result<Option<Mesh>, MeshThumbnailError>
{
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.ends_with(".stl.zip") {
        Ok(Some(parse_stl_zip(path)?))
    } else if path_str.ends_with(".stl") {
        Ok(Some(parse_stl(path)?))
    } else {
        Ok(None)
    }
}

fn parse_stl(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let mut handle = File::open(path)?;
    let stl = stl_io::read_stl(&mut handle)?;

    parse_stl_inner(&stl)
}

fn parse_stl_zip(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".stl") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            io::copy(&mut file, &mut buffer)?;
            let mut cursor = Cursor::new(buffer);

            let stl = stl_io::read_stl(&mut cursor)?;
            return parse_stl_inner(&stl);
        }
    }
    
    return Err(MeshThumbnailError::InternalError(String::from("Failed to find .stl model in zip")));
}

// https://github.com/asny/three-d-asset/blob/main/src/io/stl.rs#L9
fn parse_stl_inner(stl : &IndexedMesh) -> Result<Mesh, MeshThumbnailError>
{
    let vertices: Vec<Vec3<f32>> = stl.vertices
        .iter()
        .map(|v| Vec3::new(v[0], v[1], v[2]))
        .collect();
    
    let indices: Vec<u32> = stl.faces
        .iter()
        .flat_map(|face| face.vertices.map(|idx| idx as u32))
        .collect();

    Ok(
        Mesh {
            vertices: vertices,
            indices: indices,
        }
    )
}
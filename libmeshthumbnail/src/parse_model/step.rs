use std::{fs::File, io::{self, Cursor, Write}, path::PathBuf};

use opencascade::{mesh::Mesher, primitives::Shape};
use zip::ZipArchive;

use crate::{error::MeshThumbnailError, mesh::Mesh};

pub fn handle_step(path : &PathBuf) -> Result<Option<Mesh>, MeshThumbnailError>
{
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.ends_with(".step.zip") || path_str.ends_with(".stp.zip") {
        Ok(Some(parse_step_zip(path)?))
    } else if path_str.ends_with(".step") || path_str.ends_with(".stp") {
        Ok(Some(parse_step(path)?))
    } else {
        Ok(None)
    }
}

fn parse_step(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let shape = Shape::read_step(path)?;
    let mesher = Mesher::try_new(&shape, 0.01)?;
    let mesh = mesher.mesh()?;

    Ok(Mesh {
        vertices: mesh
            .vertices
            .into_iter()
            .map(|v| vek::Vec3::new(v.x as f32, v.y as f32, v.z as f32))
            .collect(),
        indices: mesh
            .indices
            .into_iter()
            .map(|i| i as u32)
            .collect(),
    })
}

fn parse_step_zip(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let mut temp_path = temp_dir.path().to_path_buf();
    temp_path.push("a.step");
    let mut temp_file = File::create(&temp_path)?;
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;
    let mut write_ok = false;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".step") || file.name().ends_with(".stp") {
            io::copy(&mut file, &mut temp_file)?;
            temp_file.flush()?;
            write_ok = true;
            break;
        }
    }

    drop(zip);
    drop(temp_file);

    if !write_ok {
        return Err(MeshThumbnailError::InternalError(String::from("Failed to find .step model in zip")));
    }
    
    parse_step(&temp_path)
}
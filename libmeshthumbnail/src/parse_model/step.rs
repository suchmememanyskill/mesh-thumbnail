use std::{env, fs::File, io::{self, Cursor, Write}, path::PathBuf};

use opencascade::{mesh::Mesher, primitives::Shape};
use stl_io::{Triangle, Vector, Vertex};
use zip::ZipArchive;

use crate::{error::MeshThumbnailError, mesh::Mesh};

const TOLERANCE_DEFAULT : f64 = 0.01;

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
    let tolerance = env::var("LIBMESHTHUMBNAIL_STEP_TRIANGULATION_TOLERANCE").map(|val| val.parse::<f64>().unwrap_or(TOLERANCE_DEFAULT)).unwrap_or(TOLERANCE_DEFAULT);
    let shape = Shape::read_step(path)?;
    let mesher = Mesher::try_new(&shape, tolerance)?;
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

pub fn convert_step_to_stl(step: &[u8]) -> Result<Vec<u8>, MeshThumbnailError> {
    // Todo: This is kinda hacky
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let mut temp_path = temp_dir.path().to_path_buf();
    temp_path.push("a.step");
    let mut temp_file = File::create(&temp_path)?;
    temp_file.write_all(step)?;
    temp_file.flush()?;
    drop(temp_file);

    convert_step_path_to_stl(&temp_path)
}

pub fn convert_step_path_to_stl(step_path : &PathBuf) -> Result<Vec<u8>, MeshThumbnailError> {
    let tolerance = env::var("LIBMESHTHUMBNAIL_STEP_TRIANGULATION_TOLERANCE").map(|val| val.parse::<f64>().unwrap_or(TOLERANCE_DEFAULT)).unwrap_or(TOLERANCE_DEFAULT);
    let shape = Shape::read_step(&step_path)?;
    let mesher = Mesher::try_new(&shape, tolerance)?;
    let mesh = mesher.mesh()?;

    let mut triangles: Vec<Triangle> = Vec::with_capacity(mesh.indices.len() / 3 + 1);

    for i in (0..mesh.indices.len()).step_by(3) {
        if i + 2 < mesh.indices.len() {
            let idx0 = mesh.indices[i];
            let idx1 = mesh.indices[i + 1];
            let idx2 = mesh.indices[i + 2];

            let v0 = &mesh.vertices[idx0];
            let v1 = &mesh.vertices[idx1];
            let v2 = &mesh.vertices[idx2];

            let n0 = &mesh.normals[idx0];
            let n1 = &mesh.normals[idx1];
            let n2 = &mesh.normals[idx2];
            let normal = Vector::new([
                ((n0.x + n1.x + n2.x) / 3.0) as f32,
                ((n0.y + n1.y + n2.y) / 3.0) as f32,
                ((n0.z + n1.z + n2.z) / 3.0) as f32,
            ]);

            let triangle = Triangle {
                normal,
                vertices: [
                    Vertex::new([v0.x as f32, v0.y as f32, v0.z as f32]),
                    Vertex::new([v1.x as f32, v1.y as f32, v1.z as f32]),
                    Vertex::new([v2.x as f32, v2.y as f32, v2.z as f32]),
                ],
            };

            triangles.push(triangle);
        }
    }

    let mut data = Vec::new();
    stl_io::write_stl(&mut data, triangles.iter())?;

    Ok(data)
}
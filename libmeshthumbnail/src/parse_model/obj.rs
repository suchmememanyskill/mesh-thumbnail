use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use vek::Vec3;
use wavefront_obj::obj::{self, ObjSet};
use zip::ZipArchive;

use crate::{error::MeshThumbnailError, mesh::Mesh};


pub fn handle_obj(path : &PathBuf) -> Result<Option<Mesh>, MeshThumbnailError>
{
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.ends_with(".obj.zip") {
        Ok(Some(parse_obj_zip(path)?))
    } else if path_str.ends_with(".obj") {
        Ok(Some(parse_obj(path)?))
    } else {
        Ok(None)
    }
}

fn parse_obj(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let mut handle = File::open(path)?;
    let mut buffer = Vec::new();
    handle.read_to_end(&mut buffer)?;

    let obj = obj::parse(std::str::from_utf8(&buffer).unwrap())?;
    parse_obj_inner(&obj)
}

fn parse_obj_zip(path : &PathBuf) -> Result<Mesh, MeshThumbnailError>
{
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".obj") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;

            return Ok(parse_obj_inner(&obj::parse(std::str::from_utf8(&buffer).unwrap())?)?);
        }
    }
    
    return Err(MeshThumbnailError::InternalError(String::from("Failed to find .obj model in zip")));
}

// https://github.com/asny/three-d-asset/blob/main/src/io/obj.rs#L54
fn parse_obj_inner(obj : &ObjSet) -> Result<Mesh, MeshThumbnailError>
{
    let mut all_meshes : Vec<Mesh> = obj.objects.iter().map(|object| {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
         for mesh in object.geometry.iter() { 
             let mut map: HashMap<usize, usize> = HashMap::new();
 
             let mut process = |i: wavefront_obj::obj::VTNIndex| {
                 let mut index = map.get(&i.0).map(|v| *v);
 
                 if index.is_none() {
                     index = Some(positions.len());
                     map.insert(i.0, index.unwrap());
                     let position = object.vertices[i.0];
                     positions.push(Vec3::new(position.x as f32, position.y as f32, position.z as f32));
                 }
 
                 indices.push(index.unwrap() as u32);
             };
             for shape in mesh.shapes.iter() {
                 // All triangles with same material
                 match shape.primitive {
                     wavefront_obj::obj::Primitive::Triangle(i0, i1, i2) => {
                         process(i0);
                         process(i1);
                         process(i2);
                     }
                     _ => {}
                 }
             }
         }

         Mesh {
            vertices: positions,
            indices: indices,
        }
     }).collect();

     all_meshes.sort_by(|a, b| a.indices.len().cmp(&b.indices.len()));

     if all_meshes.len() <= 0
     {
         return Err(MeshThumbnailError::InternalError(String::from("No meshes found in obj model")));
     }
 
     let mesh = all_meshes.pop().unwrap();

     return Ok(mesh);
}
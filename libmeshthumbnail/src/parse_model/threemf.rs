use std::{collections::HashMap, fs::File, io::Read, path::PathBuf};

use regex::Regex;
use vek::{Mat4, Rgba, Vec3};
use zip::ZipArchive;

use crate::{error::MeshThumbnailError, mesh::{Mesh, MeshWithTransform, ParseResult}};


pub fn handle_threemf(path : &PathBuf) -> Result<Option<ParseResult>, MeshThumbnailError>
{
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".3mf") {
        Ok(Some(parse_3mf(path)?))
    } else {
        Ok(None)
    }
}

fn parse_3mf(path : &PathBuf) -> Result<ParseResult, MeshThumbnailError>
{
    let handle = File::open(path)?;
    let mfmodel = threemf::read(handle)?;

    // Try to extract extruder colors from Slic3r config
    let extruder_colors = extract_extruder_colors_from_3mf(path);
    
    // Try to extract object/volume information from Slic3r model config
    let object_volumes = extract_object_volumes_from_3mf(path, &extruder_colors);

    // Build a map of object ID to mesh
    let mut object_map: HashMap<usize, &threemf::Mesh> = HashMap::new();
    
    for model in mfmodel.iter() {
        for object in model.resources.object.iter() {
            if let Some(mesh) = &object.mesh {
                object_map.insert(object.id, mesh);
            }
        }
    }

    if object_map.is_empty() {
        return Err(MeshThumbnailError::InternalError(String::from("No meshes found in 3mf model")));
    }

    let mut result_meshes: Vec<MeshWithTransform> = Vec::new();

    // Process build items (placed objects)
    for model in mfmodel.iter() {
        for item in model.build.item.iter() {
            if let Some(mesh) = object_map.get(&item.objectid) {
                // Get volume information for this object
                let volumes = object_volumes.get(&item.objectid);
                
                // Create transformation matrix from build item transform
                let transform = if let Some(t) = &item.transform {
                    Mat4::from_col_arrays([
                        [t[0] as f32, t[1] as f32, t[2] as f32, 0.0],
                        [t[3] as f32, t[4] as f32, t[5] as f32, 0.0],
                        [t[6] as f32, t[7] as f32, t[8] as f32, 0.0],
                        [t[9] as f32, t[10] as f32, t[11] as f32, 1.0],
                    ])
                } else {
                    Mat4::identity()
                };

                // If we have volume information, split into separate meshes by color
                if let Some(vol_list) = volumes {
                    for vol in vol_list {
                        let positions: Vec<Vec3<f32>> = mesh.vertices.vertex.iter().map(|a| Vec3 {
                            x: a.x as f32,
                            y: a.y as f32,
                            z: a.z as f32,
                        }).collect();

                        // Only include triangles in this volume's range
                        let start_tri = vol.first_triangle_id;
                        let end_tri = vol.last_triangle_id + 1;
                        
                        if end_tri <= mesh.triangles.triangle.len() {
                            let indices: Vec<u32> = mesh.triangles
                                .triangle
                                .iter()
                                .skip(start_tri)
                                .take(end_tri - start_tri)
                                .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter())
                                .collect();

                            result_meshes.push(MeshWithTransform {
                                mesh: Mesh { vertices: positions, indices },
                                transform,
                                color: vol.color,
                            });
                        }
                    }
                } else {
                    // No volume info, use entire mesh with single color
                    let positions: Vec<Vec3<f32>> = mesh.vertices.vertex.iter().map(|a| Vec3 {
                        x: a.x as f32,
                        y: a.y as f32,
                        z: a.z as f32,
                    }).collect();

                    let indices: Vec<u32> = mesh.triangles
                        .triangle
                        .iter()
                        .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter())
                        .collect();

                    result_meshes.push(MeshWithTransform {
                        mesh: Mesh { vertices: positions, indices },
                        transform,
                        color: None,
                    });
                }
            }
        }
    }

    // Fallback: if no build items found, return all meshes without transforms
    if result_meshes.is_empty() {
        for (_, mesh) in object_map.iter() {
            let positions: Vec<Vec3<f32>> = mesh.vertices.vertex.iter().map(|a| Vec3 {
                x: a.x as f32,
                y: a.y as f32,
                z: a.z as f32,
            }).collect();

            let indices: Vec<u32> = mesh.triangles
                .triangle
                .iter()
                .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter())
                .collect();

            result_meshes.push(MeshWithTransform {
                mesh: Mesh { vertices: positions, indices },
                transform: Mat4::identity(),
                color: None,
            });
        }
    }

    Ok(ParseResult::multiple(result_meshes))
}

// Extract extruder colors from Slic3r_PE.config in 3MF archive
fn extract_extruder_colors_from_3mf(path: &PathBuf) -> Vec<Rgba<u8>> {
    let mut colors = Vec::new();
    
    if let Ok(file) = File::open(path) {
        if let Ok(mut zip) = ZipArchive::new(file) {
            for i in 0..zip.len() {
                if let Ok(mut file) = zip.by_index(i) {
                    if file.name() == "Metadata/Slic3r_PE.config" {
                        let mut content = String::new();
                        if file.read_to_string(&mut content).is_ok() {
                            for line in content.lines() {
                                if line.starts_with("; extruder_colour =") {
                                    if let Some(colors_str) = line.split('=').nth(1) {
                                        let color_strs: Vec<&str> = colors_str.trim().split(';').collect();
                                        for color_str in color_strs {
                                            if let Some(color) = parse_hex_color_to_rgba(color_str.trim()) {
                                                colors.push(color);
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    
    colors
}

#[derive(Clone)]
struct VolumeInfo {
    first_triangle_id: usize,
    last_triangle_id: usize,
    color: Option<Rgba<u8>>,
}

// Extract object-to-volume mappings with colors from Slic3r_PE_model.config
fn extract_object_volumes_from_3mf(path: &PathBuf, extruder_colors: &[Rgba<u8>]) -> HashMap<usize, Vec<VolumeInfo>> {
    let mut object_volumes: HashMap<usize, Vec<VolumeInfo>> = HashMap::new();
    
    if let Ok(file) = File::open(path) {
        if let Ok(mut zip) = ZipArchive::new(file) {
            for i in 0..zip.len() {
                if let Ok(mut file) = zip.by_index(i) {
                    if file.name() == "Metadata/Slic3r_PE_model.config" {
                        let mut content = String::new();
                        if file.read_to_string(&mut content).is_ok() {
                            parse_slic3r_volumes(&content, extruder_colors, &mut object_volumes);
                        }
                        break;
                    }
                }
            }
        }
    }
    
    object_volumes
}

// Parse Slic3r_PE_model.config XML to extract volumes with their triangle ranges and colors
fn parse_slic3r_volumes(content: &str, extruder_colors: &[Rgba<u8>], object_volumes: &mut HashMap<usize, Vec<VolumeInfo>>) {
    let object_id_regex = Regex::new(r#"<object id="(\d+)""#).unwrap();
    let volume_regex = Regex::new(r#"<volume firstid="(\d+)" lastid="(\d+)">"#).unwrap();
    let object_extruder_regex = Regex::new(r#"<metadata type="object" key="extruder" value="(\d+)""#).unwrap();
    let volume_extruder_regex = Regex::new(r#"<metadata type="volume" key="extruder" value="(\d+)""#).unwrap();
    let color_regex = Regex::new(r#"<metadata type="volume" key="color" value="(#[0-9A-Fa-f]{6})""#).unwrap();
    
    let mut current_object_id: Option<usize> = None;
    let mut object_extruder: Option<usize> = None;
    let mut current_volumes: Vec<VolumeInfo> = Vec::new();
    let mut in_volume = false;
    let mut current_first_id: Option<usize> = None;
    let mut current_last_id: Option<usize> = None;
    let mut current_extruder: Option<usize> = None;
    let mut current_color: Option<Rgba<u8>> = None;
    
    for line in content.lines() {
        // Check for object ID
        if let Some(caps) = object_id_regex.captures(line) {
            // Save previous object's volumes
            if let Some(obj_id) = current_object_id {
                if !current_volumes.is_empty() {
                    object_volumes.insert(obj_id, current_volumes.clone());
                }
            }
            
            // Start new object
            current_object_id = caps.get(1).and_then(|m| m.as_str().parse().ok());
            object_extruder = None;
            current_volumes.clear();
            in_volume = false;
        }
        
        // Check for object-level extruder
        if !in_volume && line.contains(r#"type="object""#) && line.contains(r#"key="extruder""#) {
            if let Some(caps) = object_extruder_regex.captures(line) {
                object_extruder = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }
        
        // Check for volume start with triangle range
        if let Some(caps) = volume_regex.captures(line) {
            // Save previous volume if any
            if in_volume && current_first_id.is_some() && current_last_id.is_some() {
                let color = current_color.or_else(|| {
                    current_extruder.or(object_extruder).and_then(|ext| {
                        if ext > 0 && ext <= extruder_colors.len() {
                            Some(extruder_colors[ext - 1])
                        } else {
                            None
                        }
                    })
                });
                
                current_volumes.push(VolumeInfo {
                    first_triangle_id: current_first_id.unwrap(),
                    last_triangle_id: current_last_id.unwrap(),
                    color,
                });
            }
            
            // Start new volume
            in_volume = true;
            current_first_id = caps.get(1).and_then(|m| m.as_str().parse().ok());
            current_last_id = caps.get(2).and_then(|m| m.as_str().parse().ok());
            current_extruder = None;
            current_color = None;
        }
        
        // Check for extruder in current volume
        if in_volume && line.contains(r#"type="volume""#) && line.contains(r#"key="extruder""#) {
            if let Some(caps) = volume_extruder_regex.captures(line) {
                current_extruder = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }
        
        // Check for inline color in current volume
        if in_volume {
            if let Some(caps) = color_regex.captures(line) {
                if let Some(color_str) = caps.get(1) {
                    current_color = parse_hex_color_to_rgba(color_str.as_str());
                }
            }
        }
        
        // Check for volume end
        if line.contains("</volume>") && in_volume {
            if current_first_id.is_some() && current_last_id.is_some() {
                let color = current_color.or_else(|| {
                    current_extruder.or(object_extruder).and_then(|ext| {
                        if ext > 0 && ext <= extruder_colors.len() {
                            Some(extruder_colors[ext - 1])
                        } else {
                            None
                        }
                    })
                });
                
                current_volumes.push(VolumeInfo {
                    first_triangle_id: current_first_id.unwrap(),
                    last_triangle_id: current_last_id.unwrap(),
                    color,
                });
            }
            
            in_volume = false;
            current_first_id = None;
            current_last_id = None;
            current_extruder = None;
            current_color = None;
        }
    }
    
    // Don't forget the last object
    if let Some(obj_id) = current_object_id {
        if !current_volumes.is_empty() {
            object_volumes.insert(obj_id, current_volumes);
        }
    }
}

// Parse hex color string to Rgba<u8>
fn parse_hex_color_to_rgba(hex: &str) -> Option<Rgba<u8>> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    
    Some(Rgba::new(r, g, b, 255))
}
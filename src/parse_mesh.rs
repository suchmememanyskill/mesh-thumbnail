use regex::Regex;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::num::ParseFloatError;
use std::{collections::HashMap, fs::File};
use stl_io;
use stl_io::IndexedMesh;
use three_d::*;
use threemf;
use wavefront_obj::obj::{self, ObjSet};
use zip::ZipArchive;
use zip::result::ZipError;

#[derive(Clone)]
pub struct MeshWithTransform {
    pub mesh: CpuMesh,
    pub transform: Mat4,
    pub color: Option<Srgba>,
}

pub struct ParseResult {
    pub meshes: Vec<MeshWithTransform>,
}

impl ParseResult {
    pub fn single(mesh: CpuMesh) -> Self {
        ParseResult {
            meshes: vec![MeshWithTransform {
                mesh,
                transform: Mat4::identity(),
                color: None,
            }],
        }
    }

    pub fn multiple(meshes: Vec<MeshWithTransform>) -> Self {
        ParseResult { meshes }
    }
}

pub enum ParseError {
    ReadError(String),
    ParseError(String),
    MeshConvertError(String),
}

impl ToString for ParseError {
    fn to_string(&self) -> String {
        match self {
            ParseError::ReadError(str) => String::from(format!("Failed to read file: {}", str)),
            ParseError::ParseError(str) => {
                String::from(format!("Failed to interpret model: {}", str))
            }
            ParseError::MeshConvertError(str) => {
                String::from(format!("Failed to convert mesh from model: {}", str))
            }
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> ParseError {
        ParseError::ReadError(e.to_string())
    }
}

impl From<threemf::Error> for ParseError {
    fn from(e: threemf::Error) -> ParseError {
        ParseError::ParseError(e.to_string())
    }
}

impl From<ZipError> for ParseError {
    fn from(e: ZipError) -> ParseError {
        ParseError::ReadError(e.to_string())
    }
}

impl From<three_d_asset::Error> for ParseError {
    fn from(e: three_d_asset::Error) -> ParseError {
        ParseError::MeshConvertError(e.to_string())
    }
}

impl From<wavefront_obj::ParseError> for ParseError {
    fn from(e: wavefront_obj::ParseError) -> ParseError {
        ParseError::ParseError(e.to_string())
    }
}

impl From<ParseFloatError> for ParseError {
    fn from(e: ParseFloatError) -> ParseError {
        ParseError::ParseError(e.to_string())
    }
}

pub fn parse_file(path: &str) -> Result<ParseResult, ParseError> {
    if path.ends_with(".stl") {
        return Ok(ParseResult::single(parse_stl(path)?));
    } else if path.ends_with(".3mf") {
        return parse_3mf(path);
    } else if path.ends_with(".stl.zip") {
        return Ok(ParseResult::single(parse_stl_zip(path)?));
    } else if path.ends_with(".obj") {
        return Ok(ParseResult::single(parse_obj(path)?));
    } else if path.ends_with(".obj.zip") {
        return Ok(ParseResult::single(parse_obj_zip(path)?));
    } else if path.ends_with(".gcode") {
        return Ok(ParseResult::single(parse_gcode(path)?));
    } else if path.ends_with(".gcode.zip") {
        return Ok(ParseResult::single(parse_gcode_zip(path)?));
    }

    return Err(ParseError::ParseError(String::from("Unknown file type")));
}

fn parse_3mf(path: &str) -> Result<ParseResult, ParseError> {
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
        return Err(ParseError::MeshConvertError(String::from(
            "No meshes found in 3mf model",
        )));
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
                    Mat4::from_cols(
                        vec4(t[0] as f32, t[1] as f32, t[2] as f32, 0.0),
                        vec4(t[3] as f32, t[4] as f32, t[5] as f32, 0.0),
                        vec4(t[6] as f32, t[7] as f32, t[8] as f32, 0.0),
                        vec4(t[9] as f32, t[10] as f32, t[11] as f32, 1.0),
                    )
                } else {
                    Mat4::identity()
                };

                // If we have volume information, split into separate meshes by color
                if let Some(vol_list) = volumes {
                    for vol in vol_list {
                        let mut positions: Vec<Vec3> = Vec::new();
                        let mut indices: Vec<u32> = Vec::new();

                        // Collect all vertices (we'll need them all since indices reference them)
                        positions.extend(mesh.vertices.vertex.iter().map(|a| Vec3 {
                            x: a.x as f32,
                            y: a.y as f32,
                            z: a.z as f32,
                        }));

                        // Only include triangles in this volume's range
                        let start_tri = vol.first_triangle_id;
                        let end_tri = vol.last_triangle_id + 1; // +1 because lastid is inclusive

                        if end_tri <= mesh.triangles.triangle.len() {
                            indices.extend(
                                mesh.triangles
                                    .triangle
                                    .iter()
                                    .skip(start_tri)
                                    .take(end_tri - start_tri)
                                    .flat_map(|a| {
                                        [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter()
                                    }),
                            );

                            let cpu_mesh = CpuMesh {
                                positions: Positions::F32(positions),
                                indices: Indices::U32(indices),
                                ..Default::default()
                            };

                            result_meshes.push(MeshWithTransform {
                                mesh: cpu_mesh,
                                transform,
                                color: vol.color,
                            });
                        }
                    }
                } else {
                    // No volume info, use entire mesh with single color
                    let mut positions: Vec<Vec3> = Vec::new();
                    let mut indices: Vec<u32> = Vec::new();

                    positions.extend(mesh.vertices.vertex.iter().map(|a| Vec3 {
                        x: a.x as f32,
                        y: a.y as f32,
                        z: a.z as f32,
                    }));

                    indices.extend(
                        mesh.triangles
                            .triangle
                            .iter()
                            .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter()),
                    );

                    let cpu_mesh = CpuMesh {
                        positions: Positions::F32(positions),
                        indices: Indices::U32(indices),
                        ..Default::default()
                    };

                    result_meshes.push(MeshWithTransform {
                        mesh: cpu_mesh,
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
            let mut positions: Vec<Vec3> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();

            positions.extend(mesh.vertices.vertex.iter().map(|a| Vec3 {
                x: a.x as f32,
                y: a.y as f32,
                z: a.z as f32,
            }));

            indices.extend(
                mesh.triangles
                    .triangle
                    .iter()
                    .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter()),
            );

            result_meshes.push(MeshWithTransform {
                mesh: CpuMesh {
                    positions: Positions::F32(positions),
                    indices: Indices::U32(indices),
                    ..Default::default()
                },
                transform: Mat4::identity(),
                color: None,
            });
        }
    }

    Ok(ParseResult::multiple(result_meshes))
}

// Extract extruder colors from Slic3r_PE.config in 3MF archive
fn extract_extruder_colors_from_3mf(path: &str) -> Vec<Srgba> {
    let mut colors = Vec::new();

    if let Ok(file) = File::open(path) {
        if let Ok(mut zip) = ZipArchive::new(file) {
            for i in 0..zip.len() {
                if let Ok(mut file) = zip.by_index(i) {
                    if file.name() == "Metadata/Slic3r_PE.config" {
                        let mut content = String::new();
                        if file.read_to_string(&mut content).is_ok() {
                            // Parse extruder_colour line
                            for line in content.lines() {
                                if line.starts_with("; extruder_colour =") {
                                    if let Some(colors_str) = line.split('=').nth(1) {
                                        let color_strs: Vec<&str> =
                                            colors_str.trim().split(';').collect();
                                        for color_str in color_strs {
                                            if let Some(color) =
                                                parse_hex_color_to_srgba(color_str.trim())
                                            {
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
    color: Option<Srgba>,
}

// Extract object-to-volume mappings with colors from Slic3r_PE_model.config
fn extract_object_volumes_from_3mf(
    path: &str,
    extruder_colors: &[Srgba],
) -> HashMap<usize, Vec<VolumeInfo>> {
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
fn parse_slic3r_volumes(
    content: &str,
    extruder_colors: &[Srgba],
    object_volumes: &mut HashMap<usize, Vec<VolumeInfo>>,
) {
    let object_id_regex = Regex::new(r#"<object id="(\d+)""#).unwrap();
    let volume_regex = Regex::new(r#"<volume firstid="(\d+)" lastid="(\d+)">"#).unwrap();
    let object_extruder_regex =
        Regex::new(r#"<metadata type="object" key="extruder" value="(\d+)""#).unwrap();
    let volume_extruder_regex =
        Regex::new(r#"<metadata type="volume" key="extruder" value="(\d+)""#).unwrap();
    let color_regex =
        Regex::new(r#"<metadata type="volume" key="color" value="(#[0-9A-Fa-f]{6})""#).unwrap();

    let mut current_object_id: Option<usize> = None;
    let mut object_extruder: Option<usize> = None;
    let mut current_volumes: Vec<VolumeInfo> = Vec::new();
    let mut in_volume = false;
    let mut current_first_id: Option<usize> = None;
    let mut current_last_id: Option<usize> = None;
    let mut current_extruder: Option<usize> = None;
    let mut current_color: Option<Srgba> = None;

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
                    // Try volume extruder first, then fall back to object extruder
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
                    current_color = parse_hex_color_to_srgba(color_str.as_str());
                }
            }
        }

        // Check for volume end
        if line.contains("</volume>") && in_volume {
            // Save current volume
            if current_first_id.is_some() && current_last_id.is_some() {
                let color = current_color.or_else(|| {
                    // Try volume extruder first, then fall back to object extruder
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

// Parse hex color string to Srgba
fn parse_hex_color_to_srgba(hex: &str) -> Option<Srgba> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Srgba::new_opaque(r, g, b))
}

fn parse_stl(path: &str) -> Result<CpuMesh, ParseError> {
    let mut handle = File::open(path)?;
    let stl = stl_io::read_stl(&mut handle)?;

    parse_stl_inner(&stl)
}

fn parse_stl_zip(path: &str) -> Result<CpuMesh, ParseError> {
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".stl") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;
            let mut cursor = io::Cursor::new(buffer);

            let stl = stl_io::read_stl(&mut cursor)?;
            return parse_stl_inner(&stl);
        }
    }

    return Err(ParseError::MeshConvertError(String::from(
        "Failed to find .stl model in zip",
    )));
}

fn parse_obj(path: &str) -> Result<CpuMesh, ParseError> {
    let mut handle = File::open(path)?;
    let mut buffer = Vec::new();
    handle.read_to_end(&mut buffer)?;

    let obj = obj::parse(std::str::from_utf8(&buffer).unwrap())?;
    parse_obj_inner(&obj)
}

fn parse_obj_zip(path: &str) -> Result<CpuMesh, ParseError> {
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".obj") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;

            return Ok(parse_obj_inner(&obj::parse(
                std::str::from_utf8(&buffer).unwrap(),
            )?)?);
        }
    }

    return Err(ParseError::MeshConvertError(String::from(
        "Failed to find .obj model in zip",
    )));
}

// https://github.com/asny/three-d-asset/blob/main/src/io/stl.rs#L9
fn parse_stl_inner(stl: &IndexedMesh) -> Result<CpuMesh, ParseError> {
    let positions = stl
        .vertices
        .iter()
        .map(|vertex| Vec3 {
            x: vertex[0],
            y: vertex[1],
            z: vertex[2],
        })
        .collect();

    let indices = stl
        .faces
        .iter()
        .flat_map(|f| f.vertices.map(|a| a as u32))
        .collect();

    Ok(CpuMesh {
        positions: Positions::F32(positions),
        indices: Indices::U32(indices),
        ..Default::default()
    })
}

// https://github.com/asny/three-d-asset/blob/main/src/io/obj.rs#L54
fn parse_obj_inner(obj: &ObjSet) -> Result<CpuMesh, ParseError> {
    let mut all_meshes: Vec<CpuMesh> = obj
        .objects
        .iter()
        .map(|object| {
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
                        positions.push(Vector3::new(position.x, position.y, position.z));
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

            CpuMesh {
                positions: Positions::F64(positions),
                indices: Indices::U32(indices),
                ..Default::default()
            }
        })
        .collect();

    all_meshes.sort_by(|a, b| a.indices.len().cmp(&b.indices.len()).reverse());

    if all_meshes.len() <= 0 {
        return Err(ParseError::MeshConvertError(String::from(
            "No meshes found in 3mf model",
        )));
    }

    let mesh = &all_meshes[0];

    return Ok(CpuMesh {
        positions: mesh.positions.clone(),
        indices: mesh.indices.clone(),
        ..Default::default()
    });
}

struct Point {
    v: Vec3,
    use_line: bool,
}

fn parse_gcode(path: &str) -> Result<CpuMesh, ParseError> {
    let mut handle = File::open(path)?;

    parse_gcode_inner(&mut handle)
}

fn parse_gcode_zip(path: &str) -> Result<CpuMesh, ParseError> {
    let handle = File::open(path)?;
    let mut zip = ZipArchive::new(handle)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.name().ends_with(".gcode") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer)?;
            let mut cursor = io::Cursor::new(buffer);

            return parse_gcode_inner(&mut cursor);
        }
    }

    return Err(ParseError::MeshConvertError(String::from(
        "Failed to find .stl model in zip",
    )));
}
fn parse_gcode_inner<W>(reader: &mut W) -> Result<CpuMesh, ParseError>
where
    W: Read,
{
    let reader = io::BufReader::new(reader);
    let mut entries = Vec::with_capacity(0x10000);
    let mut last_x = 0f32;
    let mut last_y = 0f32;
    let mut last_z = 0f32;
    let regex_xy = Regex::new(r"X([\d.]+)\s+Y([\d.]+)\s+E").unwrap();
    let regex_xy_no_extrusion = Regex::new(r"X([\d.]+)\s+Y([\d.]+)").unwrap();
    let regex_z = Regex::new(r"Z([\d.]+)").unwrap();
    let mut position_unsafe = false;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("G1") || line.starts_with("G0") {
            if let Some(caps) = regex_z.captures(&line) {
                last_z = caps.get(1).unwrap().as_str().parse::<f32>()?;
            }

            if let Some(caps) = regex_xy.captures(&line) {
                if position_unsafe {
                    entries.push(Point {
                        v: vec3(-last_x, last_z, last_y),
                        use_line: false,
                    });
                    position_unsafe = false;
                }

                last_x = caps.get(1).unwrap().as_str().parse::<f32>()?;
                last_y = caps.get(2).unwrap().as_str().parse::<f32>()?;

                entries.push(Point {
                    v: vec3(-last_x, last_z, last_y),
                    use_line: true,
                });
            } else if let Some(caps) = regex_xy_no_extrusion.captures(&line) {
                last_x = caps.get(1).unwrap().as_str().parse::<f32>()?;
                last_y = caps.get(2).unwrap().as_str().parse::<f32>()?;
                position_unsafe = true;
            }
        }
    }

    if entries.len() <= 2 {
        return Err(ParseError::ParseError(String::from(
            "Gcode file contains no move instructions",
        )));
    }

    let angle_subdivisions = if entries.len() < 1000000 { 3 } else { 2 };
    let mut test_cylinder = CpuMesh::cylinder(angle_subdivisions);
    test_cylinder
        .transform(edge_transform(entries[0].v, entries[1].v))
        .unwrap();

    let estimated_entries = entries.iter().filter(|x| x.use_line).count();
    let mut positions = Vec::with_capacity(test_cylinder.positions.len() * estimated_entries);
    let mut indices = Vec::with_capacity(test_cylinder.indices.len().unwrap() * estimated_entries);

    for i in 0..entries.len() - 1 {
        if !entries[i + 1].use_line {
            continue;
        }

        let mut cylinder = CpuMesh::cylinder(angle_subdivisions);
        cylinder
            .transform(edge_transform(entries[i].v, entries[i + 1].v))
            .unwrap();

        let l = positions.len() as u32;

        positions.extend(cylinder.positions.into_f32());

        indices.extend(cylinder.indices.into_u32().unwrap().iter().map(|i| *i + l));
    }

    return Ok(CpuMesh {
        positions: Positions::F32(positions.clone()),
        indices: Indices::U32(indices.clone()),
        ..Default::default()
    });
}

// Smart code from https://github.com/asny/three-d/blob/master/examples/wireframe/src/main.rs
fn edge_transform(p1: Vec3, p2: Vec3) -> Mat4 {
    Mat4::from_translation(p1)
        * Into::<Mat4>::into(Quat::from_arc(
            vec3(1.0, 0.0, 0.0),
            (p2 - p1).normalize(),
            None,
        ))
        * Mat4::from_nonuniform_scale((p1 - p2).magnitude(), 0.2, 0.4)
}

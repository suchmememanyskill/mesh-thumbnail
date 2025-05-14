use regex::Regex;
use stl_io::IndexedMesh;
use three_d::*;
use threemf;
use std::num::ParseFloatError;
use std::{collections::HashMap, fs::File};
use std::io;
use std::io::Read;
use std::io::BufRead;
use stl_io;
use zip::ZipArchive;
use zip::result::ZipError;
use wavefront_obj::obj::{self, ObjSet};


pub enum ParseError
{
    ReadError(String),
    ParseError(String),
    MeshConvertError(String),
}

impl ToString for ParseError {
    fn to_string(&self) -> String {
        match self {
            ParseError::ReadError(str) => String::from(format!("Failed to read file: {}", str)),
            ParseError::ParseError(str) => String::from(format!("Failed to interpret model: {}", str)),
            ParseError::MeshConvertError(str) => String::from(format!("Failed to convert mesh from model: {}", str)),
        }
    }
}

impl From<io::Error> for ParseError 
{
    fn from(e: io::Error) -> ParseError 
    {
        ParseError::ReadError(e.to_string())
    }
}

impl From<threemf::Error> for ParseError
{
    fn from(e: threemf::Error) -> ParseError
    {
        ParseError::ParseError(e.to_string())
    }
}

impl From<ZipError> for ParseError
{
    fn from(e: ZipError) -> ParseError
    {
        ParseError::ReadError(e.to_string())
    }
}

impl From<three_d_asset::Error> for ParseError
{
    fn from(e: three_d_asset::Error) -> ParseError
    {
        ParseError::MeshConvertError(e.to_string())
    }
}

impl From<wavefront_obj::ParseError> for ParseError
{
    fn from(e: wavefront_obj::ParseError) -> ParseError
    {
        ParseError::ParseError(e.to_string())
    }
}

impl From<ParseFloatError> for ParseError
{
    fn from(e: ParseFloatError) -> ParseError
    {
        ParseError::ParseError(e.to_string())
    }
}

pub fn parse_file(path : &str) -> Result<CpuMesh, ParseError>
{
    if path.ends_with(".stl")
    {
        return parse_stl(path);
    }
    else if path.ends_with(".3mf")
    {
        return parse_3mf(path);   
    }
    else if path.ends_with(".stl.zip")
    {
        return parse_stl_zip(path);
    }
    else if path.ends_with(".obj")
    {
        return parse_obj(path);
    }
    else if path.ends_with(".obj.zip")
    {
        return parse_obj_zip(path);
    }
    else if path.ends_with(".gcode")
    {
        return parse_gcode(path);
    }
    else if path.ends_with(".gcode.zip")
    {
        return parse_gcode_zip(path);
    }

    return Err(ParseError::ParseError(String::from("Unknown file type")));
}

fn parse_3mf(path : &str) -> Result<CpuMesh, ParseError>
{
    /*
    let mut data = fs::read("model.3mf").unwrap();
    let mut file = io::Cursor::new(data);
     */
    let handle = File::open(path)?;
    let mfmodel = threemf::read(handle)?;

    let mut positions : Vec<Vec3> = Vec::new();
    let mut indices : Vec<u32> = Vec::new();
    
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
        return Err(ParseError::MeshConvertError(String::from("No meshes found in 3mf model")));
    }

    let mesh = all_meshes[0];

    positions.extend(mesh.vertices
        .vertex
            .iter()
            .map(|a| Vec3 {
                x: a.x as f32,
                y: a.y as f32,
                z: a.z as f32
            }));

    indices.extend(
        mesh.triangles.triangle
        .iter()
        .flat_map(|a| [a.v1 as u32, a.v2 as u32, a.v3 as u32].into_iter()));

    Ok(
        CpuMesh {
            positions: Positions::F32(positions),
            indices: Indices::U32(indices),
            ..Default::default()
        }
    )
}


fn parse_stl(path : &str) -> Result<CpuMesh, ParseError>
{
    let mut handle = File::open(path)?;
    let stl = stl_io::read_stl(&mut handle)?;

    parse_stl_inner(&stl)
}

fn parse_stl_zip(path : &str) -> Result<CpuMesh, ParseError>
{
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
    
    return Err(ParseError::MeshConvertError(String::from("Failed to find .stl model in zip")));
}

fn parse_obj(path : &str) -> Result<CpuMesh, ParseError>
{
    let mut handle = File::open(path)?;
    let mut buffer = Vec::new();
    handle.read_to_end(&mut buffer)?;

    let obj = obj::parse(std::str::from_utf8(&buffer).unwrap())?;
    parse_obj_inner(&obj)
}

fn parse_obj_zip(path : &str) -> Result<CpuMesh, ParseError>
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
    
    return Err(ParseError::MeshConvertError(String::from("Failed to find .obj model in zip")));
}

// https://github.com/asny/three-d-asset/blob/main/src/io/stl.rs#L9
fn parse_stl_inner(stl : &IndexedMesh) -> Result<CpuMesh, ParseError>
{
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

    Ok(
        CpuMesh {
            positions: Positions::F32(positions),
            indices: Indices::U32(indices),
            ..Default::default()
        }
    )
}

// https://github.com/asny/three-d-asset/blob/main/src/io/obj.rs#L54
fn parse_obj_inner(obj : &ObjSet) -> Result<CpuMesh, ParseError>
{
    let mut all_meshes : Vec<CpuMesh> = obj.objects.iter().map(|object| {
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
     }).collect();

     all_meshes.sort_by(|a, b| a.indices.len().cmp(&b.indices.len()).reverse());

     if all_meshes.len() <= 0
     {
         return Err(ParseError::MeshConvertError(String::from("No meshes found in 3mf model")));
     }
 
     let mesh = &all_meshes[0];

     return Ok(CpuMesh {
        positions: mesh.positions.clone(),
        indices: mesh.indices.clone(),
        ..Default::default()
     });
}

struct Point 
{
    v: Vec3,
    use_line: bool,
}

fn parse_gcode(path : &str) -> Result<CpuMesh, ParseError>
{
    let mut handle = File::open(path)?;

    parse_gcode_inner(&mut handle)
}

fn parse_gcode_zip(path : &str) -> Result<CpuMesh, ParseError>
{
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
    
    return Err(ParseError::MeshConvertError(String::from("Failed to find .stl model in zip")));
}
fn parse_gcode_inner<W>(reader: &mut W) -> Result<CpuMesh, ParseError>
where
    W: Read
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
            if let Some(caps) = regex_z.captures(&line)
            {
                last_z = caps.get(1).unwrap().as_str().parse::<f32>()?;
            }

            if let Some(caps) = regex_xy.captures(&line) 
            {
                if position_unsafe
                {
                    entries.push(Point { v: vec3(-last_x, last_z, last_y), use_line: false});
                    position_unsafe = false;
                }

                last_x = caps.get(1).unwrap().as_str().parse::<f32>()?;
                last_y = caps.get(2).unwrap().as_str().parse::<f32>()?;

                entries.push(Point { v: vec3(-last_x, last_z, last_y), use_line: true});
            }
            else if let Some(caps) = regex_xy_no_extrusion.captures(&line)
            {
                last_x = caps.get(1).unwrap().as_str().parse::<f32>()?;
                last_y = caps.get(2).unwrap().as_str().parse::<f32>()?;
                position_unsafe = true;
            }
        }
    }

    if entries.len() <= 2
    {
        return Err(ParseError::ParseError(String::from("Gcode file contains no move instructions")));
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
        if !entries[i + 1].use_line
        {
            continue;
        }

        let mut cylinder = CpuMesh::cylinder(angle_subdivisions);
        cylinder
            .transform(edge_transform(entries[i].v, entries[i + 1].v))
            .unwrap();

        let l = positions.len() as u32;

        positions.extend(
            cylinder.positions.into_f32()
        );

        indices.extend(
            cylinder.indices.into_u32()
                .unwrap()
                .iter()
                .map(|i| *i + l)
        );
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
        * Mat4::from_nonuniform_scale((p1 - p2).magnitude(), 0.2, 0.2)
}
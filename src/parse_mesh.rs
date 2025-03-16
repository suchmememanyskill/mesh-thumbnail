use stl_io::IndexedMesh;
use three_d::*;
use threemf;
use std::fs::File;
use std::io;
use std::io::Read;
use stl_io;
use zip::ZipArchive;
use zip::result::ZipError;

pub enum ParseError
{
    ReadError,
    ParseError,
    MeshConvertError,
}

impl ToString for ParseError {
    fn to_string(&self) -> String {
        match self {
            ParseError::ReadError => String::from("Failed to read file"),
            ParseError::ParseError => String::from("Failed to interpret model"),
            ParseError::MeshConvertError => String::from("Failed to convert mesh from model"),
        }
    }
}

impl From<io::Error> for ParseError 
{
    fn from(e: io::Error) -> ParseError 
    {
        ParseError::ReadError
    }
}

impl From<threemf::Error> for ParseError
{
    fn from(e: threemf::Error) -> ParseError
    {
        ParseError::ParseError
    }
}

impl From<ZipError> for ParseError
{
    fn from(e: ZipError) -> ParseError
    {
        ParseError::ReadError
    }
}

impl From<three_d_asset::Error> for ParseError
{
    fn from(e: three_d_asset::Error) -> ParseError
    {
        ParseError::MeshConvertError
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
    else 
    {
        let mut raw_assets = three_d_asset::io::load(&[path])?;
        return Ok(raw_assets.deserialize(path)?);
    }
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
    
    let f = mfmodel
        .iter()
        .map(|f| f.resources.object.iter())
        .flat_map(|f| f)
        .filter(|predicate| predicate.mesh.is_some())
        .map(|f| f.mesh.as_ref().unwrap())
        .next().unwrap();

    positions.extend(f.vertices
        .vertex
            .iter()
            .map(|a| Vec3 {
                x: a.x as f32,
                y: a.y as f32,
                z: a.z as f32
            }));

    indices.extend(
        f.triangles.triangle
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
    
    return Err(ParseError::MeshConvertError);
}

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
use stl_io::IndexedMesh;
use three_d::*;
use threemf;
use std::fs::File;
use std::io;
use std::io::Read;
use stl_io;
use zip::ZipArchive;

pub fn parse_file(path : &str) -> CpuMesh
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
        let mut raw_assets = three_d_asset::io::load(&[path]).unwrap();
        return raw_assets.deserialize(path).unwrap();
    }
}

fn parse_3mf(path : &str) -> CpuMesh
{
    /*
    let mut data = fs::read("model.3mf").unwrap();
    let mut file = io::Cursor::new(data);
     */
    let handle = File::open(path).unwrap();
    let mfmodel = threemf::read(handle).unwrap();

    let mut positions : Vec<Vec3> = Vec::new();
    let mut indices : Vec<u32> = Vec::new();
    
    mfmodel
        .iter()
        .map(|f| f.resources.object.iter())
        .flat_map(|f| f)
        .filter(|predicate| predicate.mesh.is_some())
        .map(|f| f.mesh.as_ref().unwrap())
        .for_each(|f| {
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
        });

    CpuMesh {
        positions: Positions::F32(positions),
        indices: Indices::U32(indices),
        ..Default::default()
    }
}


fn parse_stl(path : &str) -> CpuMesh
{
    let mut handle = File::open(path).unwrap();
    let stl = stl_io::read_stl(&mut handle).unwrap();

    parse_stl_inner(&stl)
}

fn parse_stl_zip(path : &str) -> CpuMesh
{
    let handle = File::open(path).unwrap();
    let mut zip = ZipArchive::new(handle).unwrap();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        if file.name().ends_with(".stl") {
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer).unwrap();
            let mut cursor = io::Cursor::new(buffer);

            let stl = stl_io::read_stl(&mut cursor).unwrap();
            return parse_stl_inner(&stl);
        }
    }
    
    panic!("No .stl models found in zip");
}

fn parse_stl_inner(stl : &IndexedMesh) -> CpuMesh
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

    CpuMesh {
        positions: Positions::F32(positions),
        indices: Indices::U32(indices),
        ..Default::default()
    }
}
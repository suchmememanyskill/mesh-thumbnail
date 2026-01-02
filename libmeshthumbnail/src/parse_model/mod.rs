use std::path::PathBuf;

mod stl;
mod obj;
mod threemf;
mod gcode;
#[cfg(feature = "step")]
mod step;

pub fn handle_parse(path : &PathBuf) -> Result<Option<crate::mesh::Mesh>, crate::error::MeshThumbnailError>
{
    if let Some(mesh) = stl::handle_stl(path)? {
        return Ok(Some(mesh));
    }

    if let Some(mesh) = obj::handle_obj(path)? {
        return Ok(Some(mesh));
    }

    if let Some(mesh) = threemf::handle_threemf(path)? {
        return Ok(Some(mesh));
    }

    if let Some(mesh) = gcode::handle_gcode(path)? {
        return Ok(Some(mesh));
    }

    #[cfg(feature = "step")]
    if let Some(mesh) = step::handle_step(path)? {
        return Ok(Some(mesh));
    }

    Ok(None)
}
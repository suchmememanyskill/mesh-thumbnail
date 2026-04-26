use std::path::PathBuf;

use crate::mesh::ParseResult;

mod stl;
mod obj;
mod threemf;
mod gcode;
#[cfg(feature = "step")]
mod step;
#[cfg(feature = "step")]
pub use step::convert_step_to_stl;
#[cfg(feature = "step")]
pub use step::convert_step_path_to_stl;

pub fn handle_parse(path : &PathBuf) -> Result<Option<ParseResult>, crate::error::MeshThumbnailError>
{
    if let Some(mesh) = stl::handle_stl(path)? {
        return Ok(Some(ParseResult::single(mesh)));
    }

    if let Some(mesh) = obj::handle_obj(path)? {
        return Ok(Some(ParseResult::single(mesh)));
    }

    if let Some(result) = threemf::handle_threemf(path)? {
        return Ok(Some(result));
    }

    if let Some(mesh) = gcode::handle_gcode(path)? {
        return Ok(Some(ParseResult::single(mesh)));
    }

    #[cfg(feature = "step")]
    if let Some(mesh) = step::handle_step(path)? {
        return Ok(Some(ParseResult::single(mesh)));
    }

    Ok(None)
}
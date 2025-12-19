#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]

use std::path::PathBuf;

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};
use pyo3::{create_exception, wrap_pyfunction};

use crate::{
    Format, ThumbnailError, ThumbnailOptions, generate_thumbnail_bytes_for_file,
    generate_thumbnail_for_file,
};

create_exception!(python, PyThumbnailError, PyException);

#[pyclass(name = "ThumbnailOptions")]
#[derive(Clone)]
pub struct PyThumbnailOptions {
    #[pyo3(get, set)]
    rotatex: f32,
    #[pyo3(get, set)]
    rotatey: f32,
    #[pyo3(get, set)]
    width: u32,
    #[pyo3(get, set)]
    height: u32,
    format: String,
    #[pyo3(get, set)]
    color: String,
    #[pyo3(get, set)]
    overwrite: bool,
    #[pyo3(get, set)]
    fallback_3mf_thumbnail: bool,
    #[pyo3(get, set)]
    prefer_3mf_thumbnail: bool,
    #[pyo3(get, set)]
    prefer_gcode_thumbnail: bool,
    #[pyo3(get, set)]
    inverse_zoom: f32,
}

#[pymethods]
impl PyThumbnailOptions {
    #[new]
    #[pyo3(signature = (rotatex=0.0, rotatey=0.0, width=512, height=512, format="png", color="DDDDDD", overwrite=false, fallback_3mf_thumbnail=false, prefer_3mf_thumbnail=false, prefer_gcode_thumbnail=false, inverse_zoom=1.0))]
    fn new(
        rotatex: f32,
        rotatey: f32,
        width: u32,
        height: u32,
        format: &str,
        color: &str,
        overwrite: bool,
        fallback_3mf_thumbnail: bool,
        prefer_3mf_thumbnail: bool,
        prefer_gcode_thumbnail: bool,
        inverse_zoom: f32,
    ) -> PyResult<Self> {
        Ok(Self {
            rotatex,
            rotatey,
            width,
            height,
            format: normalize_format_string(format)?,
            color: color.to_string(),
            overwrite,
            fallback_3mf_thumbnail,
            prefer_3mf_thumbnail,
            prefer_gcode_thumbnail,
            inverse_zoom,
        })
    }

    #[getter]
    fn format(&self) -> String {
        self.format.clone()
    }

    #[setter]
    fn set_format(&mut self, value: &str) -> PyResult<()> {
        self.format = normalize_format_string(value)?;
        Ok(())
    }

    fn copy(&self) -> Self {
        self.clone()
    }
}

fn normalize_format_string(value: &str) -> PyResult<String> {
    match value.to_ascii_lowercase().as_str() {
        "png" => Ok(String::from("png")),
        "jpg" | "jpeg" => Ok(String::from("jpg")),
        _ => Err(PyValueError::new_err("format must be 'png' or 'jpg'")),
    }
}

fn py_options_to_rust(options: Option<PyRef<'_, PyThumbnailOptions>>) -> PyResult<ThumbnailOptions> {
    let mut rust_options = ThumbnailOptions::default();

    if let Some(opts) = options {
        rust_options.rotatex = opts.rotatex;
        rust_options.rotatey = opts.rotatey;
        rust_options.width = opts.width;
        rust_options.height = opts.height;
        rust_options.color = opts.color.clone();
        rust_options.overwrite = opts.overwrite;
        rust_options.fallback_3mf_thumbnail = opts.fallback_3mf_thumbnail;
        rust_options.prefer_3mf_thumbnail = opts.prefer_3mf_thumbnail;
        rust_options.prefer_gcode_thumbnail = opts.prefer_gcode_thumbnail;
        rust_options.inverse_zoom = opts.inverse_zoom;
        rust_options.format = format_from_string(&opts.format)?;
    }

    rust_options.images_per_file = 1;
    Ok(rust_options)
}

fn format_from_string(value: &str) -> PyResult<Format> {
    match normalize_format_string(value)?.as_str() {
        "png" => Ok(Format::Png),
        "jpg" => Ok(Format::Jpg),
        _ => unreachable!(),
    }
}

fn thumbnail_error_to_pyerr(err: ThumbnailError) -> PyErr {
    PyThumbnailError::new_err(err.to_string())
}

#[pyfunction]
#[pyo3(name = "generate_thumbnail_for_file")]
#[pyo3(signature = (file, outdir, options=None))]
fn generate_thumbnail_for_file_py(
    file: &Bound<'_, PyAny>,
    outdir: &Bound<'_, PyAny>,
    options: Option<PyRef<'_, PyThumbnailOptions>>,
) -> PyResult<()> {
    let file_path: PathBuf = file.extract()?;
    let outdir_path: PathBuf = outdir.extract()?;
    let rust_options = py_options_to_rust(options)?;

    generate_thumbnail_for_file(&file_path, &outdir_path, &rust_options)
        .map_err(thumbnail_error_to_pyerr)
}

#[pyfunction]
#[pyo3(name = "generate_thumbnail_bytes")]
#[pyo3(signature = (file, options=None))]
fn generate_thumbnail_bytes_py<'py>(
    py: Python<'py>,
    file: &Bound<'py, PyAny>,
    options: Option<PyRef<'py, PyThumbnailOptions>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let file_path: PathBuf = file.extract()?;
    let rust_options = py_options_to_rust(options)?;

    let bytes = generate_thumbnail_bytes_for_file(&file_path, &rust_options)
        .map_err(thumbnail_error_to_pyerr)?;

    Ok(PyBytes::new_bound(py, &bytes))
}

#[pymodule]
pub fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyThumbnailOptions>()?;
    m.add_function(wrap_pyfunction!(generate_thumbnail_for_file_py, m)?)?;
    m.add_function(wrap_pyfunction!(generate_thumbnail_bytes_py, m)?)?;
    m.add("FORMAT_PNG", "png")?;
    m.add("FORMAT_JPG", "jpg")?;
    m.add(
        "MeshThumbnailError",
        m.py().get_type_bound::<PyThumbnailError>(),
    )?;
    Ok(())
}

use std::sync::Mutex;

use gdal::raster::GdalDataType;
use gdal::{Dataset, Metadata};
use rustler::{Atom, Binary, Env, NewBinary, ResourceArc};

mod atoms {
    rustler::atoms! {
        ok,
        error,
        unknown,
        uint8,
        int8,
        uint16,
        int16,
        uint32,
        int32,
        uint64,
        int64,
        float32,
        float64,
    }
}

struct DatasetResource {
    inner: Mutex<Dataset>,
}

#[rustler::resource_impl]
impl rustler::Resource for DatasetResource {}

fn gdal_err_to_string(e: gdal::errors::GdalError) -> String {
    format!("{e}")
}

// ---------------------------------------------------------------------------
// NIF: open
// ---------------------------------------------------------------------------
#[rustler::nif(schedule = "DirtyIo")]
fn gdal_open(path: String) -> Result<ResourceArc<DatasetResource>, String> {
    let ds = Dataset::open(&path).map_err(gdal_err_to_string)?;
    Ok(ResourceArc::new(DatasetResource {
        inner: Mutex::new(ds),
    }))
}

// ---------------------------------------------------------------------------
// NIF: raster_count
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_raster_count(resource: ResourceArc<DatasetResource>) -> Result<usize, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.raster_count())
}

// ---------------------------------------------------------------------------
// NIF: raster_size
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_raster_size(resource: ResourceArc<DatasetResource>) -> Result<(usize, usize), String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.raster_size())
}

// ---------------------------------------------------------------------------
// NIF: read_band (full band as raw u8 bytes)
// ---------------------------------------------------------------------------
#[rustler::nif(schedule = "DirtyIo")]
fn gdal_read_band(
    env: Env,
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
) -> Result<Binary, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;
    let band_type = band.band_type();

    // Read raw bytes regardless of data type
    let bytes = match band_type {
        GdalDataType::UInt8 => {
            let buf = band.read_band_as::<u8>().map_err(gdal_err_to_string)?;
            buf.data().to_vec()
        }
        GdalDataType::Int16 => {
            let buf = band.read_band_as::<i16>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        GdalDataType::UInt16 => {
            let buf = band.read_band_as::<u16>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        GdalDataType::Int32 => {
            let buf = band.read_band_as::<i32>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        GdalDataType::UInt32 => {
            let buf = band.read_band_as::<u32>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        GdalDataType::Float32 => {
            let buf = band.read_band_as::<f32>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        GdalDataType::Float64 => {
            let buf = band.read_band_as::<f64>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
        _ => {
            // Fallback: read as f64 and return raw bytes
            let buf = band.read_band_as::<f64>().map_err(gdal_err_to_string)?;
            buf.data()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>()
        }
    };

    let mut binary = NewBinary::new(env, bytes.len());
    binary.as_mut_slice().copy_from_slice(&bytes);
    Ok(binary.into())
}

// ---------------------------------------------------------------------------
// NIF: read_band_window (sub-region as raw u8 bytes)
// ---------------------------------------------------------------------------
#[rustler::nif(schedule = "DirtyIo")]
fn gdal_read_band_window(
    env: Env,
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
    x: isize,
    y: isize,
    w: usize,
    h: usize,
) -> Result<Binary, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;

    // Always read as u8 for windowed reads — caller can cast based on band_type
    let buf = band
        .read_as::<u8>((x, y), (w, h), (w, h), None)
        .map_err(gdal_err_to_string)?;
    let data = buf.data();

    let mut binary = NewBinary::new(env, data.len());
    binary.as_mut_slice().copy_from_slice(data);
    Ok(binary.into())
}

// ---------------------------------------------------------------------------
// NIF: band_type
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_band_type(resource: ResourceArc<DatasetResource>, band_idx: usize) -> Result<Atom, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;
    let dt = band.band_type();
    Ok(data_type_to_atom(dt))
}

fn data_type_to_atom(dt: GdalDataType) -> Atom {
    match dt {
        GdalDataType::UInt8 => atoms::uint8(),
        GdalDataType::UInt16 => atoms::uint16(),
        GdalDataType::Int16 => atoms::int16(),
        GdalDataType::UInt32 => atoms::uint32(),
        GdalDataType::Int32 => atoms::int32(),
        GdalDataType::Float32 => atoms::float32(),
        GdalDataType::Float64 => atoms::float64(),
        _ => atoms::unknown(),
    }
}

// ---------------------------------------------------------------------------
// NIF: no_data_value
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_no_data_value(
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
) -> Result<Option<f64>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;
    Ok(band.no_data_value())
}

// ---------------------------------------------------------------------------
// NIF: spatial_ref_wkt
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_spatial_ref_wkt(resource: ResourceArc<DatasetResource>) -> Result<String, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let srs = ds.spatial_ref().map_err(gdal_err_to_string)?;
    srs.to_wkt().map_err(gdal_err_to_string)
}

// ---------------------------------------------------------------------------
// NIF: spatial_ref_proj4
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_spatial_ref_proj4(resource: ResourceArc<DatasetResource>) -> Result<String, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let srs = ds.spatial_ref().map_err(gdal_err_to_string)?;
    srs.to_proj4().map_err(gdal_err_to_string)
}

// ---------------------------------------------------------------------------
// NIF: geo_transform
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_geo_transform(resource: ResourceArc<DatasetResource>) -> Result<Vec<f64>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let gt = ds.geo_transform().map_err(gdal_err_to_string)?;
    Ok(gt.to_vec())
}

// ---------------------------------------------------------------------------
// NIF: metadata_item
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_metadata_item(
    resource: ResourceArc<DatasetResource>,
    key: String,
    domain: String,
) -> Result<Option<String>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.metadata_item(&key, &domain))
}

// ---------------------------------------------------------------------------
// NIF: driver_name
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_driver_name(resource: ResourceArc<DatasetResource>) -> Result<String, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.driver().short_name())
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
rustler::init!("Elixir.ExGdal.Native");

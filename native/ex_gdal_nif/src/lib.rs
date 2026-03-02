use std::sync::Mutex;

use gdal::cpl::CslStringList;
use gdal::raster::GdalDataType;
use gdal::raster::processing::contour::contour_generate;
use gdal::vector::{LayerAccess, LayerOptions, OGRFieldType, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager, Metadata};
use rustler::{Atom, Binary, Env, NewBinary, NifMap, ResourceArc};

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
// NIF: metadata_domains — list all metadata domain names
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_metadata_domains(resource: ResourceArc<DatasetResource>) -> Result<Vec<String>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.metadata_domains())
}

// ---------------------------------------------------------------------------
// NIF: metadata_domain — all "Key=Value" entries for a domain
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_metadata_domain(
    resource: ResourceArc<DatasetResource>,
    domain: String,
) -> Result<Option<Vec<String>>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    Ok(ds.metadata_domain(&domain))
}

// ---------------------------------------------------------------------------
// NIF: band_description — the description string for a band (1-based index)
// ---------------------------------------------------------------------------
#[rustler::nif]
fn gdal_band_description(
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
) -> Result<String, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;
    band.description().map_err(gdal_err_to_string)
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
// NIF: contours
// ---------------------------------------------------------------------------
#[derive(NifMap)]
struct ContourFeature<'a> {
    id: i64,
    level: f64,
    level_min: Option<f64>,
    level_max: Option<f64>,
    wkb: Binary<'a>,
}

#[rustler::nif(schedule = "DirtyIo")]
fn gdal_contours<'a>(
    env: Env<'a>,
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
    interval: Option<f64>,
    base: f64,
    fixed_levels: Vec<f64>,
    polygonize: bool,
    nodata: Option<f64>,
) -> Result<Vec<ContourFeature<'a>>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;

    // 1. Create in-memory vector dataset + layer
    let mem_driver =
        DriverManager::get_driver_by_name("Memory").map_err(gdal_err_to_string)?;
    let mut mem_ds = mem_driver
        .create_vector_only("")
        .map_err(gdal_err_to_string)?;

    let geom_type = if polygonize {
        OGRwkbGeometryType::wkbMultiPolygon
    } else {
        OGRwkbGeometryType::wkbLineString
    };
    let mut layer = mem_ds
        .create_layer(LayerOptions {
            name: "contours",
            ty: geom_type,
            ..Default::default()
        })
        .map_err(gdal_err_to_string)?;

    // 2. Define fields
    if polygonize {
        layer
            .create_defn_fields(&[
                ("ID", OGRFieldType::OFTInteger),
                ("ELEV_MIN", OGRFieldType::OFTReal),
                ("ELEV_MAX", OGRFieldType::OFTReal),
            ])
            .map_err(gdal_err_to_string)?;
    } else {
        layer
            .create_defn_fields(&[
                ("ID", OGRFieldType::OFTInteger),
                ("ELEV", OGRFieldType::OFTReal),
            ])
            .map_err(gdal_err_to_string)?;
    }

    // 3. Build contour options
    let mut opts = CslStringList::new();
    if !fixed_levels.is_empty() {
        let s = fixed_levels
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(",");
        opts.add_string(&format!("FIXED_LEVELS={s}"))
            .map_err(|e| format!("{e}"))?;
    } else if let Some(iv) = interval {
        opts.add_string(&format!("LEVEL_INTERVAL={iv}"))
            .map_err(|e| format!("{e}"))?;
        opts.add_string(&format!("LEVEL_BASE={base}"))
            .map_err(|e| format!("{e}"))?;
    }
    if polygonize {
        opts.add_string("POLYGONIZE=YES")
            .map_err(|e| format!("{e}"))?;
        opts.add_string("ELEV_FIELD_MIN=ELEV_MIN")
            .map_err(|e| format!("{e}"))?;
        opts.add_string("ELEV_FIELD_MAX=ELEV_MAX")
            .map_err(|e| format!("{e}"))?;
    } else {
        opts.add_string("ELEV_FIELD=ELEV")
            .map_err(|e| format!("{e}"))?;
    }
    opts.add_string("ID_FIELD=ID")
        .map_err(|e| format!("{e}"))?;
    if let Some(nd) = nodata {
        opts.add_string(&format!("NODATA={nd}"))
            .map_err(|e| format!("{e}"))?;
    }

    // 4. Generate contours
    contour_generate(&band, &layer, &opts).map_err(gdal_err_to_string)?;

    // 5. Read features and extract WKB + field values
    let mut results = Vec::new();
    for feature in layer.features() {
        let geom = match feature.geometry() {
            Some(g) => g,
            None => continue,
        };
        let wkb_bytes = geom.wkb().map_err(gdal_err_to_string)?;

        let id_idx = feature.field_index("ID").map_err(gdal_err_to_string)?;
        let id = feature
            .field(id_idx)
            .ok()
            .flatten()
            .and_then(|v| v.into_int())
            .unwrap_or(0) as i64;

        let (level, level_min, level_max) = if polygonize {
            let min_idx = feature
                .field_index("ELEV_MIN")
                .map_err(gdal_err_to_string)?;
            let max_idx = feature
                .field_index("ELEV_MAX")
                .map_err(gdal_err_to_string)?;
            let min_val = feature
                .field(min_idx)
                .ok()
                .flatten()
                .and_then(|v| v.into_real());
            let max_val = feature
                .field(max_idx)
                .ok()
                .flatten()
                .and_then(|v| v.into_real());
            (min_val.unwrap_or(0.0), min_val, max_val)
        } else {
            let elev_idx = feature.field_index("ELEV").map_err(gdal_err_to_string)?;
            let elev = feature
                .field(elev_idx)
                .ok()
                .flatten()
                .and_then(|v| v.into_real())
                .unwrap_or(0.0);
            (elev, None, None)
        };

        let mut binary = NewBinary::new(env, wkb_bytes.len());
        binary.as_mut_slice().copy_from_slice(&wkb_bytes);

        results.push(ContourFeature {
            id,
            level,
            level_min,
            level_max,
            wkb: binary.into(),
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
rustler::init!("Elixir.ExGdal.Native");

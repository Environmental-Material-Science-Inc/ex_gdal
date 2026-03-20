# Contour Generation for ex_gdal

## Motivation

The stochastic plots feature in Liora needs to extract contour polygons from
concentration rasters (GeoTIFFs) at fixed threshold levels. The current plan
calls for a pure-Elixir marching squares implementation, but GDAL already ships
a production-grade contour generator (`GDALContourGenerate` /
`GDALContourGenerateEx`) that handles all the hard parts: marching squares,
segment stitching, hole detection, polygon assembly, and saddle-point
disambiguation. The `gdal-sys` crate already exposes these symbols — they just
need a safe Rust wrapper in the NIF.

Adding contour generation to ex_gdal replaces the entire `ContourExtractor`
module from the stochastic plan with a single NIF call per band, and eliminates
the biggest open question in that plan (marching squares correctness).

---

## Safety Strategy

Calling `gdal-sys` FFI functions directly from a Rustler NIF is dangerous: any
null-pointer dereference, use-after-free, or missed cleanup crashes the entire
BEAM, not just the calling process. The contour pipeline involves ~10 separate
OGR C API calls (driver lookup, datasource creation, layer creation, field
definition, feature iteration, geometry extraction, WKB export, feature
destruction, datasource destruction). Getting any one of those wrong is a
potential VM crash with no recovery.

The georust `gdal` crate (v0.19, already our dependency) provides safe Rust
wrappers for almost everything we need:

| Operation | Safe `gdal` crate API | Raw `gdal-sys` alternative |
|-----------|----------------------|---------------------------|
| Create in-memory dataset | `DriverManager::get_driver_by_name("Memory")` + `driver.create_vector_only("")` | `OGRGetDriverByName` + `OGR_Dr_CreateDataSource` (2 null checks) |
| Create layer | `dataset.create_layer(LayerOptions { ... })` | `OGR_DS_CreateLayer` (null check, ownership) |
| Add fields | `layer.create_defn_fields(&[...])` | `OGR_Fld_Create` + `OGR_L_CreateField` per field (null checks, cleanup) |
| Build options | `CslStringList::new()` + `.add_string()` | Manual CSL construction |
| Get band handle | `ds.rasterband(idx)` → `band.c_rasterband()` | `GDALGetRasterBand` (null check) |
| Get layer handle | `layer.c_layer()` | Already raw |
| Iterate features | `layer.features()` iterator | `OGR_L_ResetReading` + `OGR_L_GetNextFeature` loop (null check per feature, manual destroy) |
| Read geometry | `feature.geometry().wkb()` | `OGR_F_GetGeometryRef` + `OGR_G_WkbSize` + `OGR_G_ExportToWkb` (borrowed pointer — must NOT free, buffer sizing) |
| Cleanup | `Drop` impls on Dataset, Layer, Feature | Manual `OGR_F_Destroy` + `OGR_DS_Destroy` (order matters, double-free risk) |

The only function with no safe wrapper is `GDALContourGenerateEx` itself.

**Approach:** Add a thin safe wrapper for `GDALContourGenerateEx` to the
`gdal` crate at `~/work/gdal`, following the existing DEM processing pattern
(`src/raster/processing/dem/mod.rs`). Then the NIF uses exclusively safe Rust —
no raw pointer manipulation, no manual cleanup, no null checks. The Rust borrow
checker enforces that the band and layer outlive the contour call, and `Drop`
implementations handle all resource cleanup.

This confines the single `unsafe` block to the `gdal` crate, where it belongs
alongside the other ~200 unsafe FFI calls that crate already manages.

---

## Available GDAL C API

All symbols below exist in the `gdal-sys` prebuilt bindings for every supported
GDAL version (3.4–3.12):

### High-level (one-shot)

```c
// Classic API — interval-based or fixed-level contours, writes LineStrings
CPLErr GDALContourGenerate(
    GDALRasterBandH  hBand,
    double           dfContourInterval,
    double           dfContourBase,
    int              nFixedLevelCount,
    double          *padfFixedLevels,
    int              bUseNoData,
    double           dfNoDataValue,
    void            *hLayer,          // OGR layer to write features into
    int              iIDField,        // field index for contour ID (-1 to skip)
    int              iElevField,      // field index for elevation value (-1 to skip)
    GDALProgressFunc pfnProgress,
    void            *pProgressArg
);

// Extended API — option-string based, supports POLYGONIZE=YES
CPLErr GDALContourGenerateEx(
    GDALRasterBandH  hBand,
    void            *hLayer,
    CSLConstList     options,         // key=value pairs
    GDALProgressFunc pfnProgress,
    void            *pProgressArg
);
```

`GDALContourGenerateEx` options (passed as CSL string list):
- `LEVEL_INTERVAL=<double>` — contour interval
- `LEVEL_BASE=<double>` — base contour level
- `FIXED_LEVELS=<l1,l2,...>` — explicit list of contour levels
- `NODATA=<double>` — override nodata value
- `ID_FIELD=<name>` — field name for feature ID
- `ELEV_FIELD=<name>` — field name for elevation value
- `ELEV_FIELD_MIN=<name>` — field name for min elevation (polygonize mode)
- `ELEV_FIELD_MAX=<name>` — field name for max elevation (polygonize mode)
- `POLYGONIZE=YES|NO` — if YES, produce polygons instead of linestrings

### Low-level (scanline-based)

```c
GDALContourGeneratorH GDAL_CG_Create(
    int nWidth, int nHeight,
    int bNoDataSet, double dfNoDataValue,
    double dfContourInterval, double dfContourBase,
    GDALContourWriter pfnWriter, void *pCBData
);
CPLErr GDAL_CG_FeedLine(GDALContourGeneratorH hCG, double *padfScanline);
void   GDAL_CG_Destroy(GDALContourGeneratorH hCG);
```

We will use the high-level `GDALContourGenerateEx` API. The low-level API is
only useful if we wanted to stream scanlines without reading the full band,
which is unnecessary for our small grids (~14K pixels).

---

## Current ex_gdal Architecture

| Layer | Role |
|-------|------|
| `lib/ex_gdal.ex` | Public Elixir API, wraps NIF results in structs |
| `lib/ex_gdal/native.ex` | Rustler NIF stubs |
| `lib/ex_gdal/dataset.ex` | `%ExGdal.Dataset{}` struct (ref, path, raster_count, raster_size, driver) |
| `lib/ex_gdal/geo_transform.ex` | `%ExGdal.GeoTransform{}` struct |
| `native/ex_gdal_nif/src/lib.rs` | All Rust NIF implementations |

Patterns:
- Dataset is held as `ResourceArc<DatasetResource>` where `DatasetResource`
  wraps `Mutex<gdal::Dataset>`.
- I/O-bound NIFs use `#[rustler::nif(schedule = "DirtyIo")]`.
- Errors are mapped via `gdal_err_to_string` → `Result<T, String>`.
- Band data is returned as raw `Binary` (native-endian bytes).
- No vector/OGR support exists yet — this will be the first NIF that produces
  geometries.

---

## Design

### Output format: WKB binary list

The contour NIF produces OGR features in an in-memory layer. We need to get
those geometries back to Elixir. Options considered:

| Format | Pros | Cons |
|--------|------|------|
| WKT strings | Human-readable, easy to debug | Verbose, slow to parse, lossy for precision |
| GeoJSON strings | Frontend-friendly | Verbose, unnecessary overhead for DB path |
| **WKB binaries** | **Compact, lossless, PostGIS-native** | **Not human-readable** |

**Decision: WKB.** Liora inserts geometries into PostGIS, which natively
accepts WKB. The `geo` Elixir library can decode WKB via `Geo.WKB.decode!/1`
when needed. Returning a list of `{level, wkb_binary}` tuples keeps the NIF
output minimal and avoids serialization overhead.

### Rust NIF implementation

The NIF will:

1. Lock the `DatasetResource` mutex and get the raster band.
2. Create an in-memory OGR dataset (`Memory` driver) with one layer.
3. Add `ID` (integer), `ELEV` (real), `ELEV_MIN` (real), and `ELEV_MAX` (real)
   fields to the layer.
4. Build a CSL options string list from the Elixir-provided options.
5. Call `GDALContourGenerateEx` with the band handle and the in-memory layer.
6. Iterate the layer's features, extracting each geometry as WKB + elevation
   fields.
7. Return the results to Elixir as a list of maps.

### Elixir API

```elixir
@type contour_feature :: %{
  id: integer(),
  level: float(),
  level_min: float() | nil,
  level_max: float() | nil,
  wkb: binary()
}

@spec contours(Dataset.t(), pos_integer(), keyword()) ::
  {:ok, [contour_feature()]} | {:error, String.t()}
def contours(dataset, band_idx, opts \\ [])
```

Options:
- `:interval` — `float`, contour interval (mutually exclusive with `:levels`)
- `:base` — `float`, base contour level (default 0.0, used with `:interval`)
- `:levels` — `[float()]`, explicit list of contour levels
- `:polygonize` — `boolean`, produce polygons instead of linestrings (default `true`)
- `:nodata` — `float | nil`, override the band's nodata value

When `:polygonize` is `true`, each returned feature has `level_min` and
`level_max` populated. When `false`, only `level` is populated.

The default is `:polygonize => true` because the stochastic pipeline needs
filled regions (MultiPolygons where concentration >= threshold), not contour
lines.

### Usage in Liora

```elixir
{:ok, ds} = ExGdal.open(local_tif_path)
{:ok, nodata} = ExGdal.no_data_value(ds, 1)

thresholds = StochasticThresholds.thresholds_for("B")

{:ok, features} = ExGdal.contours(ds, band_idx,
  levels: thresholds,
  polygonize: true,
  nodata: nodata
)

# features is a list of %{level_min: 0.005, level_max: 0.05, wkb: <<...>>}
# Insert WKB directly into PostGIS with ST_Transform for reprojection:
#
#   INSERT INTO stochastic_output_geometries (geometry, ...)
#   VALUES (ST_Transform(ST_GeomFromWKB($1, $source_srid), 4326), ...)
```

This replaces the entire `ContourExtractor` module (marching squares, ring
assembly, hole classification) with a single NIF call, and the reprojection
question is handled by passing the source SRID to `ST_GeomFromWKB`.

---

## Implementation Plan

### Step 0: Safe contour wrapper in the `gdal` crate

**Repo:** `~/work/gdal`

Add a new module at `src/raster/processing/contour.rs` following the DEM
processing pattern. This is the only place `unsafe` code lives for contour
generation.

**New file:** `src/raster/processing/contour.rs`

```rust
use std::ptr;

use gdal_sys::CPLErr;

use crate::cpl::CslStringList;
use crate::errors::Result;
use crate::raster::RasterBand;
use crate::utils::_last_cpl_err;
use crate::vector::LayerAccess;

/// Generate contour lines or polygons from a raster band into an OGR layer.
///
/// Wraps [`GDALContourGenerateEx`](https://gdal.org/api/gdal_alg.html#_CPPv422GDALContourGenerateEx16GDALRasterBandHP12OGRLayerH12CSLConstList16GDALProgressFuncPv).
///
/// The caller is responsible for creating the output layer with appropriate
/// geometry type and fields. The `options` parameter controls contour
/// generation behavior via key=value pairs:
///
/// - `LEVEL_INTERVAL` — contour interval
/// - `LEVEL_BASE` — base contour level
/// - `FIXED_LEVELS` — comma-separated list of explicit contour levels
/// - `NODATA` — override nodata value
/// - `ID_FIELD` — output field name for feature ID
/// - `ELEV_FIELD` — output field name for elevation (linestring mode)
/// - `ELEV_FIELD_MIN` / `ELEV_FIELD_MAX` — output field names for min/max
///   elevation (polygon mode)
/// - `POLYGONIZE=YES` — produce polygons instead of linestrings
///
/// # Example
///
/// ```rust, no_run
/// # fn main() -> gdal::errors::Result<()> {
/// use gdal::Dataset;
/// use gdal::DriverManager;
/// use gdal::cpl::CslStringList;
/// use gdal::vector::LayerOptions;
/// use gdal::raster::processing::contour::contour_generate;
///
/// let ds = Dataset::open("input.tif")?;
/// let band = ds.rasterband(1)?;
///
/// let mem_driver = DriverManager::get_driver_by_name("Memory")?;
/// let mut mem_ds = mem_driver.create_vector_only("")?;
/// let layer = mem_ds.create_layer(LayerOptions {
///     name: "contours",
///     ty: gdal_sys::OGRwkbGeometryType::wkbMultiPolygon,
///     ..Default::default()
/// })?;
///
/// let mut opts = CslStringList::new();
/// opts.add_string("FIXED_LEVELS=0.005,0.05,0.5");
/// opts.add_string("POLYGONIZE=YES");
/// opts.add_string("ELEV_FIELD_MIN=ELEV_MIN");
/// opts.add_string("ELEV_FIELD_MAX=ELEV_MAX");
/// opts.add_string("ID_FIELD=ID");
///
/// contour_generate(&band, &layer, &opts)?;
///
/// for feature in layer.features() {
///     let wkb = feature.geometry().wkb()?;
///     // ... use WKB geometry
/// }
/// # Ok(())
/// # }
/// ```
pub fn contour_generate<L: LayerAccess>(
    band: &RasterBand,
    layer: &L,
    options: &CslStringList,
) -> Result<()> {
    let rv = unsafe {
        gdal_sys::GDALContourGenerateEx(
            band.c_rasterband(),
            layer.c_layer() as *mut std::ffi::c_void,
            options.as_ptr(),
            None,
            ptr::null_mut(),
        )
    };
    if rv != CPLErr::CE_None {
        return Err(_last_cpl_err(rv));
    }
    Ok(())
}
```

**Wire it up:**

- Add `mod contour;` and `pub use contour::*;` in
  `src/raster/processing/mod.rs`.
- The function is generic over `L: LayerAccess`, so it works with both `Layer`
  (borrowed from a dataset) and `OwnedLayer`.
- `RasterBand::c_rasterband()` and `LayerAccess::c_layer()` are both already
  `pub unsafe` — the single `unsafe` block here is the only place they're
  called for contour generation.
- The borrow checker enforces that `band` and `layer` (and their parent
  datasets) remain alive for the duration of the call.
- No tests in the `gdal` crate itself — the function is a thin FFI wrapper
  with no logic. We test the full pipeline from ex_gdal's test suite.

**Cargo.toml change:** None. The `gdal` crate already depends on `gdal-sys`.

**Dependency change in ex_gdal:** Switch from the crates.io `gdal` dependency
to a path dependency pointing at `~/work/gdal` until we upstream the change:

```toml
# native/ex_gdal_nif/Cargo.toml
[dependencies]
rustler = "0.36"
gdal = { path = "../../../gdal" }
```

### Step 1: Rust NIF — `gdal_contours` (safe Rust only)

**File:** `native/ex_gdal_nif/src/lib.rs`

The NIF uses only safe `gdal` crate types. No `gdal-sys` imports, no raw
pointers, no manual cleanup.

```rust
use gdal::DriverManager;
use gdal::cpl::CslStringList;
use gdal::vector::{LayerAccess, LayerOptions, FieldDefn};
use gdal::raster::processing::contour::contour_generate;

#[derive(NifMap)]
struct ContourFeature {
    id: i64,
    level: f64,
    level_min: Option<f64>,
    level_max: Option<f64>,
    wkb: Binary,
}

#[rustler::nif(schedule = "DirtyIo")]
fn gdal_contours(
    env: Env,
    resource: ResourceArc<DatasetResource>,
    band_idx: usize,
    interval: Option<f64>,
    base: f64,
    fixed_levels: Vec<f64>,
    polygonize: bool,
    nodata: Option<f64>,
) -> Result<Vec<ContourFeature>, String> {
    let ds = resource.inner.lock().map_err(|e| format!("{e}"))?;
    let band = ds.rasterband(band_idx).map_err(gdal_err_to_string)?;

    // 1. Create in-memory vector dataset + layer (safe, Drop-managed)
    let mem_driver = DriverManager::get_driver_by_name("Memory")
        .map_err(gdal_err_to_string)?;
    let mut mem_ds = mem_driver.create_vector_only("")
        .map_err(gdal_err_to_string)?;

    let geom_type = if polygonize {
        OGRwkbGeometryType::wkbMultiPolygon
    } else {
        OGRwkbGeometryType::wkbLineString
    };
    let layer = mem_ds.create_layer(LayerOptions {
        name: "contours",
        ty: geom_type,
        ..Default::default()
    }).map_err(gdal_err_to_string)?;

    // 2. Define fields (safe FieldDefn API)
    let id_field = FieldDefn::new("ID", OGRFieldType::OFTInteger)
        .map_err(gdal_err_to_string)?;
    id_field.add_to_layer(&layer).map_err(gdal_err_to_string)?;

    if polygonize {
        for name in ["ELEV_MIN", "ELEV_MAX"] {
            let field = FieldDefn::new(name, OGRFieldType::OFTReal)
                .map_err(gdal_err_to_string)?;
            field.add_to_layer(&layer).map_err(gdal_err_to_string)?;
        }
    } else {
        let field = FieldDefn::new("ELEV", OGRFieldType::OFTReal)
            .map_err(gdal_err_to_string)?;
        field.add_to_layer(&layer).map_err(gdal_err_to_string)?;
    }

    // 3. Build contour options (safe CslStringList)
    let mut opts = CslStringList::new();
    if !fixed_levels.is_empty() {
        let s = fixed_levels.iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(",");
        opts.add_string(&format!("FIXED_LEVELS={s}"));
    } else if let Some(iv) = interval {
        opts.add_string(&format!("LEVEL_INTERVAL={iv}"));
        opts.add_string(&format!("LEVEL_BASE={base}"));
    }
    if polygonize {
        opts.add_string("POLYGONIZE=YES");
        opts.add_string("ELEV_FIELD_MIN=ELEV_MIN");
        opts.add_string("ELEV_FIELD_MAX=ELEV_MAX");
    } else {
        opts.add_string("ELEV_FIELD=ELEV");
    }
    opts.add_string("ID_FIELD=ID");
    if let Some(nd) = nodata {
        opts.add_string(&format!("NODATA={nd}"));
    }

    // 4. Generate contours (single safe call wrapping the FFI)
    contour_generate(&band, &layer, &opts)
        .map_err(gdal_err_to_string)?;

    // 5. Read features (safe iterator, safe WKB export)
    let mut results = Vec::new();
    for feature in layer.features() {
        let geom = feature.geometry();
        let wkb_bytes = geom.wkb().map_err(gdal_err_to_string)?;

        let id = feature.field("ID")
            .map_err(gdal_err_to_string)?
            .and_then(|v| v.into_int())
            .unwrap_or(0) as i64;

        let (level, level_min, level_max) = if polygonize {
            let min = feature.field("ELEV_MIN")
                .ok().flatten().and_then(|v| v.into_real());
            let max = feature.field("ELEV_MAX")
                .ok().flatten().and_then(|v| v.into_real());
            (min.unwrap_or(0.0), min, max)
        } else {
            let elev = feature.field("ELEV")
                .ok().flatten().and_then(|v| v.into_real())
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

    // mem_ds, layer, band all cleaned up by Drop — no manual cleanup

    Ok(results)
}
```

**What's different from the previous version:**
- Zero `gdal-sys` imports in the NIF
- Zero raw pointers
- Zero manual `OGR_F_Destroy` / `OGR_DS_Destroy` calls
- Zero null checks (the safe API returns `Result`)
- If any step fails, Rust unwinds and `Drop` cleans up all resources
- The borrow checker proves at compile time that `band` and `layer` outlive
  the `contour_generate` call

### Step 2: Elixir NIF stub

**File:** `lib/ex_gdal/native.ex`

```elixir
def gdal_contours(_resource, _band_idx, _interval, _base, _fixed_levels, _polygonize, _nodata),
  do: :erlang.nif_error(:nif_not_loaded)
```

### Step 3: Elixir public API

**File:** `lib/ex_gdal.ex`

```elixir
@doc """
Extracts contour geometries from a raster band.

Returns a list of contour features, each containing a WKB geometry binary
and elevation metadata. Use either `:interval` for regularly-spaced contours
or `:levels` for specific threshold values.

## Options

  * `:interval` - contour interval (mutually exclusive with `:levels`)
  * `:base` - base contour level (default `0.0`, used with `:interval`)
  * `:levels` - explicit list of contour levels as floats
  * `:polygonize` - if `true`, produce filled polygons instead of linestrings
    (default `true`)
  * `:nodata` - override the band's nodata value

## Examples

    # Fixed threshold levels (stochastic pipeline use case)
    {:ok, features} = ExGdal.contours(ds, 1,
      levels: [0.005, 0.05, 0.5],
      polygonize: true
    )

    # Regular interval contours
    {:ok, features} = ExGdal.contours(ds, 1, interval: 10.0, base: 0.0)

Each feature is a map with keys:

  * `:id` - integer feature ID
  * `:level` - contour elevation (linestring mode)
  * `:level_min` - lower bound of contour band (polygon mode)
  * `:level_max` - upper bound of contour band (polygon mode)
  * `:wkb` - geometry as WKB binary
"""
@spec contours(Dataset.t(), pos_integer(), keyword()) ::
  {:ok, [map()]} | {:error, String.t()}
def contours(%Dataset{ref: ref}, band_idx, opts \\ []) do
  interval = Keyword.get(opts, :interval)
  base = Keyword.get(opts, :base, 0.0)
  levels = Keyword.get(opts, :levels, [])
  polygonize = Keyword.get(opts, :polygonize, true)
  nodata = Keyword.get(opts, :nodata)

  Native.gdal_contours(ref, band_idx, interval, base, levels, polygonize, nodata)
end
```

### Step 4: Tests

**File:** `test/ex_gdal_test.exs`

Add tests using the existing `dem-hills.tiff` fixture:

```elixir
describe "contours/3" do
  test "extracts contour linestrings at regular interval" do
    {:ok, ds} = ExGdal.open("test/fixtures/dem-hills.tiff")
    {:ok, features} = ExGdal.contours(ds, 1,
      interval: 50.0,
      polygonize: false
    )

    assert is_list(features)
    assert length(features) > 0

    feat = hd(features)
    assert is_integer(feat.id)
    assert is_float(feat.level)
    assert is_binary(feat.wkb)
    assert byte_size(feat.wkb) > 0
  end

  test "extracts contour polygons at fixed levels" do
    {:ok, ds} = ExGdal.open("test/fixtures/dem-hills.tiff")
    {:ok, features} = ExGdal.contours(ds, 1,
      levels: [100.0, 200.0, 300.0],
      polygonize: true
    )

    assert is_list(features)
    assert length(features) > 0

    feat = hd(features)
    assert is_float(feat.level_min)
    assert is_float(feat.level_max)
    assert is_binary(feat.wkb)
  end

  test "returns empty list for levels above raster range" do
    {:ok, ds} = ExGdal.open("test/fixtures/dem-hills.tiff")
    {:ok, features} = ExGdal.contours(ds, 1,
      levels: [99999.0],
      polygonize: true
    )

    assert features == []
  end

  test "respects nodata override" do
    {:ok, ds} = ExGdal.open("test/fixtures/dem-hills.tiff")
    {:ok, features} = ExGdal.contours(ds, 1,
      levels: [100.0],
      polygonize: true,
      nodata: -9999.0
    )

    assert is_list(features)
  end

  test "returns error for invalid band index" do
    {:ok, ds} = ExGdal.open("test/fixtures/dem-hills.tiff")
    assert {:error, _} = ExGdal.contours(ds, 999, levels: [100.0])
  end
end
```

### Step 5: Documentation

Update the README to document the new `contours/3` function and add a usage
example showing the stochastic pipeline use case.

---

## Considerations

### Thread safety

`GDALContourGenerateEx` reads from the band and writes to the OGR layer.
Both the input dataset (locked via `Mutex`) and the output layer (created
per-call, not shared) are safe to use from a dirty scheduler thread. The
mutex is held for the duration of the NIF call, same as `gdal_read_band`.

### Performance

For a 120x116 grid with 3 threshold levels, `GDALContourGenerateEx` will
complete in microseconds. The dirty I/O scheduler is appropriate because the
band data may need to be read from disk (or S3 via GDAL's /vsicurl), but the
contour computation itself is negligible.

When processing ~100 realizations x ~210 bands, the NIF is called ~21,000
times. Each call is independent and fast. The bottleneck remains S3 download,
not contour extraction.

### GDAL version compatibility

`GDALContourGenerateEx` with `POLYGONIZE=YES` is available in GDAL >= 2.4.
The `gdal-sys` prebuilt bindings include it for all supported versions
(3.4–3.12). No feature-gating needed.

### Coordinate reference system

The contour geometries inherit the raster's CRS (typically UTM). The NIF
returns raw WKB without reprojection. Liora handles reprojection at insert
time via PostGIS:

```sql
ST_Transform(ST_GeomFromWKB($1, $source_srid), 4326)
```

The source SRID can be determined from `ExGdal.spatial_ref_wkt/1` or
`spatial_ref_proj4/1` and mapped to an EPSG code. This keeps the NIF simple
and avoids adding OGR coordinate transformation to the NIF boundary.

### Geometry validity

GDAL's contour generator can occasionally produce geometries that fail
PostGIS validation (self-intersections at pixel corners). Wrapping the insert
with `ST_MakeValid()` is cheap insurance:

```sql
ST_Transform(ST_MakeValid(ST_GeomFromWKB($1, $source_srid)), 4326)
```

---

## Impact on Stochastic Plots Plan

This feature eliminates the following from the stochastic plan:

- **`Liora.Simulations.ContourExtractor`** module — no longer needed
- **Open question 3** (marching squares implementation) — resolved by GDAL
- **Open question 4** (UTM→WGS84 reprojection) — resolved by PostGIS
  `ST_Transform` at insert time, with source SRID from `ex_gdal`
- **Ring classification / hole handling** — handled by GDAL's `POLYGONIZE=YES`

The ingestion worker simplifies to:

```elixir
{:ok, ds} = ExGdal.open(tmp_path)
{:ok, nodata} = ExGdal.no_data_value(ds, 1)

for band_idx <- 1..ds.raster_count do
  {:ok, features} = ExGdal.contours(ds, band_idx,
    levels: thresholds,
    polygonize: true,
    nodata: nodata
  )

  insert_stochastic_geometries(features, metadata)
end
```

No raw band reading, no binary decoding, no Elixir-side grid processing.

---

## Sequence of Work

0. **`gdal` crate** — add `contour_generate` in `src/raster/processing/contour.rs`, wire into `mod.rs` (~30m)
1. **Cargo.toml** — point ex_gdal at local `gdal` path dependency (~5m)
2. **Rust NIF** — implement `gdal_contours` in `lib.rs` using safe `gdal` API (~1-2h)
3. **Elixir API** — add `Native.gdal_contours` stub + `ExGdal.contours/3` (~30m)
4. **Tests** — add contour tests using `dem-hills.tiff` (~30m)
5. **Verify** — run `mix test`, confirm WKB output is valid via `Geo.WKB.decode!/1` (~15m)
6. **Documentation** — update README (~15m)

Estimated total: **half a day**.

After verifying the feature works end-to-end, upstream the `contour_generate`
wrapper to the georust/gdal repository and switch the Cargo.toml back to a
crates.io version dependency once it's released.

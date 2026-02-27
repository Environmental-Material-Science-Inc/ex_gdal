# ExGdal

Elixir bindings for reading raster geospatial data (GeoTIFF, etc.) via GDAL.

The NIF layer is written in Rust using [Rustler](https://github.com/rusterlium/rustler) and wraps the [gdal](https://github.com/georust/gdal) Rust crate. Dataset handles are held in `ResourceArc<Mutex<Dataset>>` so they are managed by the BEAM garbage collector and safe to pass between processes.

## API

All functions return `{:ok, result}` or `{:error, reason}`.

```elixir
{:ok, ds} = ExGdal.open("/path/to/raster.tif")

ds.driver       #=> "GTiff"
ds.raster_count #=> 3
ds.raster_size  #=> {120, 116}

{:ok, :float64}  = ExGdal.band_type(ds, 1)
{:ok, -9999.0}   = ExGdal.no_data_value(ds, 1)
{:ok, data}      = ExGdal.read_band(ds, 1)          # full band, raw native-endian bytes
{:ok, window}    = ExGdal.read_band_window(ds, 1, 0, 0, 10, 10)
{:ok, gt}        = ExGdal.geo_transform(ds)          # %ExGdal.GeoTransform{}
{:ok, wkt}       = ExGdal.spatial_ref_wkt(ds)
{:ok, proj4}     = ExGdal.spatial_ref_proj4(ds)
{:ok, val}       = ExGdal.metadata_item(ds, "AREA_OR_POINT")
{:ok, "GTiff"}   = ExGdal.driver_name(ds)
```

Band indices are 1-based, matching GDAL convention.

`read_band/2` returns raw bytes in native endianness. For a float64 band on a 120x116 raster, that is `120 * 116 * 8 = 111_360` bytes. Use `band_type/2` to know how to interpret the binary.

### Structs

`%ExGdal.Dataset{}` holds the NIF resource reference along with cached `raster_count`, `raster_size`, `path`, and `driver` fields.

`%ExGdal.GeoTransform{}` has named fields: `origin_x`, `origin_y`, `pixel_width`, `pixel_height`, `skew_x`, `skew_y`. These correspond to GDAL's 6-element affine transform array.

## Prerequisites

- Erlang/OTP 27+
- Elixir 1.15+
- Rust 1.80+ (via rustup)
- `libgdal-dev` (system GDAL headers and shared library)

On Ubuntu/Debian:

```sh
sudo apt install libgdal-dev
```

On macOS:

```sh
brew install gdal
```

## Setup

```sh
git clone <repo-url> && cd ex_gdal
mix deps.get
mix compile
```

The first compile builds the Rust NIF crate in release mode. Subsequent compiles are incremental and fast.

## Running tests

```sh
mix test
```

Test fixtures (`tinymarble.tif`, `dem-hills.tiff`, `gcp.tif`) are in `test/fixtures/`, copied from the upstream gdal crate's fixture set.

## Project structure

```
mix.exs                             # Elixir project config
lib/
  ex_gdal.ex                       # Public API
  ex_gdal/
    native.ex                      # NIF function stubs (use Rustler)
    dataset.ex                     # %ExGdal.Dataset{} struct
    geo_transform.ex               # %ExGdal.GeoTransform{} struct
native/ex_gdal_nif/
  Cargo.toml                       # Rust crate config
  src/lib.rs                       # NIF implementations
test/
  ex_gdal_test.exs                 # Integration tests
  fixtures/                        # Sample raster files
```

## How the NIF works

The Rust NIF crate (`native/ex_gdal_nif`) depends on the `gdal` crate from crates.io (currently 0.19). It links against the system's `libgdal` shared library at compile time.

All I/O NIF functions (`gdal_open`, `gdal_read_band`, `gdal_read_band_window`) run on the BEAM dirty I/O scheduler so they do not block normal schedulers.

The `Dataset` from the gdal crate is `Send` but not `Sync`. It is wrapped in `Mutex<Dataset>` inside a `ResourceArc` to allow safe concurrent access from multiple BEAM processes.

## Precompiled NIF builds

The project includes `rustler_precompiled` as a dependency for future use. To ship precompiled binaries:

1. Set up a GitHub Actions workflow that builds the NIF for each target (x86_64-linux-gnu, aarch64-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin).
2. Upload the `.so`/`.dylib` files to a GitHub release with checksum files.
3. Switch `lib/ex_gdal/native.ex` from `use Rustler` to `use RustlerPrecompiled`.

For bundled builds that do not require system libgdal, add `gdal-src` to `Cargo.toml` dependencies with `features = ["internal_drivers"]`. This compiles GDAL from source (slow, but produces a fully static NIF).

## License

MIT

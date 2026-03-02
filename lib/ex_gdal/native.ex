defmodule ExGdal.Native do
  @moduledoc false

  version = Mix.Project.config()[:version]

  use RustlerPrecompiled,
    otp_app: :ex_gdal,
    crate: "ex_gdal_nif",
    base_url:
      "https://github.com/Environmental-Material-Science-Inc/ex_gdal/releases/download/v#{version}",
    version: version,
    force_build: System.get_env("EXGDAL_BUILD") in ["1", "true"],
    targets: [
      "x86_64-unknown-linux-gnu",
      "aarch64-unknown-linux-gnu",
      "x86_64-apple-darwin",
      "aarch64-apple-darwin"
    ]

  def gdal_open(_path), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_raster_count(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_raster_size(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_read_band(_resource, _band_idx), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_read_band_window(_resource, _band_idx, _x, _y, _w, _h), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_band_type(_resource, _band_idx), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_no_data_value(_resource, _band_idx), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_spatial_ref_wkt(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_spatial_ref_proj4(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_geo_transform(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_metadata_item(_resource, _key, _domain), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_metadata_domains(_resource), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_metadata_domain(_resource, _domain), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_band_description(_resource, _band_idx), do: :erlang.nif_error(:nif_not_loaded)
  def gdal_driver_name(_resource), do: :erlang.nif_error(:nif_not_loaded)

  def gdal_contours(_resource, _band_idx, _interval, _base, _fixed_levels, _polygonize, _nodata),
    do: :erlang.nif_error(:nif_not_loaded)
end

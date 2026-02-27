defmodule ExGdal.Native do
  @moduledoc false

  use Rustler, otp_app: :ex_gdal, crate: "ex_gdal_nif"

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
  def gdal_driver_name(_resource), do: :erlang.nif_error(:nif_not_loaded)
end

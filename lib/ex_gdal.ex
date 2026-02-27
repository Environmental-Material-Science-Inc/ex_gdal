defmodule ExGdal do
  @moduledoc """
  Elixir bindings for GDAL via Rustler NIF.

  Provides read access to raster datasets (GeoTIFF, etc.) through the GDAL library.
  """

  alias ExGdal.{Dataset, GeoTransform, Native}

  @doc """
  Opens a raster dataset at the given path.

  Returns `{:ok, %Dataset{}}` with cached metadata, or `{:error, reason}`.
  """
  @spec open(String.t()) :: {:ok, Dataset.t()} | {:error, String.t()}
  def open(path) do
    with {:ok, ref} <- Native.gdal_open(path),
         {:ok, count} <- Native.gdal_raster_count(ref),
         {:ok, size} <- Native.gdal_raster_size(ref),
         {:ok, driver} <- Native.gdal_driver_name(ref) do
      {:ok,
       %Dataset{
         ref: ref,
         path: path,
         raster_count: count,
         raster_size: size,
         driver: driver
       }}
    end
  end

  @doc "Returns the number of raster bands."
  @spec band_count(Dataset.t()) :: {:ok, non_neg_integer()} | {:error, String.t()}
  def band_count(%Dataset{raster_count: count}), do: {:ok, count}

  @doc "Returns `{width, height}` in pixels."
  @spec raster_size(Dataset.t()) :: {:ok, {non_neg_integer(), non_neg_integer()}} | {:error, String.t()}
  def raster_size(%Dataset{raster_size: size}), do: {:ok, size}

  @doc """
  Reads an entire band as a raw binary of native-endian pixels.

  Band index is 1-based.
  """
  @spec read_band(Dataset.t(), pos_integer()) :: {:ok, binary()} | {:error, String.t()}
  def read_band(%Dataset{ref: ref}, band_idx) do
    Native.gdal_read_band(ref, band_idx)
  end

  @doc """
  Reads a rectangular window from a band as raw bytes.

  Band index is 1-based. (x, y) is the top-left corner, (w, h) is the window size.
  """
  @spec read_band_window(Dataset.t(), pos_integer(), integer(), integer(), non_neg_integer(), non_neg_integer()) ::
          {:ok, binary()} | {:error, String.t()}
  def read_band_window(%Dataset{ref: ref}, band_idx, x, y, w, h) do
    Native.gdal_read_band_window(ref, band_idx, x, y, w, h)
  end

  @doc """
  Returns the data type of a band as an atom.

  Possible values: `:uint8`, `:int16`, `:uint16`, `:int32`, `:uint32`,
  `:float32`, `:float64`, `:unknown`.
  """
  @spec band_type(Dataset.t(), pos_integer()) :: {:ok, atom()} | {:error, String.t()}
  def band_type(%Dataset{ref: ref}, band_idx) do
    Native.gdal_band_type(ref, band_idx)
  end

  @doc "Returns the no-data value for a band, or `{:ok, nil}` if not set."
  @spec no_data_value(Dataset.t(), pos_integer()) :: {:ok, float() | nil} | {:error, String.t()}
  def no_data_value(%Dataset{ref: ref}, band_idx) do
    Native.gdal_no_data_value(ref, band_idx)
  end

  @doc "Returns the spatial reference as WKT."
  @spec spatial_ref_wkt(Dataset.t()) :: {:ok, String.t()} | {:error, String.t()}
  def spatial_ref_wkt(%Dataset{ref: ref}) do
    Native.gdal_spatial_ref_wkt(ref)
  end

  @doc "Returns the spatial reference as PROJ4 string."
  @spec spatial_ref_proj4(Dataset.t()) :: {:ok, String.t()} | {:error, String.t()}
  def spatial_ref_proj4(%Dataset{ref: ref}) do
    Native.gdal_spatial_ref_proj4(ref)
  end

  @doc "Returns the geo-transform as a `%GeoTransform{}` struct."
  @spec geo_transform(Dataset.t()) :: {:ok, GeoTransform.t()} | {:error, String.t()}
  def geo_transform(%Dataset{ref: ref}) do
    case Native.gdal_geo_transform(ref) do
      {:ok, list} when is_list(list) and length(list) == 6 ->
        {:ok, GeoTransform.from_list(list)}

      {:error, _} = err ->
        err
    end
  end

  @doc """
  Reads a metadata item. Domain defaults to `""` (the default domain).
  """
  @spec metadata_item(Dataset.t(), String.t(), String.t()) :: {:ok, String.t() | nil} | {:error, String.t()}
  def metadata_item(%Dataset{ref: ref}, key, domain \\ "") do
    Native.gdal_metadata_item(ref, key, domain)
  end

  @doc "Returns the short driver name (e.g. `\"GTiff\"`)."
  @spec driver_name(Dataset.t()) :: {:ok, String.t()} | {:error, String.t()}
  def driver_name(%Dataset{driver: driver}), do: {:ok, driver}
end

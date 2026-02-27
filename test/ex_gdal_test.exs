defmodule ExGdalTest do
  use ExUnit.Case

  @tinymarble "test/fixtures/tinymarble.tif"
  @dem_hills "test/fixtures/dem-hills.tiff"

  describe "open/1" do
    test "opens a valid GeoTIFF" do
      assert {:ok, %ExGdal.Dataset{} = ds} = ExGdal.open(@tinymarble)
      assert ds.path == @tinymarble
      assert ds.driver == "GTiff"
      assert ds.raster_count > 0
      assert {w, h} = ds.raster_size
      assert w > 0 and h > 0
    end

    test "returns error for missing file" do
      assert {:error, reason} = ExGdal.open("nonexistent.tif")
      assert is_binary(reason)
    end
  end

  describe "band_count/1" do
    test "tinymarble has 3 bands" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, 3} = ExGdal.band_count(ds)
    end
  end

  describe "raster_size/1" do
    test "returns width and height" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, {w, h}} = ExGdal.raster_size(ds)
      assert w > 0
      assert h > 0
    end
  end

  describe "read_band/2" do
    test "reads band 1 of tinymarble as binary" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, data} = ExGdal.read_band(ds, 1)
      assert is_binary(data)
      {:ok, {w, h}} = ExGdal.raster_size(ds)
      # uint8 band: 1 byte per pixel
      assert byte_size(data) == w * h
    end

    test "returns error for invalid band index" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:error, _} = ExGdal.read_band(ds, 0)
    end
  end

  describe "read_band_window/6" do
    test "reads a sub-region" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, data} = ExGdal.read_band_window(ds, 1, 0, 0, 10, 10)
      assert byte_size(data) == 10 * 10
    end
  end

  describe "band_type/2" do
    test "tinymarble bands are uint8" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, :uint8} = ExGdal.band_type(ds, 1)
    end

    test "dem-hills band is float32" do
      {:ok, ds} = ExGdal.open(@dem_hills)
      assert {:ok, :float32} = ExGdal.band_type(ds, 1)
    end
  end

  describe "no_data_value/2" do
    test "tinymarble has no nodata" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, nil} = ExGdal.no_data_value(ds, 1)
    end
  end

  describe "spatial_ref_wkt/1" do
    test "returns WKT string for dataset with SRS" do
      {:ok, ds} = ExGdal.open(@dem_hills)
      assert {:ok, wkt} = ExGdal.spatial_ref_wkt(ds)
      assert is_binary(wkt)
      assert String.contains?(wkt, "GEOGCS") or String.contains?(wkt, "GEOGCRS")
    end

    test "returns error for dataset without SRS" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:error, _} = ExGdal.spatial_ref_wkt(ds)
    end
  end

  describe "spatial_ref_proj4/1" do
    test "returns proj4 string for dataset with SRS" do
      {:ok, ds} = ExGdal.open(@dem_hills)
      assert {:ok, proj4} = ExGdal.spatial_ref_proj4(ds)
      assert is_binary(proj4)
      assert String.contains?(proj4, "+proj")
    end
  end

  describe "geo_transform/1" do
    test "returns GeoTransform struct" do
      {:ok, ds} = ExGdal.open(@dem_hills)
      assert {:ok, %ExGdal.GeoTransform{} = gt} = ExGdal.geo_transform(ds)
      assert is_float(gt.origin_x)
      assert is_float(gt.origin_y)
      assert is_float(gt.pixel_width)
      assert is_float(gt.pixel_height)
    end
  end

  describe "driver_name/1" do
    test "returns GTiff for GeoTIFF" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, "GTiff"} = ExGdal.driver_name(ds)
    end
  end

  describe "metadata_item/3" do
    test "returns nil for missing key" do
      {:ok, ds} = ExGdal.open(@tinymarble)
      assert {:ok, nil} = ExGdal.metadata_item(ds, "NONEXISTENT_KEY")
    end
  end
end

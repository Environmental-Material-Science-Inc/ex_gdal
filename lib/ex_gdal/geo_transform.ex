defmodule ExGdal.GeoTransform do
  @moduledoc """
  Represents a GDAL 6-element affine geo-transform.

  The transform maps pixel/line coordinates to georeferenced coordinates:

      x_geo = origin_x + pixel * pixel_width + line * skew_x
      y_geo = origin_y + pixel * skew_y + line * pixel_height
  """

  defstruct [:origin_x, :pixel_width, :skew_x, :origin_y, :skew_y, :pixel_height]

  @type t :: %__MODULE__{
          origin_x: float(),
          pixel_width: float(),
          skew_x: float(),
          origin_y: float(),
          skew_y: float(),
          pixel_height: float()
        }

  @doc "Build from GDAL's [f64; 6] list."
  def from_list([origin_x, pixel_width, skew_x, origin_y, skew_y, pixel_height]) do
    %__MODULE__{
      origin_x: origin_x,
      pixel_width: pixel_width,
      skew_x: skew_x,
      origin_y: origin_y,
      skew_y: skew_y,
      pixel_height: pixel_height
    }
  end
end

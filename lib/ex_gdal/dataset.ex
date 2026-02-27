defmodule ExGdal.Dataset do
  @moduledoc """
  Wraps a NIF reference to an opened GDAL dataset.
  """

  defstruct [:ref, :path, :raster_count, :raster_size, :driver]

  @type t :: %__MODULE__{
          ref: reference(),
          path: String.t(),
          raster_count: non_neg_integer(),
          raster_size: {non_neg_integer(), non_neg_integer()},
          driver: String.t()
        }
end

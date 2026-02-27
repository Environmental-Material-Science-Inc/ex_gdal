defmodule ExGdal.MixProject do
  use Mix.Project

  @version "0.1.0"

  def project do
    [
      app: :ex_gdal,
      version: @version,
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger]
    ]
  end

  defp deps do
    [
      {:rustler, "~> 0.36.0", runtime: false},
      {:rustler_precompiled, "~> 0.8"}
    ]
  end
end

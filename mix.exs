defmodule ExGdal.MixProject do
  use Mix.Project

  @version "0.1.0"

  def project do
    [
      app: :ex_gdal,
      version: @version,
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      description: description(),
      package: package(),
      name: "ExGdal",
      source_url: "https://github.com/Environmental-Material-Science-Inc/ex_gdal"
    ]
  end

  def application do
    [
      extra_applications: [:logger]
    ]
  end

  defp description() do
    "Elixir bindings to https://gdal.org/en/stable/ via https://github.com/georust/gdal and https://github.com/rusterlium/rustler"
  end

  defp package() do
    %{
      files: ~w(lib priv native .formatter.exs mix.exs README.md LICENSE.md),
      licenses: ["MIT"],
      links: %{"GitHub" => "https://github.com/Environmental-Material-Science-Inc/ex_gdal"}
    }
  end

  defp deps do
    [
      {:rustler, "~> 0.36.0", runtime: false},
      {:rustler_precompiled, "~> 0.8"},
      {:ex_doc, "~> 0.14", only: :dev, runtime: false}
    ]
  end
end

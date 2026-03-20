# Windows Support

Windows (`x86_64-pc-windows-msvc`) builds are currently disabled. This document
records what was done to get Windows partially working and what remains to be
fixed.

## What we did

### 1. Added Windows to CI matrix and RustlerPrecompiled targets

Commit `4a43e20` added `x86_64-pc-windows-msvc` to the build matrix in
`.github/workflows/release.yml` and to the target list in `lib/ex_gdal/native.ex`.

### 2. Enabled long paths

Commit `d749ee0` added a CI step to run `git config --system core.longpaths true`
before checkout. Without this, the GDAL git submodule checkout fails because
some file paths in the GDAL source tree exceed the Windows 260-character limit.

### 3. Made libsqlite3-sys always bundled

Commit `c8e3502` (in the [ems gdal fork]) changed `libsqlite3-sys` from an
optional dependency (gated behind `driver_sqlite`) to a mandatory dependency
with the `bundled` feature enabled. PROJ always requires SQLite3, and there is
no system SQLite3 on Windows CI runners, so the bundled build is necessary.

## What still needs to be fixed

### 1. PROJ cmake requires the `sqlite3` CLI binary

PROJ 9.6.2's CMakeLists.txt runs `find_program(EXE_SQLITE3 sqlite3)` and fails
with `sqlite3 binary not found!` if the executable isn't on PATH. The sqlite3
CLI is used at build time to generate `proj.db`.

**Fix:** Install sqlite3 tools on the Windows CI runner before the build step:

```yaml
- name: Install SQLite3 tools (Windows)
  if: runner.os == 'Windows'
  run: choco install sqlite -y
```

### 2. proj-sys hardcodes `libsqlite3.a` library path

In `proj-sys` v0.27.0 (`build.rs` line 126), the SQLite3 library path is
constructed as:

```rust
config.define("SQLITE3_LIBRARY", format!("{sqlite_lib_dir}/libsqlite3.a"));
```

On MSVC, `libsqlite3-sys` (via the `cc` crate) produces `sqlite3.lib`, not
`libsqlite3.a`. This causes PROJ's cmake to fail at link time because the
library file doesn't exist.

**Fix:** This is an upstream bug in `proj-sys`. Options:

- **Upstream PR:** Fix `proj-sys` to use the correct filename per platform:
  ```rust
  let lib_file = if cfg!(target_env = "msvc") {
      format!("{sqlite_lib_dir}/sqlite3.lib")
  } else {
      format!("{sqlite_lib_dir}/libsqlite3.a")
  };
  config.define("SQLITE3_LIBRARY", lib_file);
  ```
- **Fork:** Fork [georust/proj] to the ems org, apply the fix, and update
  `gdal-src/Cargo.toml` to use the git dependency for `proj-sys`.
- **Cargo patch:** Add a `[patch.crates-io]` section in
  `native/ex_gdal_nif/Cargo.toml` pointing `proj-sys` to a fixed fork.

### 3. proj-sys uses deprecated cmake variable names

PROJ 9.6.2 warns:

```
Use SQLite3_INCLUDE_DIR instead of SQLITE3_INCLUDE_DIR
Use SQLite3_LIBRARY instead of SQLITE3_LIBRARY
```

This is non-blocking today but may break in a future PROJ release. The fix
is the same as issue 2 (patch proj-sys build.rs to use the new variable names).

## Summary

| Issue | Blocker? | Where to fix |
|-------|----------|-------------|
| Long paths | Fixed | CI workflow |
| Bundled libsqlite3-sys | Fixed | ems gdal fork |
| Missing sqlite3 CLI | Yes | CI workflow |
| Wrong library filename on MSVC | Yes | proj-sys (upstream or fork) |
| Deprecated cmake var names | No (warning) | proj-sys |

[ems gdal fork]: https://github.com/Environmental-Material-Science-Inc/gdal
[georust/proj]: https://github.com/georust/proj

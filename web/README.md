# openparts-web

The OpenParts web site: a single-page app written in
[MoonBit](https://www.moonbitlang.com) with
[rabbita](https://github.com/moonbit-community/rabbita), built with Vite.

- `cmd/app/` -- the app: routing (`route.mbt`, English pages under `/en/`),
  API calls (`api.mbt`), pages (`view.mbt`), the part viewer's mount
  (`viewer.mbt`), strings (`i18n.mbt`) and the few browser calls
  (`ffi.mbt`). The KiCad bundle zip is built in the browser with
  `bobzhang/zip`.
- The part viewer -- the 3D model and the footprint with datasheet
  dimensions and a measure tool -- is Rust:
  [`crates/viewer/openparts-viewer`](../crates/viewer/openparts-viewer)
  (wgpu on WebGPU, WebGL2 where WebGPU isn't available), compiled to
  WebAssembly. `scripts/build-wasm.sh` builds it and runs wasm-bindgen
  (`--target web`) into `src/viewer-pkg/` (generated, not committed);
  `src/viewer.js` loads it on the first part page and `ffi.mbt`'s
  `viewer_mount` calls its `mount(model, footprint, options)`. It
  computes the geometry itself (openparts-mcad/-pcbcad from the
  package JSON), so dimension lines sit exactly on the model, which it
  loads from the API's 3MF (STL as the fallback). Its UI strings come
  from `i18n.mbt` (`viewer_labels`).
- `api/` -- MoonBit types of every JSON document openparts-server
  returns. `api/types.mbt` is **generated** from the Rust types
  (schemars JSON Schema -> `scripts/gen_api_types.py`); never edit it,
  run `scripts/gen-api-types.sh`. CI fails when it is stale.
  `api/decode_test.mbt` decodes real responses with them.

## Develop

Needs `moon`, `bun` (or npm), and for the viewer Rust with the
`wasm32-unknown-unknown` target and `wasm-bindgen-cli` at exactly the
`wasm-bindgen` version in `Cargo.lock`
(`scripts/build-wasm.sh --bindgen-version` prints it).

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version "$(scripts/build-wasm.sh --bindgen-version)"
moon update             # the registry index (once)
moon check --target js  # fetches the MoonBit dependencies (.mooncakes/)
bun install             # Vite
bun run build:wasm      # the viewer only -> src/viewer-pkg/
bun run dev             # (viewer +) http://localhost:5173, /api proxied to :8080
bun run test            # moon test --target js
bun run build           # (viewer +) dist/
```

The viewer's own tests are Rust:
`cargo test -p openparts-viewer` (geometry, parsing, dimensions,
snapping; the browser side compiles only for wasm32:
`cargo clippy -p openparts-viewer --target wasm32-unknown-unknown`).

Run openparts-server on port 8080 for the API (`/api` is proxied in
dev and preview; the server has no CORS).

## Deploy

- Behind openparts-server: build with the default base `/` and point
  `OPENPARTS_WEB_DIR` at `dist/`; the server serves `index.html` for
  every path that isn't a file or the API, and the app routes.
- GitHub Pages: `OPENPARTS_BASE=/openparts/` (the project site's path);
  `dist/404.html` is a copy of `index.html` so deep links work.
  `VITE_OPENPARTS_API_BASE` sets the API's URL (default `/api`).

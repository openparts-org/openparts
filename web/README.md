# openparts-web

The OpenParts web site: a single-page app written in
[MoonBit](https://www.moonbitlang.com) with
[rabbita](https://github.com/moonbit-community/rabbita), built with Vite.

- `cmd/app/` -- the app: routing (`route.mbt`, English pages under `/en/`),
  API calls (`api.mbt`), pages (`view.mbt`), the 3D preview
  (`viewer.mbt`, three.js through `mizchi/three`), strings (`i18n.mbt`)
  and the few browser calls (`ffi.mbt`). The KiCad bundle zip is built
  in the browser with `bobzhang/zip`.
- `api/` -- MoonBit types of every JSON document openparts-server
  returns. `api/types.mbt` is **generated** from the Rust types
  (schemars JSON Schema -> `scripts/gen_api_types.py`); never edit it,
  run `scripts/gen-api-types.sh`. CI fails when it is stale.
  `api/decode_test.mbt` decodes real responses with them.

## Develop

Needs `moon` and `bun` (or npm).

```sh
moon check --target js  # fetches the MoonBit dependencies (.mooncakes/)
bun install             # Vite, three.js and the three.js bindings' JS
bun run dev             # http://localhost:5173, /api proxied to :8080
bun run test            # moon test --target js
bun run build           # dist/
```

Run openparts-server on port 8080 for the API (`/api` is proxied in
dev and preview; the server has no CORS).

## Deploy

- Behind openparts-server: build with the default base `/` and point
  `OPENPARTS_WEB_DIR` at `dist/`; the server serves `index.html` for
  every path that isn't a file or the API, and the app routes.
- GitHub Pages: `OPENPARTS_BASE=/openparts/` (the project site's path);
  `dist/404.html` is a copy of `index.html` so deep links work.
  `VITE_OPENPARTS_API_BASE` sets the API's URL (default `/api`).

# openparts-web

The [OpenParts](https://github.com/openparts-org/openparts) website: a landing page plus a parts
search/browse UI backed by `openparts-server`'s REST API (fetched client-side, not baked in at
build time). Astro + Preact islands, TypeScript, [bun](https://bun.sh). Japanese (default, `/...`)
and English (`/en/...`).

## Local development

Clone as a sibling of `openparts-server` and `openparts-data`:

```
openparts/
├── openparts-web/      (this repo)
├── openparts-server/
└── openparts-data/
```

1. Start the API server:

   ```sh
   cd ../openparts-server
   OPENPARTS_DATA_DIR=../openparts-data cargo run
   ```

2. Install and run this site:

   ```sh
   bun install
   bun dev
   ```

   Open `http://localhost:4321/openparts/` (`base` is set for GitHub Pages).

`openparts-server` has no CORS middleware, so the Vite dev proxy in `astro.config.mjs` forwards
`/api/*` to it, keeping the browser same-origin. Override the API base with
`PUBLIC_OPENPARTS_API_BASE` (see `.env.example`).

## Build

```sh
bun build
bun preview
```

The dev proxy only applies to `bun dev` -- `bun preview` serves the static build as-is, so live API
calls need `PUBLIC_OPENPARTS_API_BASE` pointed at a reachable, same-origin API.

## Deploy (GitHub Pages)

`.github/workflows/deploy.yml` builds and deploys on push to `main` (requires Pages "Source" set to
GitHub Actions in repo Settings). `openparts-server` isn't deployed anywhere yet, so search/part
detail/3D preview/downloads show their error states until one exists -- once it does, set the
`OPENPARTS_API_BASE` repo variable to its URL and CORS support will need adding there too if it's on
a different origin.

## Contributing

Part data lives in `openparts-data`. See this site's [`/contributing`](http://localhost:4321/contributing) page.

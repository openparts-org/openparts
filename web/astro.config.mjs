// @ts-check
import { defineConfig } from 'astro/config';

import preact from '@astrojs/preact';

// https://astro.build/config
export default defineConfig({
  // Default: the GitHub Pages project site, served at
  // https://openparts-org.github.io/openparts/, so internal links
  // must be prefixed with this base (see src/lib/base.ts). A self-hosted
  // build (openparts-server serving the site at the root of its own
  // domain, the API under /api) sets OPENPARTS_SITE and OPENPARTS_BASE=/.
  site: process.env.OPENPARTS_SITE ?? 'https://openparts-org.github.io',
  base: process.env.OPENPARTS_BASE ?? '/openparts',
  integrations: [preact()],
  vite: {
    server: {
      proxy: {
        // openparts-server has no CORS middleware; proxy /api/* to it in
        // dev so the browser only ever talks same-origin.
        '/api': {
          target: 'http://localhost:8080',
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api/, ''),
        },
      },
    },
  },
});
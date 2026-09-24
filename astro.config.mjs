// @ts-check
import { defineConfig } from 'astro/config';

import preact from '@astrojs/preact';

// https://astro.build/config
export default defineConfig({
  // GitHub Pages project site: served at
  // https://openparts-org.github.io/openparts-web/, so internal links
  // must be prefixed with this base (see src/lib/base.ts).
  site: 'https://openparts-org.github.io',
  base: '/openparts-web',
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
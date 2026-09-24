// @ts-check
import { defineConfig } from 'astro/config';

import preact from '@astrojs/preact';

// https://astro.build/config
export default defineConfig({
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
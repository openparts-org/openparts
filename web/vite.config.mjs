import { defineConfig } from "vite";
import moonbit from "vite-plugin-moonbit";

// The site is a single-page MoonBit (rabbita) app. OPENPARTS_BASE is
// where it is served from: "/" behind openparts-server (the default),
// "/openparts/" for the GitHub Pages project site. VITE_OPENPARTS_API_BASE
// is the API's URL ("/api", same origin, when unset).
const api = { "/api": "http://127.0.0.1:8080" };

export default defineConfig({
  base: process.env.OPENPARTS_BASE ?? "/",
  plugins: [moonbit({ target: "js", watch: false })],
  // openparts-server has no CORS: proxy /api to it in dev and preview.
  server: { proxy: api },
  preview: { port: 4173, proxy: api },
});

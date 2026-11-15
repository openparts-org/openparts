// The part viewer (crates/viewer/openparts-viewer: Rust + wgpu compiled
// to WebAssembly, built into ./viewer-pkg by scripts/build-wasm.sh),
// loaded on the first part page. cmd/app/ffi.mbt calls this.
let loading;
globalThis.openpartsViewer = () =>
  (loading ??= import("./viewer-pkg/openparts_viewer.js").then(async (m) => {
    await m.default();
    return m;
  }));

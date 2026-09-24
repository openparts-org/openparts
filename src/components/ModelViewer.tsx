import { useEffect, useRef, useState } from "preact/hooks";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { STLLoader } from "three/addons/loaders/STLLoader.js";
import { artifactDownloadUrl } from "../lib/api";
import type { ApiErrorBody } from "../lib/types";

type Status = "loading" | "ready" | "error";

export default function ModelViewer({
  manufacturer,
  mpn,
}: {
  manufacturer: string;
  mpn: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    let renderer: THREE.WebGLRenderer | null = null;
    let controls: OrbitControls | null = null;
    let mesh: THREE.Mesh | null = null;
    let frameId = 0;
    let resizeObserver: ResizeObserver | null = null;

    async function init() {
      setStatus("loading");
      setErrorMessage(null);

      const url = artifactDownloadUrl(manufacturer, mpn, "stl");
      const res = await fetch(url);
      if (!res.ok) {
        const body: ApiErrorBody = await res
          .json()
          .catch(() => ({ error: res.statusText }));
        if (!cancelled) {
          setErrorMessage(body.error);
          setStatus("error");
        }
        return;
      }
      const buffer = await res.arrayBuffer();
      if (cancelled || !container) return;

      const geometry = new STLLoader().parse(buffer);
      geometry.computeBoundingSphere();
      geometry.center();
      const radius = geometry.boundingSphere?.radius ?? 1;

      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0xffffff);

      const camera = new THREE.PerspectiveCamera(
        45,
        container.clientWidth / container.clientHeight,
        radius / 100,
        radius * 100,
      );
      camera.position.set(radius * 1.8, radius * 1.4, radius * 1.8);

      renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(container.clientWidth, container.clientHeight);
      container.innerHTML = "";
      container.appendChild(renderer.domElement);

      const material = new THREE.MeshStandardMaterial({
        color: 0x8f97a3,
        metalness: 0.2,
        roughness: 0.6,
      });
      mesh = new THREE.Mesh(geometry, material);
      scene.add(mesh);

      scene.add(new THREE.HemisphereLight(0xffffff, 0x888888, 1.2));
      const keyLight = new THREE.DirectionalLight(0xffffff, 1.2);
      keyLight.position.set(radius * 2, radius * 3, radius * 2);
      scene.add(keyLight);

      controls = new OrbitControls(camera, renderer.domElement);
      controls.target.set(0, 0, 0);
      controls.enableDamping = true;
      controls.minDistance = radius * 0.5;
      controls.maxDistance = radius * 10;
      controls.update();

      function resize() {
        if (!renderer || !container) return;
        const { clientWidth, clientHeight } = container;
        if (clientWidth === 0 || clientHeight === 0) return;
        camera.aspect = clientWidth / clientHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(clientWidth, clientHeight);
      }

      resizeObserver = new ResizeObserver(resize);
      resizeObserver.observe(container);

      function animate() {
        frameId = requestAnimationFrame(animate);
        controls?.update();
        if (renderer) renderer.render(scene, camera);
      }
      animate();

      if (!cancelled) setStatus("ready");
    }

    init().catch((err) => {
      if (!cancelled) {
        setErrorMessage(
          err instanceof Error ? err.message : "3Dモデルの表示に失敗しました。",
        );
        setStatus("error");
      }
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      controls?.dispose();
      if (mesh) {
        mesh.geometry.dispose();
        (mesh.material as THREE.Material).dispose();
      }
      if (renderer) {
        renderer.dispose();
        renderer.domElement.remove();
      }
    };
  }, [manufacturer, mpn]);

  return (
    <div>
      <div ref={containerRef} class="model-viewer-canvas">
        {status === "loading" && (
          <p class="text-muted model-viewer-overlay">読み込み中...</p>
        )}
        {status === "error" && (
          <p class="text-muted model-viewer-overlay">
            {errorMessage ?? "3Dモデルを表示できませんでした。"}
          </p>
        )}
      </div>
      {status === "ready" && (
        <p class="text-muted model-viewer-hint">
          ドラッグで回転、スクロールでズームできます。
        </p>
      )}
    </div>
  );
}

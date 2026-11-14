import { useEffect, useRef, useState } from "preact/hooks";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { STLLoader } from "three/addons/loaders/STLLoader.js";
import { ThreeMFLoader } from "three/addons/loaders/3MFLoader.js";
import { artifactDownloadUrl } from "../lib/api";
import { t, type Lang } from "../lib/i18n";
import type { ApiErrorBody } from "../lib/types";

type Status = "loading" | "ready" | "error";

// "Nice" grid cell sizes (mm) to choose from, so the floor grid reads like
// graph paper (1mm/2mm/5mm/10mm/... squares) rather than an arbitrary
// fraction of the model's size.
const NICE_STEPS_MM = [0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500];

function chooseGridStep(footprint: number): number {
  // Aim for the grid to span roughly footprint * 3, subdivided into
  // somewhere around 10-20 squares -- few enough to read individual
  // squares, many enough to judge size at a glance.
  const targetSpan = footprint * 3;
  let best = NICE_STEPS_MM[0];
  let bestDiff = Infinity;
  for (const step of NICE_STEPS_MM) {
    const divisions = targetSpan / step;
    const diff = Math.abs(divisions - 15);
    if (diff < bestDiff) {
      bestDiff = diff;
      best = step;
    }
  }
  return best;
}

type Loaded = { object: THREE.Object3D } | ApiErrorBody;

async function errorBody(res: Response): Promise<ApiErrorBody> {
  return res.json().catch(() => ({ error: res.statusText }));
}

// Prefers the 3MF artifact, which keeps SolidCraft's colour per body
// (mold compound, plated leads, polarity marks, ...); falls back to the
// uncoloured STL, drawn in a neutral grey, when there is no 3MF or it
// doesn't parse.
async function loadModel(manufacturer: string, mpn: string): Promise<Loaded> {
  const threeMf = await fetch(artifactDownloadUrl(manufacturer, mpn, "3mf"));
  if (threeMf.ok) {
    try {
      const group = new ThreeMFLoader().parse(await threeMf.arrayBuffer());
      if (group.children.length > 0) return { object: group };
    } catch {
      // fall through to the STL
    }
  }

  const stl = await fetch(artifactDownloadUrl(manufacturer, mpn, "stl"));
  if (!stl.ok) return errorBody(stl);
  const geometry = new STLLoader().parse(await stl.arrayBuffer());
  const material = new THREE.MeshStandardMaterial({
    color: 0x8f97a3,
    metalness: 0.2,
    roughness: 0.6,
  });
  return { object: new THREE.Mesh(geometry, material) };
}

function disposeObject(object: THREE.Object3D) {
  object.traverse((child) => {
    if (!(child instanceof THREE.Mesh)) return;
    child.geometry.dispose();
    const materials = Array.isArray(child.material)
      ? child.material
      : [child.material];
    for (const material of materials) {
      (material as THREE.Material & { map?: THREE.Texture | null }).map?.dispose();
      material.dispose();
    }
  });
}

export default function ModelViewer({
  manufacturer,
  mpn,
  lang,
}: {
  manufacturer: string;
  mpn: string;
  lang: Lang;
}) {
  const copy = t[lang].part;
  const containerRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    let renderer: THREE.WebGLRenderer | null = null;
    let controls: OrbitControls | null = null;
    let model: THREE.Group | null = null;
    let grid: THREE.GridHelper | null = null;
    let frameId = 0;
    let resizeObserver: ResizeObserver | null = null;

    async function init() {
      setStatus("loading");
      setErrorMessage(null);

      const loaded = await loadModel(manufacturer, mpn);
      if (cancelled || !container) {
        if ("object" in loaded) disposeObject(loaded.object);
        return;
      }
      if ("error" in loaded) {
        setErrorMessage(loaded.error);
        setStatus("error");
        return;
      }

      // 3MF/STL artifacts use OpenParts' mechanical-CAD "Z is up"
      // convention, while three.js is Y-up -- rotate so the model sits
      // flat on the floor grid the way it physically would on a bench,
      // instead of appearing on its side, then centre it on the origin.
      model = new THREE.Group();
      model.add(loaded.object);
      model.rotation.x = -Math.PI / 2;
      model.updateMatrixWorld(true);
      const center = new THREE.Box3()
        .setFromObject(model)
        .getCenter(new THREE.Vector3());
      model.position.sub(center);
      model.updateMatrixWorld(true);

      const box = new THREE.Box3().setFromObject(model);
      const size = box.getSize(new THREE.Vector3());
      const radius = box.isEmpty()
        ? 1
        : box.getBoundingSphere(new THREE.Sphere()).radius ||
          Math.max(size.x, size.y, size.z, 1) / 2;

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

      scene.add(model);

      // Real-world-scale (mm) floor grid, sized off the model's actual
      // footprint, positioned flush with the model's bottom face.
      const footprint = Math.max(size.x, size.z);
      const step = chooseGridStep(footprint);
      const divisions = Math.max(2, Math.round((footprint * 3) / step));
      const gridSize = step * divisions;
      grid = new THREE.GridHelper(gridSize, divisions, 0xb8bcc4, 0xe3e3e6);
      grid.position.y = box.min.y;
      scene.add(grid);

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
          err instanceof Error ? err.message : copy.modelLoadFailed,
        );
        setStatus("error");
      }
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      controls?.dispose();
      if (model) disposeObject(model);
      grid?.geometry.dispose();
      if (grid?.material) {
        const gridMaterial = grid.material as THREE.Material | THREE.Material[];
        (Array.isArray(gridMaterial) ? gridMaterial : [gridMaterial]).forEach(
          (m) => m.dispose(),
        );
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
          <p class="text-muted model-viewer-overlay">{copy.modelLoading}</p>
        )}
        {status === "error" && (
          <p class="text-muted model-viewer-overlay">
            {errorMessage ?? copy.modelLoadFailed}
          </p>
        )}
      </div>
    </div>
  );
}

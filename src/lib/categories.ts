import type { ComponentCategory } from "./types";

export const CATEGORY_LABELS: Record<ComponentCategory, string> = {
  microcontroller: "マイコン",
  capacitor: "コンデンサ",
  resistor: "抵抗",
  inductor: "インダクタ",
  diode: "ダイオード",
  transistor: "トランジスタ",
  led: "LED",
  crystal: "水晶発振子",
  other: "その他",
};

export const CATEGORIES = Object.keys(CATEGORY_LABELS) as ComponentCategory[];

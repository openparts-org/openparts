import type { Lang } from "./i18n";
import type { ComponentCategory } from "./types";

const CATEGORY_LABELS: Record<Lang, Record<ComponentCategory, string>> = {
  ja: {
    microcontroller: "マイコン",
    capacitor: "コンデンサ",
    resistor: "抵抗",
    inductor: "インダクタ",
    diode: "ダイオード",
    transistor: "トランジスタ",
    led: "LED",
    crystal: "水晶発振子",
    fuse: "ヒューズ",
    connector: "コネクタ",
    other: "その他",
  },
  en: {
    microcontroller: "Microcontroller",
    capacitor: "Capacitor",
    resistor: "Resistor",
    inductor: "Inductor",
    diode: "Diode",
    transistor: "Transistor",
    led: "LED",
    crystal: "Crystal",
    fuse: "Fuse",
    connector: "Connector",
    other: "Other",
  },
};

export const CATEGORIES = Object.keys(
  CATEGORY_LABELS.en,
) as ComponentCategory[];

export function categoryLabel(lang: Lang, category: ComponentCategory): string {
  return CATEGORY_LABELS[lang][category];
}

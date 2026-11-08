/**
 * Theme engine data model.
 *
 * Every colour the interface uses comes from these tokens; components must not
 * hardcode hex values. Adding a theme means adding one entry here — the token
 * names are the contract (see docs/DESIGN.md).
 */

export type ThemeTokens = {
  scheme: "dark" | "light";
  bg: string;
  bgElev: string;
  surface: string;
  surfaceAlt: string;
  border: string;
  borderStrong: string;
  text: string;
  textMuted: string;
  textFaint: string;
  accent: string;
  accentSoft: string;
  accentInk: string;
  ok: string;
  warn: string;
  danger: string;
  info: string;
  shadow: string;
};

export type Theme = {
  id: string;
  name: string;
  blurb: string;
  tokens: ThemeTokens;
};

export const themes: Theme[] = [
  {
    id: "graft",
    name: "GRAFT Industrial",
    blurb: "Graphite · steel · restrained amber",
    tokens: {
      scheme: "dark",
      bg: "#0b0d0e",
      bgElev: "#0f1213",
      surface: "#121516",
      surfaceAlt: "#171b1d",
      border: "#232829",
      borderStrong: "#333b3c",
      text: "#e2e6e6",
      textMuted: "#9aa3a4",
      textFaint: "#6a7374",
      accent: "#d9a441",
      accentSoft: "rgba(217, 164, 65, 0.14)",
      accentInk: "#15120a",
      ok: "#79b47a",
      warn: "#d9a441",
      danger: "#d16a5f",
      info: "#7ba7c4",
      shadow: "rgba(0, 0, 0, 0.45)",
    },
  },
  {
    id: "arch",
    name: "Arch",
    blurb: "Deep blue · cyan · neutral surfaces",
    tokens: {
      scheme: "dark",
      bg: "#060b12",
      bgElev: "#08101a",
      surface: "#0b1420",
      surfaceAlt: "#101b28",
      border: "#16232f",
      borderStrong: "#22364a",
      text: "#dde7f0",
      textMuted: "#8fa3b6",
      textFaint: "#5d7183",
      accent: "#1793d1",
      accentSoft: "rgba(23, 147, 209, 0.16)",
      accentInk: "#04121c",
      ok: "#5fb98f",
      warn: "#d5b45f",
      danger: "#d2695f",
      info: "#5bb8de",
      shadow: "rgba(0, 0, 0, 0.5)",
    },
  },
  {
    id: "void",
    name: "Void",
    blurb: "Deep black · minimal accents",
    tokens: {
      scheme: "dark",
      bg: "#000000",
      bgElev: "#030303",
      surface: "#070707",
      surfaceAlt: "#0d0d0d",
      border: "#161616",
      borderStrong: "#262626",
      text: "#e8e8e8",
      textMuted: "#8a8a8a",
      textFaint: "#565656",
      accent: "#d6d6d6",
      accentSoft: "rgba(214, 214, 214, 0.12)",
      accentInk: "#050505",
      ok: "#8fae8f",
      warn: "#c9b489",
      danger: "#c07f7f",
      info: "#9aa7b4",
      shadow: "rgba(0, 0, 0, 0.65)",
    },
  },
  {
    id: "crt",
    name: "CRT",
    blurb: "Phosphor terminal · scanline discipline",
    tokens: {
      scheme: "dark",
      bg: "#04100a",
      bgElev: "#06150e",
      surface: "#071a11",
      surfaceAlt: "#0b2117",
      border: "#123024",
      borderStrong: "#1d4534",
      text: "#c8f5d8",
      textMuted: "#7fb897",
      textFaint: "#547a63",
      accent: "#4ef08a",
      accentSoft: "rgba(78, 240, 138, 0.16)",
      accentInk: "#03130a",
      ok: "#4ef08a",
      warn: "#e4d06a",
      danger: "#f0786a",
      info: "#6ad3c8",
      shadow: "rgba(0, 0, 0, 0.5)",
    },
  },
  {
    id: "blueprint",
    name: "Blueprint",
    blurb: "Technical navy · drafting cyan",
    tokens: {
      scheme: "dark",
      bg: "#071726",
      bgElev: "#0a1e30",
      surface: "#0c2438",
      surfaceAlt: "#112c43",
      border: "#1b3d57",
      borderStrong: "#2a5677",
      text: "#dbeaf7",
      textMuted: "#93aec4",
      textFaint: "#62809a",
      accent: "#4bc4e0",
      accentSoft: "rgba(75, 196, 224, 0.16)",
      accentInk: "#04141d",
      ok: "#63c7a2",
      warn: "#dcc077",
      danger: "#d9756a",
      info: "#4bc4e0",
      shadow: "rgba(0, 10, 20, 0.55)",
    },
  },
  {
    id: "forest",
    name: "Forest",
    blurb: "Muted green · earthy neutrals",
    tokens: {
      scheme: "dark",
      bg: "#0c110e",
      bgElev: "#101611",
      surface: "#141b16",
      surfaceAlt: "#1a231c",
      border: "#26322a",
      borderStrong: "#37483b",
      text: "#e0e7df",
      textMuted: "#9aa89b",
      textFaint: "#6b7a6c",
      accent: "#8fbc74",
      accentSoft: "rgba(143, 188, 116, 0.15)",
      accentInk: "#0b1408",
      ok: "#8fbc74",
      warn: "#cbab72",
      danger: "#c47c6b",
      info: "#8aacb8",
      shadow: "rgba(0, 0, 0, 0.45)",
    },
  },
  {
    id: "minimal",
    name: "Minimal",
    blurb: "Clean monochrome · light surfaces",
    tokens: {
      scheme: "light",
      bg: "#f6f6f4",
      bgElev: "#efefec",
      surface: "#ffffff",
      surfaceAlt: "#f3f3f0",
      border: "#deddd8",
      borderStrong: "#c3c2bc",
      text: "#1b1c1a",
      textMuted: "#5f605c",
      textFaint: "#8b8c87",
      accent: "#1b1c1a",
      accentSoft: "rgba(27, 28, 26, 0.08)",
      accentInk: "#ffffff",
      ok: "#3f7a4b",
      warn: "#9a6b17",
      danger: "#a4463c",
      info: "#3a627f",
      shadow: "rgba(23, 24, 22, 0.12)",
    },
  },
];

export const DEFAULT_THEME_ID = "graft";

export function themeById(id: string): Theme {
  return themes.find((t) => t.id === id) ?? themes[0];
}

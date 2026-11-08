export type ViewId = string;

type UiState = {
  view: ViewId;
  paletteOpen: boolean;
  toast: { text: string; tone: "neutral" | "ok" | "danger" } | null;
};

export const ui = $state<UiState>({
  view: "home",
  paletteOpen: false,
  toast: null,
});

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function navigate(view: ViewId): void {
  ui.view = view;
  ui.paletteOpen = false;
}

export function openPalette(): void {
  ui.paletteOpen = true;
}

export function closePalette(): void {
  ui.paletteOpen = false;
}

export function notify(text: string, tone: "neutral" | "ok" | "danger" = "neutral"): void {
  ui.toast = { text, tone };
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    ui.toast = null;
  }, 3200);
}

"use client";

// The color mode preference. The inline script in app/layout.tsx applies it
// before first paint; this module changes it afterwards.

import { useSyncExternalStore } from "react";

export type ColorMode = "auto" | "light" | "dark";

const KEY = "color-mode";
const listeners = new Set<() => void>();

function read(): ColorMode {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "auto";
  } catch {
    return "auto";
  }
}

export function setColorMode(mode: ColorMode) {
  try {
    if (mode === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, mode);
  } catch {
    // Private windows may refuse storage; the choice then lasts for this page only.
  }
  const apply = (window as unknown as { __applyColorMode?: () => void }).__applyColorMode;
  if (apply) apply();
  else document.documentElement.setAttribute("data-color-mode", mode === "auto" ? "light" : mode);
  listeners.forEach((l) => l());
}

export function useColorMode(): ColorMode {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    read,
    () => "auto",
  );
}

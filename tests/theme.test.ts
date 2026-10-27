import { afterEach, expect, it, vi } from "vitest";
import { applyTheme } from "@/lib/theme";
afterEach(() => vi.unstubAllGlobals());
it("follows system changes and removes its subscription on cleanup", () => {
  let changed: (() => void) | undefined;
  const media = {
    matches: false,
    addEventListener: vi.fn((_event, handler) => {
      changed = handler;
    }),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", () => media);
  const cleanup = applyTheme("system");
  expect(document.documentElement).not.toHaveClass("dark");
  media.matches = true;
  changed?.();
  expect(document.documentElement).toHaveClass("dark");
  cleanup();
  expect(media.removeEventListener).toHaveBeenCalledWith("change", changed);
});
it("an explicit theme does not subscribe to OS changes", () => {
  const media = {
    matches: true,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", () => media);
  applyTheme("light")();
  expect(document.documentElement).not.toHaveClass("dark");
  expect(media.addEventListener).not.toHaveBeenCalled();
});

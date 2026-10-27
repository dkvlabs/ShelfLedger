// Theme handling (Phase 4): light/dark/system via the .dark class; the
// preference is stored in settings as preferences.theme.

export type Theme = "light" | "dark" | "system";

export function applyTheme(theme: Theme) {
  const dark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

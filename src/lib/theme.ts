export type Theme = "light" | "dark" | "system";
/** Return cleanup so system preference follows OS changes without leaking listeners. */
export function applyTheme(theme: Theme) {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const update = () =>
    document.documentElement.classList.toggle(
      "dark",
      theme === "dark" || (theme === "system" && media.matches),
    );
  update();
  if (theme === "system") media.addEventListener("change", update);
  return () => media.removeEventListener("change", update);
}

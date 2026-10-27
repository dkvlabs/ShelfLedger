// Lightweight EN/RO localization (Phase 4). Strings live in one dictionary;
// the language preference is stored in settings as preferences.language.
// Extend dictionaries per screen; missing keys fall back to English.

import romanian from "./ro.json";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export type Language = "en" | "ro";

const dictionaries: Record<Language, Record<string, string>> = {
  en: {
    "app.title": "ShelfLedger",
    "app.tagline": "Local-first library",
    "nav.dashboard": "Dashboard",
    "nav.library": "Library",
    "nav.reading": "Reading",
    "nav.loans": "Lending",
    "nav.sell": "Sell queue",
    "nav.insights": "Finance",
    "nav.settings": "Settings",
    "nav.wishlist": "Wishlist",
    "library.search": "Search title, author, ISBN…",
    "library.add": "Add Book",
    "library.empty.title": "Welcome to ShelfLedger",
    "library.empty.body":
      "Add your first book by ISBN or manually — everything stays on this computer.",
    "library.noMatch": "No books match your search",
    "common.add": "Add",
    "common.cancel": "Cancel",
    "common.save": "Save",
    "common.delete": "Delete",
    "common.loading": "Loading…",
    "common.back": "Back",
    "common.archive": "Archive",
    "common.overdue": "Overdue",
    "common.due": "Due",
    "common.notSet": "Not set",
    "settings.currency": "Preferred currency",
    "settings.language": "Language",
    "settings.theme": "Theme",
    "settings.theme.light": "Light",
    "settings.theme.dark": "Dark",
    "settings.theme.system": "System",
    "settings.locations": "Physical locations",
    "settings.data": "Data & backups",
    "settings.exportCsv": "Export copies (CSV)",
    "settings.exportJson": "Export library (JSON)",
    "settings.backup": "Create backup",
    "settings.restore": "Restore archive",
    "settings.import": "Import",
  },
  ro: {
    "app.title": "ShelfLedger",
    "app.tagline": "Bibliotecă locală",
    "nav.dashboard": "Panou",
    "nav.library": "Biblioteca mea",
    "nav.reading": "Lectură",
    "nav.loans": "Împrumuturi",
    "nav.sell": "Vânzare & evaluare",
    "nav.insights": "Statistici",
    "nav.settings": "Setări",
    "nav.wishlist": "Listă de dorințe",
    "library.search": "Caută titlu, autor, ISBN…",
    "library.add": "Adaugă carte",
    "library.empty.title": "Bun venit la ShelfLedger",
    "library.empty.body":
      "Adaugă prima carte după ISBN sau manual — totul rămâne pe acest calculator.",
    "library.noMatch": "Nicio carte nu corespunde căutării",
    "common.add": "Adaugă",
    "common.cancel": "Anulează",
    "common.save": "Salvează",
    "common.delete": "Șterge",
    "common.loading": "Se încarcă…",
    "common.back": "Înapoi",
    "common.archive": "Arhivează",
    "common.overdue": "Întârziat",
    "common.due": "Termen",
    "common.notSet": "Nesetat",
    "settings.currency": "Moneda preferată",
    "settings.language": "Limba",
    "settings.theme": "Temă",
    "settings.theme.light": "Deschisă",
    "settings.theme.dark": "Întunecată",
    "settings.theme.system": "Sistem",
    "settings.locations": "Locații fizice",
    "settings.data": "Date & copii de rezervă",
    "settings.exportCsv": "Exportă exemplare (CSV)",
    "settings.exportJson": "Exportă biblioteca (JSON)",
    "settings.backup": "Creează copie de rezervă",
    "settings.restore": "Restaurează arhiva",
    "settings.import": "Importă",
  },
};

export function translate(language: Language, key: string): string {
  if (language === "ro") {
    const patterns: [RegExp, (...values: string[]) => string][] = [
      [/^CSV written to (.+)$/, (path) => `Fișier CSV salvat în ${path}`],
      [/^JSON written to (.+)$/, (path) => `Fișier JSON salvat în ${path}`],
      [
        /^Backup written to (.+)$/,
        (path) => `Copie de rezervă salvată în ${path}`,
      ],
      [/^Restored library · (.+)$/, (id) => `Bibliotecă restaurată · ${id}`],
      [
        /^Library restored\. Previous library saved at (.+)\.$/,
        (path) =>
          `Bibliotecă restaurată. Biblioteca anterioară a fost salvată în ${path}.`,
      ],
      [
        /^Restored a separate library\. Open it in Settings → Libraries\. Saved at (.+)\.$/,
        (path) =>
          `Bibliotecă restaurată separat. Deschide-o din Setări → Biblioteci. Salvată în ${path}.`,
      ],
      [
        /^(\d+) copies of this work are owned$/,
        (n) => `Deții ${n} exemplare ale acestei opere`,
      ],
      [
        /^Not finished in over (\d+) year\(s\)$/,
        (n) => `Neterminată de peste ${n} ani`,
      ],
      [
        /^Personal rating is only (\d)\/5$/,
        (n) => `Evaluare personală de doar ${n}/5`,
      ],
      [/^Sentimental value (\d)\/5$/, (n) => `Valoare sentimentală ${n}/5`],
      [/^Rated (\d)\/5$/, (n) => `Evaluată cu ${n}/5`],
      [
        /^Condition is (.+)$/,
        (value) => `Stare: ${value === "poor" ? "slabă" : "acceptabilă"}`,
      ],
    ];
    for (const [pattern, format] of patterns) {
      const match = key.match(pattern);
      if (match) return format(...match.slice(1));
    }
  }
  return (
    (language === "ro"
      ? (romanian as Record<string, string>)[key]
      : undefined) ??
    dictionaries[language][key] ??
    dictionaries.en[key] ??
    key
  );
}

export const LANGUAGES: { value: Language; label: string }[] = [
  { value: "en", label: "English" },
  { value: "ro", label: "Română" },
];

/** Read the preferred language from settings (default English). */
export function useLanguage(): Language {
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });
  const lang = (settings?.["preferences"] as { language?: string } | undefined)
    ?.language;
  return lang === "ro" ? "ro" : "en";
}

export function useT() {
  const language = useLanguage();
  return (key: string) => translate(language, key);
}

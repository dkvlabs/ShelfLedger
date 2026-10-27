// Lightweight EN/RO localization (Phase 4). Strings live in one dictionary;
// the language preference is stored in settings as preferences.language.
// Extend dictionaries per screen; missing keys fall back to English.

import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export type Language = "en" | "ro";

const dictionaries: Record<Language, Record<string, string>> = {
  en: {
    "app.title": "ShelfLedger",
    "app.tagline": "Local-first library",
    "nav.dashboard": "Dashboard",
    "nav.library": "My Library",
    "nav.reading": "Reading",
    "nav.loans": "Loans",
    "nav.sell": "Sell & Value",
    "nav.insights": "Insights",
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
  return dictionaries[language][key] ?? dictionaries.en[key] ?? key;
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
  const lang = (settings?.["preferences"] as { language?: string } | undefined)?.language;
  return lang === "ro" ? "ro" : "en";
}

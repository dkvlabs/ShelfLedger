// All bundles ship locally: switching language never needs the network.
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { formatMoney } from "./format";
import languageManifest from "./locales/languages.json";
import aliases from "./locales/aliases.json";
import en from "./locales/en.json";
import ro from "./locales/ro.json";
import zh from "./locales/zh.json";
import hi from "./locales/hi.json";
import es from "./locales/es.json";
import ar from "./locales/ar.json";
import fr from "./locales/fr.json";
import bn from "./locales/bn.json";
import pt from "./locales/pt.json";
import id from "./locales/id.json";
import ur from "./locales/ur.json";

export const dictionaries = { en, zh, hi, es, ar, fr, bn, pt, id, ur, ro };
export type Language = keyof typeof dictionaries;
export const LANGUAGES = Object.entries(languageManifest).map(([value, info]) => ({
  value: value as Language, ...info, direction: info.direction as "ltr" | "rtl",
}));
export function resolveLanguage(value: unknown): Language {
  return typeof value === "string" && Object.hasOwn(dictionaries, value)
    ? value as Language : "en";
}
export function languageInfo(language: Language) {
  return LANGUAGES.find((entry) => entry.value === language)!;
}
// Match product-owned sentences; user titles, paths and metadata stay intact.
const messagePatterns: [RegExp, string][] = [
  [/^Read “(.+)” — that barcode is not a book ISBN. Try again, or type the ISBN manually\.$/, "Read “{value}” — that barcode is not a book ISBN. Try again, or type the ISBN manually."],
  [/^Read “(.+)” — that barcode is not a book ISBN. Try a photo of the ISBN barcode instead\.$/, "Read “{value}” — that barcode is not a book ISBN. Try a photo of the ISBN barcode instead."],
  [/^CSV written to (.+)$/, "CSV written to {value}"],
  [/^JSON written to (.+)$/, "JSON written to {value}"],
  [/^Backup written to (.+)$/, "Backup written to {value}"],
  [/^Restored library · (.+)$/, "Restored library · {value}"],
  [/^Library restored\. Previous library saved at (.+)\.$/, "Library restored. Previous library saved at {value}."],
  [/^Restored a separate library\. Open it in Settings → Libraries\. Saved at (.+)\.$/, "Restored a separate library. Open it in Settings → Libraries. Saved at {value}."],
  [/^(\d+) copies of this work are owned$/, "{value} copies of this work are owned"],
  [/^Not finished in over (\d+) year\(s\)$/, "Not finished in over {value} year(s)"],
  [/^Personal rating is only (\d)\/5$/, "Personal rating is only {value}/5"],
  [/^Sentimental value (\d)\/5$/, "Sentimental value {value}/5"],
  [/^Rated (\d)\/5$/, "Rated {value}/5"],
  [/^Condition is (poor|acceptable)$/, "Condition is {value}"],
];
export function translate(language: Language, key: string, values?: { value: string }): string {
  const canonical = Object.hasOwn(aliases, key) ? (aliases as Record<string, string>)[key] : key;
  const dictionary = dictionaries[resolveLanguage(language)] as Record<string, string>;
  const exact = Object.hasOwn(dictionary, canonical) ? dictionary[canonical] : undefined;
  if (exact !== undefined) return values ? exact.replace("{value}", () => values.value) : exact;
  for (const [pattern, template] of messagePatterns) {
    const match = canonical.match(pattern);
    if (match) {
      const value = template === "Condition is {value}"
        ? translate(language, match[1] === "poor" ? "Poor" : "Acceptable") : match[1];
      // A callback preserves literal $ sequences in filenames.
      return (dictionary[template] ?? template).replace("{value}", () => value);
    }
  }
  return Object.hasOwn(en, canonical) ? (en as Record<string, string>)[canonical] : canonical;
}
export function useLanguage(): Language {
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  return resolveLanguage((settings?.preferences as { language?: unknown } | undefined)?.language);
}
export function useT() {
  const language = useLanguage();
  return (key: string, values?: { value: string }) => translate(language, key, values);
}

export function useFormatMoney() {
  const language = useLanguage();
  return (minor: number | null | undefined, currency: string | null | undefined) =>
    formatMoney(minor, currency, languageInfo(language).locale);
}

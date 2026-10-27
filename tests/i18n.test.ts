import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import ts from "typescript";
import { dictionaries, LANGUAGES, languageInfo, resolveLanguage, translate } from "@/lib/i18n";
import aliases from "@/lib/locales/aliases.json";
import { ACQUISITION_LABELS, CONDITION_LABELS, LOCATION_LABELS, OWNERSHIP_LABELS, READING_STATUS_LABELS } from "@/types/domain";

describe("offline language bundles", () => {
  it("ships the ten selected languages and preserves Romanian", () => {
    expect(LANGUAGES.map((l) => l.value)).toEqual(["en", "zh", "hi", "es", "ar", "fr", "bn", "pt", "id", "ur", "ro"]);
    expect(new Set(LANGUAGES.map((l) => l.value)).size).toBe(Object.keys(dictionaries).length);
  });
  it.each(LANGUAGES)("has complete keys and interpolation for $label", ({ value }) => {
    const dictionary = dictionaries[value];
    expect(Object.keys(dictionary).sort()).toEqual(Object.keys(dictionaries.en).sort());
    for (const key of Object.keys(dictionaries.en) as (keyof typeof dictionaries.en)[]) {
      expect(dictionary[key].trim(), `${value}: ${key}`).not.toBe("");
      expect(dictionary[key].match(/\{\w+\}/g) ?? [], `${value}: ${key}`).toEqual(key.match(/\{\w+\}/g) ?? []);
    }
    for (const alias of Object.keys(aliases)) {
      expect(translate(value, alias)).toBe(dictionary[aliases[alias as keyof typeof aliases] as keyof typeof dictionary]);
    }
  });
  it("requires translations for literal UI calls and domain labels", () => {
    const missing = new Set<string>();
    function walk(directory: string) {
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const file = resolve(directory, entry.name);
        if (entry.isDirectory()) { walk(file); continue; }
        if (!/\.tsx?$/.test(file)) continue;
        const tree = ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true);
        function visit(node: ts.Node) {
          if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && ["t", "tx"].includes(node.expression.text)) {
            const key = node.arguments[0];
            if (key && ts.isStringLiteral(key) && !Object.hasOwn(dictionaries.en, key.text) && !Object.hasOwn(aliases, key.text)) missing.add(key.text);
          }
          ts.forEachChild(node, visit);
        }
        visit(tree);
      }
    }
    walk(resolve("src"));
    for (const key of Object.values({ ...ACQUISITION_LABELS, ...CONDITION_LABELS, ...LOCATION_LABELS, ...OWNERSHIP_LABELS, ...READING_STATUS_LABELS })) {
      if (!Object.hasOwn(dictionaries.en, key)) missing.add(key);
    }
    expect([...missing]).toEqual([]);
  });
  it("falls back safely for unknown preferences, keys and provider data", () => {
    expect(resolveLanguage("future-language")).toBe("en");
    expect(resolveLanguage("__proto__")).toBe("en");
    expect(resolveLanguage(null)).toBe("en");
    for (const key of ["__proto__", "constructor", "toString"]) expect(translate("fr", key)).toBe(key);
    expect(translate("fr", "A title supplied by its owner")).toBe("A title supplied by its owner");
    expect(languageInfo("ar").direction).toBe("rtl");
    expect(languageInfo("ur").direction).toBe("rtl");
    expect(languageInfo("zh").locale).toBe("zh-Hans");
    expect(languageInfo("en").direction).toBe("ltr");
  });
  it.each(LANGUAGES)("preserves paths, identifiers and raw values in $label", ({ value }) => {
    const path = "/app-data/أرشيف/$&/book.json";
    expect(translate(value, `JSON written to ${path}`)).toContain(path);
    expect(translate(value, "Delete {value}", { value: "$& 藏书" })).toContain("$& 藏书");
    expect(translate(value, "3 copies of this work are owned")).toContain("3");
    expect(translate(value, "Rated 4/5")).toContain("4/5");
    expect(translate(value, "Condition is poor")).toContain(translate(value, "Poor"));
  });
});

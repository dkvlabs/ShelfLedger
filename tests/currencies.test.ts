import { expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import digits from "@/lib/currencies.json";
import catalog from "@/lib/currency-catalog.json";
import { currencies, currencyName, formatMoney, minorDigits, minorToDecimal, parseMoneyToMinor } from "@/lib/format";

it("matches every currency and fund with defined minor units in the official ISO snapshot", () => {
  const xml = new DOMParser().parseFromString(readFileSync("docs/data/iso-4217-list-one.xml", "utf8"), "application/xml");
  const official: Record<string, number> = {};
  for (const entry of xml.querySelectorAll("CcyNtry")) {
    const code = entry.querySelector("Ccy")?.textContent;
    const scale = entry.querySelector("CcyMnrUnts")?.textContent;
    if (code && scale && /^\d+$/.test(scale)) official[code] = Number(scale);
  }
  expect(currencies.sort()).toEqual(Object.keys(official).sort());
  expect(catalog.published).toBe(xml.documentElement.getAttribute("Pblshd"));
  for (const [code, scale] of Object.entries(official)) expect(minorDigits(code), code).toBe(scale);
  expect(minorDigits("XAU")).toBeNull(); // ISO specifies N.A., not a decimal scale.
  expect(minorDigits("XXX")).toBeNull();
  expect(minorDigits("XYZ")).toBeNull();
});
it.each(Object.keys(digits))("round-trips safe amounts exactly for %s", (code) => {
  for (const amount of [0, 1, 12345, Number.MAX_SAFE_INTEGER]) {
    expect(parseMoneyToMinor(minorToDecimal(amount, code), code)).toBe(amount);
    expect(formatMoney(amount, code)).not.toBe("—");
  }
});
it("uses ISO book-money precision rather than cash rounding", () => {
  expect(parseMoneyToMinor("12.34", "PKR")).toBe(1234);
  expect(parseMoneyToMinor("12.34", "AFN")).toBe(1234);
  expect(parseMoneyToMinor("12.345", "IQD")).toBe(12345);
  expect(parseMoneyToMinor("1.2345", "CLF")).toBe(12345);
  expect(parseMoneyToMinor("1.2345", "UYW")).toBe(12345);
  expect(parseMoneyToMinor("1.01", "XAD")).toBe(101);
});
it.each([
  ["١٬٢٣٤٫٥٦", "AED", 123456],
  ["۱۲۳۴٫۵۶", "PKR", 123456],
  ["१२३४.५६", "INR", 123456],
  ["১২৩৪.৫৬", "BDT", 123456],
  ["１２３４.５６", "CNY", 123456],
] as const)("accepts localized digits in %s", (input, currency, expected) => {
  expect(parseMoneyToMinor(input, currency)).toBe(expected);
});
it("localizes names and separators while preserving stored precision", () => {
  expect(currencyName("EUR", "fr")).toMatch(/euro/i);
  expect(currencyName("CLF", "en")).not.toBe("CLF");
  expect(formatMoney(1234, "EUR", "fr")).toContain("12,34");
  expect(formatMoney(1234, "AED", "ar-u-nu-arab")).toContain("١٢٫٣٤");
  expect(formatMoney(1234, "AED", "ar")).toBe(new Intl.NumberFormat("ar", { style: "currency", currency: "AED" }).format(12.34));
  expect(formatMoney(12345, "CLF", "es")).toContain("1,2345");
});

it("uses official names when an older runtime only knows the code", () => {
  const lookup = vi.spyOn(Intl.DisplayNames.prototype, "of").mockReturnValue("CLF");
  try { expect(currencyName("CLF", "en")).toBe(catalog.current.CLF); }
  finally { lookup.mockRestore(); }
});

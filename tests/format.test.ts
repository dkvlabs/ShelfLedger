import { describe, expect, it } from "vitest";

import { formatDate, formatMoney, parseMoneyToMinor, toInputDate } from "@/lib/format";

describe("formatMoney", () => {
  it("formats 2-decimal currencies in minor units", () => {
    expect(formatMoney(1250, "RON")).toContain("12.50");
    expect(formatMoney(1250, "EUR")).toContain("12.50");
  });

  it("shows a dash for unknown money", () => {
    expect(formatMoney(null, "RON")).toBe("—");
    expect(formatMoney(1250, null)).toBe("—");
  });

  it("falls back to code suffix for unknown currency codes", () => {
    expect(formatMoney(5, "XYZ")).toContain("XYZ");
  });
});

describe("parseMoneyToMinor", () => {
  it("parses dot and comma decimals", () => {
    expect(parseMoneyToMinor("12.50")).toBe(1250);
    expect(parseMoneyToMinor("12,50")).toBe(1250);
    expect(parseMoneyToMinor("0.05")).toBe(5);
    expect(parseMoneyToMinor("5")).toBe(500);
  });

  it("handles thousands separators", () => {
    expect(parseMoneyToMinor("1 234,50")).toBe(123450);
    expect(parseMoneyToMinor("1,234.50")).toBe(123450);
    expect(parseMoneyToMinor("1.234,50")).toBe(123450);
  });

  it("rejects garbage", () => {
    expect(parseMoneyToMinor("")).toBeNull();
    expect(parseMoneyToMinor("abc")).toBeNull();
    expect(parseMoneyToMinor("12.345")).toBeNull();
  });
});

describe("formatDate", () => {
  it("renders DD.MM.YYYY from ISO dates", () => {
    expect(formatDate("2026-10-23")).toBe("23.10.2026");
    expect(formatDate("2026-10-23T18:00:00Z")).toBe("23.10.2026");
  });

  it("renders a dash for missing dates", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate(undefined)).toBe("—");
  });
});

describe("toInputDate", () => {
  it("converts DD.MM.YYYY to ISO", () => {
    expect(toInputDate("23.10.2026")).toBe("2026-10-23");
  });

  it("leaves other strings untouched", () => {
    expect(toInputDate("2026-10-23")).toBe("2026-10-23");
    expect(toInputDate("nope")).toBe("nope");
  });
});

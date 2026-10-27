import { describe, expect, it } from "vitest";

import {
  formatDate,
  formatMoney,
  minorToDecimal,
  parseMoneyToMinor,
  toInputDate,
} from "@/lib/format";

describe("formatMoney", () => {
  it("formats 2-decimal currencies in minor units", () => {
    expect(formatMoney(1250, "RON")).toContain("12.50");
    expect(formatMoney(1250, "EUR")).toContain("12.50");
  });

  it("shows a dash for unknown money", () => {
    expect(formatMoney(null, "RON")).toBe("—");
    expect(formatMoney(1250, null)).toBe("—");
  });

  it("does not invent a scale for unknown currency codes", () => {
    expect(formatMoney(5, "XYZ")).toBe("—");
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

describe("currency scales and exact integer arithmetic", () => {
  it.each([
    ["CAD", "12.34", 1234],
    ["CHF", "0.05", 5],
    ["JPY", "120", 120],
    ["KWD", "1.234", 1234],
  ])("parses %s using its minor units", (code, amount, expected) => {
    expect(parseMoneyToMinor(amount, code)).toBe(expected);
  });
  it("rejects precision loss and unsafe amounts", () => {
    expect(parseMoneyToMinor("1.01", "JPY")).toBeNull();
    expect(parseMoneyToMinor("90071992547409.92", "CAD")).toBeNull();
    expect(parseMoneyToMinor("5", "XYZ")).toBeNull();
    expect(parseMoneyToMinor("90071992547409.91", "CAD")).toBe(
      Number.MAX_SAFE_INTEGER,
    );
  });
  it("displays zero and three digit currencies correctly", () => {
    expect(formatMoney(1234, "JPY")).not.toContain(".34");
    expect(formatMoney(1234, "KWD")).toContain("1.234");
  });
});

it("keeps the last minor unit when displaying and re-editing the largest safe amount", () => {
  expect(minorToDecimal(Number.MAX_SAFE_INTEGER, "CAD")).toBe(
    "90071992547409.91",
  );
  expect(
    parseMoneyToMinor(minorToDecimal(Number.MAX_SAFE_INTEGER, "KWD"), "KWD"),
  ).toBe(Number.MAX_SAFE_INTEGER);
  expect(formatMoney(Number.MAX_SAFE_INTEGER, "CAD")).toContain(
    "90,071,992,547,409.91",
  );
});
it("formats negative fractional amounts without losing their sign", () => {
  expect(minorToDecimal(-5, "CAD")).toBe("-0.05");
  expect(formatMoney(-5, "CAD")).toContain("-CA$0.05");
});

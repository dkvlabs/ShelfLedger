import { afterEach, describe, expect, it, vi } from "vitest";
import { olxSearch, openOlx, simulateNet, validOlxUrl } from "@/lib/market";

afterEach(() => vi.restoreAllMocks());
describe("OLX link safety and exact price simulation", () => {
  it("builds encoded book searches from ISBNs and Unicode titles", () => {
    expect(olxSearch("9780306406157")).toBe(
      "https://www.olx.ro/hobby-sport-turism/carti-muzica-filme/carti/q-9780306406157/",
    );
    const url = new URL(olxSearch(" Cărți / autor? "));
    expect(url.hostname).toBe("www.olx.ro");
    expect(url.search).toBe("");
    expect(url.pathname).toContain("C%C4%83r%C8%9Bi-%2F-autor%3F");
    expect(validOlxUrl(url.href, false)).toBe(true);
    expect(validOlxUrl(url.href)).toBe(false);
  });
  it.each([
    "javascript:alert(1)",
    "https://evil.test/d/oferta/abc-ID123.html",
    "https://olx.ro.evil.test/d/oferta/abc-ID123.html",
    "http://olx.ro/d/oferta/abc-ID123.html",
    "https://user@olx.ro/d/oferta/abc-ID123.html",
    "https://olx.ro:444/d/oferta/abc-ID123.html",
  ])("rejects %s", async (value) => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    expect(validOlxUrl(value)).toBe(false);
    await expect(openOlx(value)).rejects.toThrow();
    expect(open).not.toHaveBeenCalled();
  });
  it("opens only on explicit invocation and strips tracking", async () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    await openOlx("https://olx.ro/d/oferta/book-IDtest.html?tracking=1#x");
    expect(open).toHaveBeenCalledExactlyOnceWith(
      "https://www.olx.ro/d/oferta/book-IDtest.html",
      "_blank",
      "noopener,noreferrer",
    );
  });
  it.each([
    ["40", "2.50", "5", "RON", 3250],
    ["40", "", "", "RON", 4000],
    ["0", "2", "3", "RON", -500],
    ["40", "bad", "", "RON", null],
    ["40", "-1", "", "RON", null],
    ["40", "", "0.001", "RON", null],
    ["100", "1", "2", "JPY", 97],
    ["1.234", "0.001", "", "KWD", 1233],
    ["", "", "", "RON", null],
    ["1", "9007199254740991", "9007199254740991", "JPY", null],
    ["٩٫٥٠", "١", "", "RON", 850],
  ])(
    "simulates %s minus %s and %s in %s",
    (asking, fee, shipping, currency, expected) => {
      expect(
        simulateNet(
          asking as string,
          fee as string,
          shipping as string,
          currency as string,
        ),
      ).toBe(expected);
    },
  );
});

import { describe, expect, it } from "vitest";

import {
  isbn10ToIsbn13,
  isbn13ToIsbn10,
  isValidIsbn10,
  isValidIsbn13,
  normalizeIsbn,
  parseIsbn,
} from "@/lib/isbn";

describe("normalizeIsbn", () => {
  it("strips separators and uppercases", () => {
    expect(normalizeIsbn(" 978-0-306-40615-7 ")).toBe("9780306406157");
    expect(normalizeIsbn("0-8044-2957-x")).toBe("080442957X");
  });
});

describe("isValidIsbn13", () => {
  it("validates checksums", () => {
    expect(isValidIsbn13("9780306406157")).toBe(true);
    expect(isValidIsbn13("9780306406158")).toBe(false);
    expect(isValidIsbn13("97803064061")).toBe(false);
    expect(isValidIsbn13("abcdefghijklm")).toBe(false);
  });
});

describe("isValidIsbn10", () => {
  it("validates checksums including X", () => {
    expect(isValidIsbn10("0306406152")).toBe(true);
    expect(isValidIsbn10("080442957X")).toBe(true);
    expect(isValidIsbn10("0306406153")).toBe(false);
    expect(isValidIsbn10("12345")).toBe(false);
  });
});

describe("parseIsbn", () => {
  it("accepts a valid ISBN-13 and derives ISBN-10", () => {
    expect(parseIsbn("978-0-306-40615-7")).toEqual({
      isbn13: "9780306406157",
      isbn10: "0306406152",
    });
  });

  it("accepts a valid ISBN-10 and derives ISBN-13", () => {
    expect(parseIsbn("0-19-852663-6")).toEqual({
      isbn13: "9780198526636",
      isbn10: "0198526636",
    });
  });

  it("rejects invalid input", () => {
    expect(parseIsbn("hello")).toBeNull();
    expect(parseIsbn("9780306406158")).toBeNull();
    expect(parseIsbn("")).toBeNull();
  });
});

describe("conversions", () => {
  it("round-trips 10 <-> 13", () => {
    expect(isbn10ToIsbn13("0306406152")).toBe("9780306406157");
    expect(isbn13ToIsbn10("9780306406157")).toBe("0306406152");
    expect(isbn13ToIsbn10("9791234567896")).toBeNull(); // only 978 maps back
    expect(isbn10ToIsbn13("030640615X")).toBeNull(); // non-numeric body
  });
});

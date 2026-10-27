import { describe, expect, it } from "vitest";

import { extractIsbnFromBarcode } from "@/lib/barcode";

describe("extractIsbnFromBarcode", () => {
  it("accepts a plain EAN-13 book barcode", () => {
    expect(extractIsbnFromBarcode("9780306406157")).toEqual({
      isbn13: "9780306406157",
      isbn10: "0306406152",
    });
  });

  it("accepts hyphenated and wrapper-noisy values", () => {
    const expected = { isbn13: "9780306406157", isbn10: "0306406152" };
    expect(extractIsbnFromBarcode("978-0-306-40615-7")).toEqual(expected);
    expect(extractIsbnFromBarcode('="9780306406157"')).toEqual(expected);
  });

  it("strips a glued EAN-5 add-on (price extensions)", () => {
    expect(extractIsbnFromBarcode("978030640615712345")).toEqual({
      isbn13: "9780306406157",
      isbn10: "0306406152",
    });
  });

  it("accepts ISBN-10 including X check digits", () => {
    expect(extractIsbnFromBarcode("0306406152")).toEqual({
      isbn13: "9780306406157",
      isbn10: "0306406152",
    });
    expect(extractIsbnFromBarcode("080442957X")).toEqual({
      isbn13: "9780804429573",
      isbn10: "080442957X",
    });
  });

  it("rejects non-book EANs even with a valid checksum", () => {
    // Valid EAN-13 checksum, but not a 978/979 book range.
    expect(extractIsbnFromBarcode("5901234123457")).toBeNull();
  });

  it("rejects bad checksums and garbage", () => {
    expect(extractIsbnFromBarcode("9780306406158")).toBeNull();
    expect(extractIsbnFromBarcode("hello world")).toBeNull();
    expect(extractIsbnFromBarcode("")).toBeNull();
    expect(extractIsbnFromBarcode("123")).toBeNull();
  });
});

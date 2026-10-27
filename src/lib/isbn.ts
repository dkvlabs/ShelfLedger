// Client-side ISBN validation for instant UX feedback. The Rust backend
// re-validates checksums authoritatively before any write (AGENTS.md).

/** Keep only [0-9A-Z], uppercasing lowercase input. */
export function normalizeIsbn(raw: string): string {
  return raw
    .split("")
    .filter((c) => /[a-zA-Z0-9]/.test(c))
    .join("")
    .toUpperCase();
}

export function isValidIsbn13(s: string): boolean {
  if (!/^\d{13}$/.test(s)) return false;
  const digits = s.split("").map(Number);
  const sum = digits.reduce((acc, d, i) => acc + d * (i % 2 === 0 ? 1 : 3), 0);
  return sum % 10 === 0;
}

export function isValidIsbn10(s: string): boolean {
  if (!/^[0-9]{9}[0-9X]$/.test(s)) return false;
  const values = s.split("").map((c, i) => (i === 9 && c === "X" ? 10 : Number(c)));
  const sum = values.reduce((acc, v, i) => acc + v * (10 - i), 0);
  return sum % 11 === 0;
}

export interface NormalizedIsbn {
  isbn13: string | null;
  isbn10: string | null;
}

/** Parse raw input; returns null when neither checksum is valid. */
export function parseIsbn(raw: string): NormalizedIsbn | null {
  const s = normalizeIsbn(raw);
  if (s.length === 13 && isValidIsbn13(s)) {
    return { isbn13: s, isbn10: isbn13ToIsbn10(s) };
  }
  if (s.length === 10 && isValidIsbn10(s)) {
    return { isbn13: isbn10ToIsbn13(s), isbn10: s };
  }
  return null;
}

export function isbn10ToIsbn13(isbn10: string): string | null {
  // The 10th digit can be X (value 10); only the first 9 are used for the
  // conversion, the ISBN-13 check digit is recomputed below.
  if (!/^[0-9]{9}[0-9X]$/.test(isbn10)) return null;
  const first12 = [9, 7, 8, ...isbn10.split("").slice(0, 9).map(Number)];
  const sum = first12.reduce((acc, d, i) => acc + d * (i % 2 === 0 ? 1 : 3), 0);
  const check = (10 - (sum % 10)) % 10;
  return `${first12.join("")}${check}`;
}

export function isbn13ToIsbn10(isbn13: string): string | null {
  if (!/^978\d{10}$/.test(isbn13)) return null;
  const values = isbn13
    .split("")
    .slice(3, 12)
    .map(Number);
  const sum = values.reduce((acc, v, i) => acc + v * (10 - i), 0);
  const check = (11 - (sum % 11)) % 11;
  return `${values.join("")}${check === 10 ? "X" : check}`;
}

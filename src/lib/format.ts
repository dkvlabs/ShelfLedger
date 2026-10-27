// Currency and date display policies (docs/design.md):
// - Money: integer minor units + ISO 4217 code; localized separators;
//   currencies are never summed across codes.
// - Dates: DD.MM.YYYY in the UI, ISO 8601 in storage.

import currencyDigits from "./currencies.json";
export const currencies = Object.keys(currencyDigits);
export function minorDigits(code: string): number | null {
  return (currencyDigits as Record<string, number>)[code.toUpperCase()] ?? null;
}

/** Decimal text for form fields, without converting stored integers to floats. */
export function minorToDecimal(minor: number, currency: string): string {
  const digits = minorDigits(currency);
  if (digits === null || !Number.isSafeInteger(minor)) return "";
  const amount = BigInt(minor);
  const absolute = amount < 0n ? -amount : amount;
  const scale = 10n ** BigInt(digits);
  const fraction = digits
    ? "." + String(absolute % scale).padStart(digits, "0")
    : "";
  return (amount < 0n ? "-" : "") + String(absolute / scale) + fraction;
}

/** Format integer minor units with their currency code. */
export function formatMoney(
  minor: number | null | undefined,
  currency: string | null | undefined,
): string {
  if (minor === null || minor === undefined || !currency) return "—";
  const decimals = minorDigits(currency);
  if (decimals === null || !Number.isSafeInteger(minor)) return "—";
  const absolute = BigInt(minor < 0 ? -minor : minor);
  const scale = 10n ** BigInt(decimals);
  const whole = absolute / scale;
  const signedWhole = minor < 0 ? (whole === 0n ? -0 : -whole) : whole;
  try {
    const formatter = new Intl.NumberFormat(undefined, {
      style: "currency",
      currency: currency.toUpperCase(),
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
    const fraction = decimals
      ? new Intl.NumberFormat(undefined, {
          useGrouping: false,
          minimumIntegerDigits: decimals,
          maximumFractionDigits: 0,
        }).format(absolute % scale)
      : "";
    return formatter
      .formatToParts(signedWhole)
      .map((part) => (part.type === "fraction" ? fraction : part.value))
      .join("");
  } catch {
    return `${minorToDecimal(minor, currency)} ${currency.toUpperCase()}`;
  }
}

/**
 * Convert a user-entered decimal string ("12.50", "12,50", "1 234,50") to
 * integer minor units for the selected currency. Returns null when unparseable.
 */
export function parseMoneyToMinor(
  input: string,
  currency = "RON",
): number | null {
  const decimals = minorDigits(currency);
  if (decimals === null) return null;
  const cleaned = input.replace(/[\s'’\u00a0]/g, "");
  if (cleaned === "" || cleaned === "-" || cleaned === "." || cleaned === ",") {
    return null;
  }
  let normalized: string;
  if (cleaned.includes(",") && cleaned.includes(".")) {
    normalized =
      cleaned.lastIndexOf(",") > cleaned.lastIndexOf(".")
        ? cleaned.replace(/\./g, "").replace(",", ".")
        : cleaned.replace(/,/g, "");
  } else {
    normalized = cleaned.replace(",", ".");
  }
  if (!/^-?\d*(\.\d*)?$/.test(normalized)) return null;
  const negative = normalized.startsWith("-");
  const [whole, fraction = ""] = normalized.replace(/^-/, "").split(".");
  if ((!whole && !fraction) || fraction.length > decimals) return null;
  const amount =
    BigInt(whole || "0") * 10n ** BigInt(decimals) +
    BigInt(fraction.padEnd(decimals, "0") || "0");
  const result = Number(negative ? -amount : amount);
  return Number.isSafeInteger(result) ? result : null;
}

/** ISO 8601 (or plain YYYY-MM-DD) -> DD.MM.YYYY for display. */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const datePart = iso.slice(0, 10);
  const [y, m, d] = datePart.split("-");
  if (!y || !m || !d) return iso;
  return `${d}.${m}.${y}`;
}

/** DD.MM.YYYY display -> YYYY-MM-DD for date inputs; other input untouched. */
export function toInputDate(display: string): string {
  const parts = display.trim().match(/^(\d{2})\.(\d{2})\.(\d{4})$/);
  if (parts) return `${parts[3]}-${parts[2]}-${parts[1]}`;
  return display;
}

/** Today as YYYY-MM-DD, for date-input defaults. */
export function todayIso(): string {
  const now = new Date();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${m}-${d}`;
}

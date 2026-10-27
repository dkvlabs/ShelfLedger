// Currency and date display policies (docs/design.md):
// - Money: integer minor units + ISO 4217 code; localized separators;
//   currencies are never summed across codes.
// - Dates: DD.MM.YYYY in the UI, ISO 8601 in storage.

const TWO_DECIMAL_CURRENCIES = new Set(["RON", "EUR", "USD", "GBP"]);

/** Format integer minor units with their currency code. */
export function formatMoney(
  minor: number | null | undefined,
  currency: string | null | undefined,
): string {
  if (minor === null || minor === undefined || !currency) return "—";
  const decimals = TWO_DECIMAL_CURRENCIES.has(currency.toUpperCase()) ? 2 : 0;
  const value = minor / 10 ** decimals;
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency: currency.toUpperCase(),
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    }).format(value);
  } catch {
    return `${value.toFixed(decimals)} ${currency.toUpperCase()}`;
  }
}

/**
 * Convert a user-entered decimal string ("12.50", "12,50", "1 234,50") to
 * integer minor units for a 2-decimal currency. Returns null when unparseable.
 */
export function parseMoneyToMinor(input: string): number | null {
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
  if (!/^-?\d*(\.\d{0,2})?$/.test(normalized)) return null;
  const value = Number(normalized);
  if (!Number.isFinite(value)) return null;
  return Math.round(value * 100);
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

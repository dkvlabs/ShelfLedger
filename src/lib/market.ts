import { call, isDesktop } from "./api";
import { parseMoneyToMinor } from "./format";

const BOOKS = "/hobby-sport-turism/carti-muzica-filme/carti/";
export function olxSearch(query: string): string {
  const slug = encodeURIComponent(query.trim().replace(/\s+/g, "-")).replace(
    /\./g,
    "%2E",
  );
  return `https://www.olx.ro${BOOKS}${slug ? `q-${slug}/` : ""}`;
}
export function validOlxUrl(value: string, listingOnly = true): boolean {
  try {
    const url = new URL(value.trim());
    return (
      value.length <= 2048 &&
      url.href.length <= 2048 &&
      url.protocol === "https:" &&
      ["www.olx.ro", "olx.ro"].includes(url.hostname) &&
      !url.port &&
      !url.username &&
      !url.password &&
      ((url.pathname.startsWith("/d/oferta/") &&
        url.pathname.endsWith(".html") &&
        url.pathname.length > 20) ||
        (!listingOnly && url.pathname.startsWith(BOOKS)))
    );
  } catch {
    return false;
  }
}
/** Called only by a user's click. The desktop backend independently validates. */
export async function openOlx(value: string): Promise<void> {
  if (!validOlxUrl(value, false))
    throw new Error(
      "Use a public HTTPS OLX Romania listing or book search URL",
    );
  const url = new URL(value.trim());
  url.search = "";
  url.hash = "";
  url.hostname = "www.olx.ro";
  if (isDesktop()) return call<void>("open_marketplace_url", { url: url.href });
  window.open(url.href, "_blank", "noopener,noreferrer");
}
/** Blank costs mean zero; invalid costs never silently become zero. */
export function simulateNet(
  asking: string,
  fee: string,
  shipping: string,
  currency: string,
): number | null {
  const values = [
    parseMoneyToMinor(asking, currency),
    fee.trim() ? parseMoneyToMinor(fee, currency) : 0,
    shipping.trim() ? parseMoneyToMinor(shipping, currency) : 0,
  ];
  if (values.some((v) => v === null || v < 0)) return null;
  const [a, f, s] = values as number[];
  const result = BigInt(a) - BigInt(f) - BigInt(s);
  const number = Number(result);
  return Number.isSafeInteger(number) ? number : null;
}

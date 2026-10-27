// Barcode → ISBN pipeline for the add-book scanner. Decoding uses web APIs
// only, so it runs identically in the Tauri webview and in any browser (the
// web version): the platform's Shape Detection `BarcodeDetector` when present,
// otherwise a pure-JS ZXing reader over canvas pixels. ISBN extraction is
// pure and unit-tested (tests/barcode.test.ts).

import {
  BarcodeFormat,
  BinaryBitmap,
  DecodeHintType,
  HybridBinarizer,
  MultiFormatReader,
  RGBLuminanceSource,
} from "@zxing/library";

import { normalizeIsbn, parseIsbn, type NormalizedIsbn } from "@/lib/isbn";

/**
 * Pull a book ISBN out of a raw barcode value. Handles EAN-13 (the ISBN
 * barcode on book backs), EAN-13 with a glued EAN-5 add-on, and ISBN-10.
 * Non-book EANs and invalid checksums return null — nothing is invented.
 */
export function extractIsbnFromBarcode(raw: string): NormalizedIsbn | null {
  const s = normalizeIsbn(raw);
  const candidates = s.length > 13 ? [s.slice(0, 13), s] : [s];
  for (const candidate of candidates) {
    const parsed = parseIsbn(candidate);
    if (!parsed) continue;
    const thirteen = parsed.isbn13 ?? "";
    if (candidate.length === 10 || thirteen.startsWith("978") || thirteen.startsWith("979")) {
      return parsed;
    }
  }
  return null;
}

// --- decoding ---------------------------------------------------------------

interface NativeDetector {
  detect(source: CanvasImageSource): Promise<Array<{ rawValue: string }>>;
}

function nativeDetector(): NativeDetector | null {
  const w = window as unknown as {
    BarcodeDetector?: new (init: { formats: string[] }) => NativeDetector;
  };
  if (typeof w.BarcodeDetector !== "function") return null;
  try {
    return new w.BarcodeDetector({
      formats: ["ean_13", "ean_8", "upc_a", "upc_e", "code_128"],
    });
  } catch {
    return null;
  }
}

let zxing: MultiFormatReader | null = null;

function zxingReader(): MultiFormatReader {
  if (!zxing) {
    zxing = new MultiFormatReader();
    const hints = new Map<DecodeHintType, unknown>();
    hints.set(DecodeHintType.POSSIBLE_FORMATS, [
      BarcodeFormat.EAN_13,
      BarcodeFormat.EAN_8,
      BarcodeFormat.UPC_A,
      BarcodeFormat.UPC_E,
      BarcodeFormat.CODE_128,
    ]);
    hints.set(DecodeHintType.TRY_HARDER, true);
    zxing.setHints(hints);
  }
  return zxing;
}

/** Decode one barcode value from a canvas frame; null when none is readable. */
export async function decodeBarcodeFromCanvas(
  canvas: HTMLCanvasElement,
): Promise<string | null> {
  const native = nativeDetector();
  if (native) {
    try {
      const found = await native.detect(canvas);
      if (found.length > 0 && found[0].rawValue) return found[0].rawValue;
    } catch {
      // fall through to the JS decoder
    }
  }
  try {
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx || canvas.width === 0 || canvas.height === 0) return null;
    const image = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const source = new RGBLuminanceSource(image.data, canvas.width, canvas.height);
    const bitmap = new BinaryBitmap(new HybridBinarizer(source));
    return zxingReader().decode(bitmap).getText();
  } catch {
    // NotFoundException and friends: no barcode in this frame.
    zxingReader().reset();
    return null;
  }
}

type Drawable = HTMLImageElement | ImageBitmap;

async function drawableFromFile(file: File): Promise<Drawable> {
  if (typeof createImageBitmap === "function") {
    try {
      return await createImageBitmap(file);
    } catch {
      // SVG and other non-bitmap images fall back to <img>.
    }
  }
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("could not read that image file"));
    };
    img.src = url;
  });
}

/** Decode one barcode value from a chosen image file; null when none. */
export async function decodeBarcodeFromFile(file: File): Promise<string | null> {
  const drawable = await drawableFromFile(file);
  const width = drawable.width;
  const height = drawable.height;
  if (!width || !height) return null;
  const scale = Math.min(1, 1600 / Math.max(width, height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.drawImage(drawable, 0, 0, canvas.width, canvas.height);
  return decodeBarcodeFromCanvas(canvas);
}

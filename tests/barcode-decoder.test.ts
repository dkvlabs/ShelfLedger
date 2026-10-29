import { afterEach, expect, it, vi } from "vitest";
import { decodeBarcodeFromCanvas } from "@/lib/barcode";

const decoder = vi.hoisted(() => ({ load: vi.fn(), decode: vi.fn(), reset: vi.fn() }));
vi.mock("@zxing/library", () => {
  decoder.load();
  return {
    BarcodeFormat: {}, DecodeHintType: {},
    MultiFormatReader: class { setHints() {} decode = decoder.decode; reset = decoder.reset; },
    RGBLuminanceSource: class {}, HybridBinarizer: class {}, BinaryBitmap: class {},
  };
});
afterEach(() => vi.unstubAllGlobals());

function frame() {
  return { width: 2, height: 2, getContext: () => ({ getImageData: () => ({ data: new Uint8ClampedArray(16) }) }) } as unknown as HTMLCanvasElement;
}

it("keeps the JS decoder unloaded during library use and native scanning", async () => {
  expect(decoder.load).not.toHaveBeenCalled();
  vi.stubGlobal("BarcodeDetector", class { async detect() { return [{ rawValue: "9786067882629" }]; } });
  expect(await decodeBarcodeFromCanvas(frame())).toBe("9786067882629");
  expect(decoder.load).not.toHaveBeenCalled();
});

it("loads the local fallback for usable frames and handles unreadable barcodes", async () => {
  vi.stubGlobal("BarcodeDetector", undefined);
  expect(await decodeBarcodeFromCanvas({ width: 0, height: 0, getContext: () => null } as unknown as HTMLCanvasElement)).toBeNull();
  expect(decoder.load).not.toHaveBeenCalled();
  decoder.decode.mockReturnValueOnce({ getText: () => "9786067882629" });
  expect(await decodeBarcodeFromCanvas(frame())).toBe("9786067882629");
  expect(decoder.load).toHaveBeenCalledTimes(1);
  decoder.decode.mockImplementationOnce(() => { throw new Error("not found"); });
  expect(await decodeBarcodeFromCanvas(frame())).toBeNull();
  expect(decoder.reset).toHaveBeenCalledOnce();
  expect(decoder.load).toHaveBeenCalledTimes(1);
});

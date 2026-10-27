import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createElement } from "react";
import { readFileSync } from "node:fs";

import ScanBarcodeDialog from "@/features/catalog/ScanBarcodeDialog";
import { cameraErrorMessage } from "@/lib/camera";
import { decodeBarcodeFromCanvas } from "@/lib/barcode";

vi.mock("@/lib/i18n", () => ({ useT: () => (text: string) => text }));

vi.mock("@/lib/barcode", () => ({
  decodeBarcodeFromCanvas: vi.fn(),
  decodeBarcodeFromFile: vi.fn(),
  extractIsbnFromBarcode: vi.fn(() => ({
    isbn13: "9780306406157",
    isbn10: "0306406152",
  })),
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function camera(getUserMedia = vi.fn()) {
  vi.stubGlobal("navigator", { mediaDevices: { getUserMedia } });
  vi.stubGlobal("isSecureContext", true);
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  const stop = vi.fn();
  const stream = { getTracks: () => [{ stop }] } as unknown as MediaStream;
  return { getUserMedia, stream, stop };
}

function pending<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function scanner(open = true, onOpenChange = vi.fn(), onScan = vi.fn()) {
  return createElement(ScanBarcodeDialog, { open, onOpenChange, onScan });
}

describe("scanner camera access", () => {
  it("requests only video when scanning opens and releases it on close", async () => {
    const { getUserMedia, stream, stop } = camera();
    getUserMedia.mockResolvedValue(stream);
    const view = render(scanner(false));
    expect(getUserMedia).not.toHaveBeenCalled();
    view.rerender(scanner());
    await waitFor(() =>
      expect(screen.getByLabelText("Camera preview")).toBeVisible(),
    );
    expect(getUserMedia).toHaveBeenCalledExactlyOnceWith({
      audio: false,
      video: { facingMode: "environment", width: { ideal: 1280 } },
    });
    expect(
      (screen.getByLabelText("Camera preview") as HTMLVideoElement).srcObject,
    ).toBe(stream);
    // A parent render with a fresh callback must not ask for access again.
    view.rerender(scanner());
    expect(getUserMedia).toHaveBeenCalledTimes(1);
    view.rerender(scanner(false));
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("explains denied access and retries after the user changes permissions", async () => {
    const { getUserMedia, stream } = camera();
    getUserMedia
      .mockRejectedValueOnce(new DOMException("Denied", "NotAllowedError"))
      .mockResolvedValueOnce(stream);
    render(scanner());
    expect(await screen.findByRole("status")).toHaveTextContent(
      "site's browser permissions",
    );
    expect(
      screen.getByRole("button", { name: "Scan from photo" }),
    ).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Retry camera" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Camera preview")).toBeVisible(),
    );
    expect(getUserMedia).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it.each([
    ["NotReadableError", "Close other apps using it"],
    ["NotFoundError", "No camera was found"],
  ])("distinguishes %s from permission denial", async (name, message) => {
    const { getUserMedia } = camera();
    getUserMedia.mockRejectedValue(new DOMException("Camera error", name));
    render(scanner());
    expect(await screen.findByRole("status")).toHaveTextContent(message);
  });

  it("keeps photo scanning available when the camera API is missing", async () => {
    vi.stubGlobal("navigator", {});
    render(scanner());
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Camera access is not available",
    );
    expect(
      screen.queryByRole("button", { name: "Retry camera" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Scan from photo" }),
    ).toBeEnabled();
  });

  it("explains insecure origins without requesting a camera", async () => {
    const { getUserMedia } = camera();
    vi.stubGlobal("isSecureContext", false);
    render(scanner());
    expect(await screen.findByRole("status")).toHaveTextContent(
      "HTTPS or on localhost",
    );
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it("keeps a slow permission prompt pending and accepts a later grant", async () => {
    vi.useFakeTimers();
    const request = pending<MediaStream>();
    const { getUserMedia, stream } = camera(vi.fn(() => request.promise));
    render(scanner());
    await act(async () => {
      vi.advanceTimersByTime(10_000);
    });
    expect(screen.getByRole("status")).toHaveTextContent(
      "choose Allow to scan",
    );
    expect(
      screen.queryByRole("button", { name: "Retry camera" }),
    ).not.toBeInTheDocument();
    expect(getUserMedia).toHaveBeenCalledTimes(1);
    await act(async () => {
      request.resolve(stream);
    });
    expect(screen.getByLabelText("Camera preview")).toBeVisible();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("stops a stream granted after the scanner was closed", async () => {
    const request = pending<MediaStream>();
    const { stream, stop } = camera(vi.fn(() => request.promise));
    const view = render(scanner());
    view.unmount();
    await act(async () => {
      request.resolve(stream);
    });
    expect(stop).toHaveBeenCalledExactlyOnceWith();
    expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();
  });

  it("releases the camera and offers retry when preview playback fails", async () => {
    const { getUserMedia, stream, stop } = camera();
    getUserMedia.mockResolvedValue(stream);
    vi.mocked(HTMLMediaElement.prototype.play).mockRejectedValue(
      new Error("Playback failed"),
    );
    render(scanner());
    expect(await screen.findByRole("status")).toHaveTextContent(
      "camera could not be started",
    );
    expect(stop).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Retry camera" })).toBeEnabled();
  });

  it("ignores decoding that completes after the scanner closes", async () => {
    vi.useFakeTimers();
    const decoded = pending<string>();
    vi.mocked(decodeBarcodeFromCanvas).mockReturnValue(decoded.promise);
    const { getUserMedia, stream } = camera();
    getUserMedia.mockResolvedValue(stream);
    const onScan = vi.fn();
    const view = render(scanner(true, vi.fn(), onScan));
    await act(async () => {});
    const video = screen.getByLabelText("Camera preview");
    Object.defineProperties(video, {
      readyState: { value: 2 },
      videoWidth: { value: 1280 },
      videoHeight: { value: 720 },
    });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D);
    await act(async () => {
      vi.advanceTimersByTime(350);
    });
    expect(decodeBarcodeFromCanvas).toHaveBeenCalled();
    view.unmount();
    await act(async () => {
      decoded.resolve("9780306406157");
    });
    expect(onScan).not.toHaveBeenCalled();
  });
});

describe("camera permission recovery", () => {
  it("directs desktop users to system settings", () => {
    expect(cameraErrorMessage({ name: "NotAllowedError" }, true)).toContain(
      "System Settings → Privacy & Security → Camera",
    );
  });

  it("handles unexpected errors without exposing internal details", () => {
    expect(cameraErrorMessage(null, false)).toContain(
      "Check your camera and its permissions",
    );
  });
});

describe("desktop camera configuration", () => {
  it("ships the camera purpose, signing entitlement, and local media CSP", () => {
    const root = process.cwd();
    const config = JSON.parse(
      readFileSync(`${root}/src-tauri/tauri.conf.json`, "utf8"),
    );
    expect(config.bundle.macOS.signingIdentity).toBe("-");
    const plist = new DOMParser().parseFromString(
      readFileSync(
        `${root}/src-tauri/${config.bundle.macOS.infoPlist}`,
        "utf8",
      ),
      "text/xml",
    );
    const cameraKey = [...plist.querySelectorAll("key")].find(
      (key) => key.textContent === "NSCameraUsageDescription",
    );
    expect(cameraKey?.nextElementSibling?.textContent).toContain(
      "scan ISBN barcodes",
    );
    const entitlements = new DOMParser().parseFromString(
      readFileSync(
        `${root}/src-tauri/${config.bundle.macOS.entitlements}`,
        "utf8",
      ),
      "text/xml",
    );
    expect(
      [...entitlements.querySelectorAll("key")].map((key) => key.textContent),
    ).toEqual(["com.apple.security.device.camera"]);
    expect(entitlements.querySelector("key")?.nextElementSibling?.tagName).toBe(
      "true",
    );
    expect(config.app.security.csp).toContain("media-src 'self' blob:");
    expect(config.app.security.csp).toContain("img-src 'self' data: blob:");
  });
});

import { useT } from "@/lib/i18n";
import { useEffect, useRef, useState } from "react";
import { Camera, ImageUp, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  decodeBarcodeFromCanvas,
  decodeBarcodeFromFile,
  extractIsbnFromBarcode,
} from "@/lib/barcode";
import type { NormalizedIsbn } from "@/lib/isbn";
import { isDesktop } from "@/lib/api";
import { cameraErrorMessage } from "@/lib/camera";

export interface ScanResult extends NormalizedIsbn {
  raw: string;
}

const NO_CAMERA =
  "Camera access is not available in this window. Use “Scan from photo” instead, " +
  "or scan with a USB/Bluetooth barcode scanner — it types the ISBN into the field.";

const CAMERA_TIMEOUT =
  "Still waiting for camera access. If a permission prompt is open, choose Allow to scan. " +
  "You can also scan from a photo or type the ISBN manually.";

/**
 * ISBN barcode scanner for Add Book. Live camera scanning where the platform
 * allows it, plus photo scanning where camera access is unavailable or denied.
 * Decoding runs on web APIs only,
 * so this component works identically in the web version.
 */
export default function ScanBarcodeDialog({
  open,
  onOpenChange,
  onScan,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onScan: (result: ScanResult) => void;
}) {
  const tx = useT();
  const videoRef = useRef<HTMLVideoElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const frameRef = useRef<HTMLCanvasElement | null>(null);
  const onScanRef = useRef(onScan);
  onScanRef.current = onScan;
  const onOpenChangeRef = useRef(onOpenChange);
  onOpenChangeRef.current = onOpenChange;

  const [cameraState, setCameraState] = useState<
    "idle" | "starting" | "live" | "unavailable"
  >("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [imageBusy, setImageBusy] = useState(false);
  const [cameraAttempt, setCameraAttempt] = useState(0);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    let stream: MediaStream | null = null;
    let scanTimer: number | undefined;
    let startTimer: number | undefined;
    let busy = false;
    let video: HTMLVideoElement | null = null;
    setMessage(null);

    const stop = () => {
      window.clearTimeout(startTimer);
      window.clearInterval(scanTimer);
      stream?.getTracks().forEach((track) => track.stop());
      if (video && video.srcObject === stream) video.srcObject = null;
      stream = null;
    };

    const acceptRaw = (raw: string) => {
      const isbn = extractIsbnFromBarcode(raw);
      if (isbn) {
        cancelled = true;
        stop();
        onScanRef.current({ ...isbn, raw });
        onOpenChangeRef.current(false);
      } else {
        setMessage(
          `Read “${raw}” — that barcode is not a book ISBN. Try again, or type the ISBN manually.`,
        );
      }
    };

    const scanFrame = async () => {
      if (cancelled || !video || video.readyState < 2 || busy) return;
      busy = true;
      try {
        const width = video.videoWidth;
        const height = video.videoHeight;
        if (!width || !height) return;
        const frame = (frameRef.current ??= document.createElement("canvas"));
        const scale = Math.min(1, 1280 / width);
        frame.width = Math.round(width * scale);
        frame.height = Math.round(height * scale);
        const ctx = frame.getContext("2d");
        if (!ctx) return;
        ctx.drawImage(video, 0, 0, frame.width, frame.height);
        const raw = await decodeBarcodeFromCanvas(frame);
        if (raw && !cancelled) acceptRaw(raw);
      } finally {
        busy = false;
      }
    };

    const cameraAvailable = !!navigator.mediaDevices?.getUserMedia;
    if (!cameraAvailable || window.isSecureContext === false) {
      setCameraState("unavailable");
      setMessage(
        window.isSecureContext === false
          ? "Camera access requires a secure connection. Open ShelfLedger over HTTPS or on localhost, " +
              "or scan from a photo."
          : NO_CAMERA,
      );
    } else {
      setCameraState("starting");
      // Permission prompts can remain open indefinitely. Keep the request pending
      // and offer the photo fallback without issuing overlapping requests.
      startTimer = window.setTimeout(() => {
        if (!cancelled && stream === null) {
          setMessage(CAMERA_TIMEOUT);
        }
      }, 10_000);
      navigator.mediaDevices
        .getUserMedia({
          audio: false,
          video: { facingMode: "environment", width: { ideal: 1280 } },
        })
        .then(async (cameraStream) => {
          window.clearTimeout(startTimer);
          if (cancelled) {
            cameraStream.getTracks().forEach((track) => track.stop());
            return;
          }
          stream = cameraStream;
          video = videoRef.current;
          if (video) {
            video.srcObject = stream;
            await video.play();
          }
          if (cancelled) return;
          setCameraState("live");
          setMessage(null);
          scanTimer = window.setInterval(() => void scanFrame(), 350);
        })
        .catch((error: unknown) => {
          window.clearTimeout(startTimer);
          if (!cancelled) {
            stop();
            setCameraState("unavailable");
            setMessage(cameraErrorMessage(error, isDesktop()));
          }
        });
    }

    return () => {
      cancelled = true;
      stop();
    };
  }, [open, cameraAttempt]);

  const pickImage = async (file: File) => {
    setImageBusy(true);
    setMessage(null);
    try {
      const raw = await decodeBarcodeFromFile(file);
      if (raw) {
        const isbn = extractIsbnFromBarcode(raw);
        if (isbn) {
          onScan({ ...isbn, raw });
          onOpenChange(false);
        } else {
          setMessage(
            `Read “${raw}” — that barcode is not a book ISBN. Try a photo of the ISBN barcode instead.`,
          );
        }
      } else {
        setMessage(
          "No barcode found in that image. Try a closer, sharper photo of the barcode on the back of the book.",
        );
      }
    } catch (err) {
      setMessage((err as Error).message);
    } finally {
      setImageBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) setMessage(null);
        onOpenChange(o);
      }}
    >
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{tx("Scan barcode")}</DialogTitle>
          <DialogDescription>
            {tx(
              "Point a camera at the ISBN barcode on the back of the book — or scan from a photo. USB/Bluetooth scanners work too: they just type the ISBN into the field.",
            )}{" "}
            {tx(
              "Allow camera access when prompted. Only video is requested, and images are processed on your device.",
            )}
          </DialogDescription>
        </DialogHeader>

        <video
          ref={videoRef}
          muted
          playsInline
          aria-label={tx("Camera preview")}
          className={`aspect-video w-full rounded-md bg-black object-cover ${
            cameraState === "live" ? "" : "hidden"
          }`}
        />
        {cameraState !== "live" && (
          <div className="flex aspect-video w-full items-center justify-center rounded-md border border-dashed text-sm text-muted-foreground">
            {cameraState === "starting" ? (
              <span className="flex items-center gap-2">
                <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
                {tx("Waiting for camera access…")}
              </span>
            ) : (
              "Camera not available — scan from a photo below"
            )}
          </div>
        )}

        {message && (
          <p role="status" className="text-sm text-muted-foreground">
            {tx(message)}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          {cameraState === "unavailable" &&
            typeof navigator.mediaDevices?.getUserMedia === "function" &&
            window.isSecureContext !== false && (
              <Button
                type="button"
                onClick={() => setCameraAttempt((attempt) => attempt + 1)}
              >
                <Camera />
                {tx("Retry camera")}
              </Button>
            )}
          <input
            ref={imageInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            aria-label={tx("Choose a photo of a barcode")}
            onChange={(e) => {
              const file = e.target.files?.[0];
              e.target.value = "";
              if (file) void pickImage(file);
            }}
          />
          <Button
            type="button"
            variant="secondary"
            onClick={() => imageInputRef.current?.click()}
          >
            {imageBusy ? <Loader2 className="animate-spin" /> : <ImageUp />}
            {tx("Scan from photo")}
          </Button>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {tx("Close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

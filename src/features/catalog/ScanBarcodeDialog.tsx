import { useEffect, useRef, useState } from "react";
import { ImageUp, Loader2 } from "lucide-react";

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

export interface ScanResult extends NormalizedIsbn {
  raw: string;
}

const NO_CAMERA =
  "Camera access is not available in this window. Use “Scan from photo” instead, " +
  "or scan with a USB/Bluetooth barcode scanner — it types the ISBN into the field.";

const CAMERA_BLOCKED =
  "Camera permission was denied or the camera is in use. Use “Scan from photo” instead, " +
  "or type the ISBN manually.";

const CAMERA_TIMEOUT =
  "No camera signal yet (a permission prompt may still be open). Use “Scan from photo” " +
  "below, or type the ISBN manually.";

/**
 * ISBN barcode scanner for Add Book. Live camera scanning where the platform
 * allows it, plus photo scanning everywhere (the Tauri webview on macOS has no
 * camera API — the photo path works there). Decoding runs on web APIs only,
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
  const videoRef = useRef<HTMLVideoElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const timerRef = useRef<number | null>(null);
  const busyRef = useRef(false);
  const frameRef = useRef<HTMLCanvasElement | null>(null);
  const onScanRef = useRef(onScan);
  onScanRef.current = onScan;

  const [cameraState, setCameraState] = useState<"idle" | "starting" | "live" | "unavailable">(
    "idle",
  );
  const [message, setMessage] = useState<string | null>(null);
  const [imageBusy, setImageBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;

    const stop = () => {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
        timerRef.current = null;
      }
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
    };

    const acceptRaw = (raw: string) => {
      const isbn = extractIsbnFromBarcode(raw);
      if (isbn) {
        stop();
        onScanRef.current({ ...isbn, raw });
        onOpenChange(false);
      } else {
        setMessage(
          `Read “${raw}” — that barcode is not a book ISBN. Try again, or type the ISBN manually.`,
        );
      }
    };

    const scanFrame = async () => {
      const video = videoRef.current;
      if (!video || video.readyState < 2 || busyRef.current) return;
      busyRef.current = true;
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
        if (raw) acceptRaw(raw);
      } finally {
        busyRef.current = false;
      }
    };

    if (!navigator.mediaDevices?.getUserMedia) {
      setCameraState("unavailable");
      setMessage(NO_CAMERA);
    } else {
      setCameraState("starting");
      // Some webviews accept the getUserMedia call but never answer; don't
      // spin forever — fall back to photo scanning after 10s. If the stream
      // arrives late, the .then below still upgrades to live.
      const startTimer = window.setTimeout(() => {
        if (!cancelled && streamRef.current === null) {
          setCameraState("unavailable");
          setMessage(CAMERA_TIMEOUT);
        }
      }, 10_000);
      navigator.mediaDevices
        .getUserMedia({ video: { facingMode: "environment", width: { ideal: 1280 } } })
        .then((stream) => {
          window.clearTimeout(startTimer);
          if (cancelled) {
            stream.getTracks().forEach((t) => t.stop());
            return;
          }
          streamRef.current = stream;
          const video = videoRef.current;
          if (video) {
            video.srcObject = stream;
            void video.play().catch(() => undefined);
          }
          setCameraState("live");
          setMessage(null);
          timerRef.current = window.setInterval(() => void scanFrame(), 350);
        })
        .catch(() => {
          window.clearTimeout(startTimer);
          if (!cancelled) {
            setCameraState("unavailable");
            setMessage(CAMERA_BLOCKED);
          }
        });
    }

    return () => {
      cancelled = true;
      stop();
    };
  }, [open, onOpenChange]);

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
          <DialogTitle>Scan barcode</DialogTitle>
          <DialogDescription>
            Point a camera at the ISBN barcode on the back of the book — or scan from a
            photo. USB/Bluetooth scanners work too: they just type the ISBN into the field.
          </DialogDescription>
        </DialogHeader>

        <video
          ref={videoRef}
          muted
          playsInline
          aria-label="Camera preview"
          className={`aspect-video w-full rounded-md bg-black object-cover ${
            cameraState === "live" ? "" : "hidden"
          }`}
        />
        {cameraState !== "live" && (
          <div className="flex aspect-video w-full items-center justify-center rounded-md border border-dashed text-sm text-muted-foreground">
            {cameraState === "starting" ? (
              <Loader2 className="h-5 w-5 animate-spin" aria-label="Starting camera" />
            ) : (
              "Camera not available — scan from a photo below"
            )}
          </div>
        )}

        {message && (
          <p role="status" className="text-sm text-muted-foreground">
            {message}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <input
            ref={imageInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            aria-label="Choose a photo of a barcode"
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
            Scan from photo
          </Button>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

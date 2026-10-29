import { useEffect, useState } from "react";
import { Camera, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { cameraErrorMessage } from "@/lib/camera";
import { isDesktop } from "@/lib/api";
import { useT } from "@/lib/i18n";

export default function CoverCameraDialog({ onClose, onCapture }: {
  onClose: () => void; onCapture: (data: string) => void;
}) {
  const tx = useT();
  const [video, setVideo] = useState<HTMLVideoElement | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [ready, setReady] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [photo, setPhoto] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let stream: MediaStream | null = null;
    setReady(false);
    setMessage(null);
    const element = video;
    const stop = () => { stream?.getTracks().forEach((track) => track.stop()); stream = null; if (element) element.srcObject = null; };
    // The dialog portal can mount after this component's first effect.
    if (photo || !element) return;
    const timer = window.setTimeout(() => {
      if (!cancelled) setMessage("Waiting for camera permission. Allow access, retry, or close this window and choose a photo.");
    }, 8000);
    async function start() {
      if (!navigator.mediaDevices?.getUserMedia) {
        setMessage("Camera unavailable. Close this window and choose a photo instead.");
        window.clearTimeout(timer);
        return;
      }
      try {
        const acquired = await navigator.mediaDevices.getUserMedia({ audio: false, video: { facingMode: { ideal: "environment" }, width: { ideal: 1600 }, height: { ideal: 1200 } } });
        if (cancelled) { acquired.getTracks().forEach((track) => track.stop()); return; }
        stream = acquired;
        if (element) { element.srcObject = stream; await element.play(); }
        if (!cancelled) { setReady(true); setMessage(null); }
      } catch (error) {
        stop();
        if (!cancelled) setMessage(cameraErrorMessage(error, isDesktop()));
      } finally { window.clearTimeout(timer); }
    }
    void start();
    return () => { cancelled = true; window.clearTimeout(timer); stop(); };
  }, [attempt, photo, video]);
  function capture() {
    const element = video;
    if (!element?.videoWidth || !element.videoHeight) return;
    const canvas = document.createElement("canvas");
    const scale = Math.min(1, 1200 / Math.max(element.videoWidth, element.videoHeight));
    canvas.width = Math.round(element.videoWidth * scale);
    canvas.height = Math.round(element.videoHeight * scale);
    const context = canvas.getContext("2d");
    if (!context) return;
    context.drawImage(element, 0, 0, canvas.width, canvas.height);
    setPhoto(canvas.toDataURL("image/jpeg", .85));
  }
  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
    <DialogContent className="max-w-lg">
      <DialogHeader><DialogTitle>{tx("Photograph cover")}</DialogTitle>
        <DialogDescription>{tx("Frame the front cover. Photos stay on this device and are included in library backups.")}</DialogDescription>
      </DialogHeader>
      {photo ? <img src={photo} alt={tx("Cover preview")} className="max-h-[50vh] w-full rounded-md object-contain" />
        : <video ref={setVideo} muted playsInline autoPlay className="max-h-[50vh] w-full rounded-md bg-black object-contain" />}
      {message && <p role="status" className="text-sm text-muted-foreground">{tx(message)}</p>}
      <div className="flex flex-wrap gap-2">
        {photo ? <><Button onClick={() => { onCapture(photo); onClose(); }}>{tx("Save photo")}</Button>
          <Button variant="outline" onClick={() => setPhoto(null)}><RotateCcw />{tx("Retake photo")}</Button></>
          : <><Button disabled={!ready} onClick={capture}><Camera />{tx("Take photo")}</Button>
            {message && <Button variant="outline" onClick={() => setAttempt((n) => n + 1)}>{tx("Retry camera")}</Button>}</>}
        <Button variant="ghost" onClick={onClose}>{tx("Cancel")}</Button>
      </div>
    </DialogContent>
  </Dialog>;
}

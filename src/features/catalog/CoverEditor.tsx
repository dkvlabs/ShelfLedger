import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, Download, ImageUp, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, CommandError } from "@/lib/api";
import { readCoverPhoto } from "@/lib/cover-photo";
import { useT } from "@/lib/i18n";
import { formatDate } from "@/lib/format";
import type { EditionDto } from "@/types/domain";
import BookCover from "./BookCover";
import CoverCameraDialog from "./CoverCameraDialog";

export default function CoverEditor({ edition, title }: { edition: EditionDto; title: string }) {
  const tx = useT();
  const client = useQueryClient();
  const input = useRef<HTMLInputElement>(null);
  const [cameraOpen, setCameraOpen] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { data: cover } = useQuery({ queryKey: ["cover", edition.coverPath],
    queryFn: () => api.getCover(edition.coverPath!), enabled: !!edition.coverPath?.startsWith("covers/"), staleTime: Infinity, retry: false });
  const save = useMutation({
    mutationFn: async (action: { kind: "photo"; data: string } | { kind: "file"; file: File } | { kind: "fetch" } | { kind: "remove" }) => {
      setNote(null); setError(null);
      if (action.kind === "remove") { await api.removeCover(edition.id); return true; }
      if (action.kind === "fetch") return !!(await api.fetchCover(edition.id));
      await api.saveCoverPhoto(edition.id, action.kind === "file" ? await readCoverPhoto(action.file) : action.data);
      return true;
    },
    onSuccess: (saved, action) => {
      setNote(saved ? action.kind === "remove" ? "Cover removed." : "Cover saved on this device."
        : "No cover found for this ISBN. Choose or take a photo instead.");
      void client.invalidateQueries({ queryKey: ["work"] });
      void client.invalidateQueries({ queryKey: ["works"] });
    },
    onError: (error) => setError(error instanceof CommandError && error.kind === "metadata_disabled"
      ? "Online cover fetch is disabled on this device. Choose or take a photo instead."
      : error instanceof CommandError && error.kind === "network"
        ? "The cover provider is unavailable. Your saved cover is unchanged. Choose or take a photo instead."
        : error.message),
  });
  return <div className="my-3 space-y-3">
    <BookCover path={edition.coverPath} title={title} className="mx-auto w-36 shadow-lg" />
    {cover && <p className="text-xs text-muted-foreground">{tx(cover.source === "openlibrary" ? "Cover source: Open Library" : "Cover source: your photo")} · {formatDate(cover.createdAt)}</p>}
    <p className="text-xs text-muted-foreground">{tx("Save a cover for this edition. Fetch uses its ISBN and replaces the current cover; photo upload works offline.")}</p>
    <div className="flex flex-wrap gap-2">
      <Button size="sm" variant="outline" disabled={save.isPending || !(edition.isbn13 || edition.isbn10)} onClick={() => save.mutate({ kind: "fetch" })}><Download />{tx("Fetch cover")}</Button>
      <Button size="sm" variant="outline" disabled={save.isPending} onClick={() => input.current?.click()}><ImageUp />{tx("Choose cover photo")}</Button>
      <Button size="sm" variant="outline" disabled={save.isPending} onClick={() => setCameraOpen(true)}><Camera />{tx("Photograph cover")}</Button>
      {edition.coverPath && <Button size="sm" variant="ghost" disabled={save.isPending} onClick={() => save.mutate({ kind: "remove" })}><Trash2 />{tx("Remove cover")}</Button>}
    </div>
    <input ref={input} type="file" accept="image/jpeg,image/png,image/webp" aria-label={tx("Choose cover photo")} className="hidden"
      onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) save.mutate({ kind: "file", file }); }} />
    <p className="text-xs text-muted-foreground">{tx("JPEG, PNG or WebP, up to 10 MiB. Saved covers are resized for offline use.")}</p>
    {save.isPending && <p role="status" className="text-xs">{tx("Saving cover…")}</p>}
    {note && <p role="status" className="text-xs text-muted-foreground">{tx(note)}</p>}
    {error && <p role="alert" className="text-xs text-destructive">{tx(error)}</p>}
    {cameraOpen && <CoverCameraDialog onClose={() => setCameraOpen(false)} onCapture={(data) => save.mutate({ kind: "photo", data })} />}
  </div>;
}

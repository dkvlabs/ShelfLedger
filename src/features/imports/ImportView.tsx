import { useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FileUp } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import type { ImportPreview } from "@/types/domain";

export default function ImportView() {
  const queryClient = useQueryClient();
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [policy, setPolicy] = useState<"skip" | "new_work">("skip");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const pendingCommit = useRef<string | null>(null);

  const pickFile = useMutation({
    mutationFn: async () => {
      const file = await open({
        multiple: false,
        filters: [
          { name: "Goodreads export / JSON", extensions: ["csv", "json"] },
        ],
      });
      return typeof file === "string" ? file : null;
    },
    onSuccess: async (file) => {
      setError(null);
      setPreview(null);
      setResult(null);
      if (!file) return;
      setPath(file);
      try {
        if (file.toLowerCase().endsWith(".json")) {
          // JSON import commits directly after a confirm dialog.
          pendingCommit.current = null;
          const res = await api.importLibraryJson(file);
          setResult(
            `JSON import: ${res.importedWorks} works, ${res.importedEditions} editions, ${res.importedCopies} copies, ${res.skipped} skipped` +
              (res.errors.length ? ` — errors: ${res.errors.join("; ")}` : ""),
          );
          void queryClient.invalidateQueries({ queryKey: ["works"] });
          void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
        } else {
          const p = await api.previewGoodreadsImport(file);
          pendingCommit.current = file;
          setPreview(p);
          setSelected(new Set(p.rows.map((r) => r.rowNumber)));
        }
      } catch (err) {
        setError((err as Error).message);
      }
    },
    onError: (err) => setError((err as Error).message),
  });

  const commit = useMutation({
    mutationFn: () => {
      if (!pendingCommit.current) throw new Error("no file selected");
      return api.commitGoodreadsImport(pendingCommit.current, {
        rowNumbers: [...selected],
        duplicatePolicy: policy,
      });
    },
    onSuccess: (res) => {
      setPreview(null);
      setPath(null);
      setResult(
        `Imported ${res.importedWorks} works, ${res.importedEditions} editions, ${res.skipped} skipped` +
          (res.errors.length ? ` — errors: ${res.errors.join("; ")}` : ""),
      );
      void queryClient.invalidateQueries({ queryKey: ["works"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions"] });
    },
    onError: (err) => setError((err as Error).message),
  });

  const toggleRow = (rowNumber: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(rowNumber)) next.delete(rowNumber);
      else next.add(rowNumber);
      return next;
    });
  };

  return (
    <div className="max-w-3xl space-y-4 p-6">
      <h1 className="text-xl font-semibold">Import</h1>
      <p className="text-sm text-muted-foreground">
        Goodreads CSV export (preview, mapping and duplicate resolution) or a
        ShelfLedger JSON library file. The app reads the file directly; nothing is
        uploaded anywhere.
      </p>

      <Button onClick={() => pickFile.mutate()} disabled={pickFile.isPending}>
        <FileUp /> Choose file…
      </Button>
      {path && <p className="text-xs text-muted-foreground">Selected: {path}</p>}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {result && (
        <p className="rounded-md border p-3 text-sm" role="status">
          {result}
        </p>
      )}

      {preview && (
        <Card>
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <CardTitle className="text-sm">
                Preview — {preview.rows.length} rows
              </CardTitle>
              <div className="flex items-center gap-2">
                <Label htmlFor="policy" className="text-xs text-muted-foreground">
                  Duplicates
                </Label>
                <Select value={policy} onValueChange={(v) => setPolicy(v as typeof policy)}>
                  <SelectTrigger id="policy" className="w-40">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="skip">Skip duplicates</SelectItem>
                    <SelectItem value="new_work">Add as new work anyway</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="max-h-96 space-y-2 overflow-y-auto">
              {preview.rows.map((row) => (
                <div key={row.rowNumber} className="rounded-md border p-2 text-sm">
                  <label className="flex items-start gap-2">
                    <input
                      type="checkbox"
                      className="mt-1"
                      checked={selected.has(row.rowNumber)}
                      onChange={() => toggleRow(row.rowNumber)}
                      aria-label={`Import row ${row.rowNumber}: ${row.title}`}
                    />
                    <span>
                      <span className="font-medium">{row.title || "(no title)"}</span>{" "}
                      <span className="text-muted-foreground">
                        — {row.author || "unknown"} {row.isbn13 ? `· ${row.isbn13}` : ""}
                      </span>
                      <span className="ml-2 flex flex-wrap items-center gap-1">
                        {row.shelf && <Badge variant="outline">{row.shelf}</Badge>}
                        {row.rating != null && row.rating > 0 && (
                          <Badge variant="secondary">{row.rating}/5</Badge>
                        )}
                        {row.dateRead && (
                          <Badge variant="secondary">read {row.dateRead}</Badge>
                        )}
                      </span>
                      {(row.duplicateOf || row.warnings.length > 0) && (
                        <span className="mt-1 block text-xs text-amber-600 dark:text-amber-400">
                          {row.duplicateOf && <>Already in library: {row.duplicateOf}. </>}
                          {row.warnings.join(" ")}
                        </span>
                      )}
                    </span>
                  </label>
                </div>
              ))}
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setPreview(null)}>
                Cancel
              </Button>
              <Button disabled={selected.size === 0 || commit.isPending} onClick={() => commit.mutate()}>
                Import {selected.size} row(s)
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

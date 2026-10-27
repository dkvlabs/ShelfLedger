import { useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FileUp, Check, ArrowRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { api } from "@/lib/api";
import { pickFile } from "@/lib/files";
import { formatDate } from "@/lib/format";
import { useT } from "@/lib/i18n";
import type { FileImportPreview, ImportResult } from "@/types/domain";
const fields = [
  "Title",
  "Author",
  "ISBN13",
  "ISBN",
  "Exclusive Shelf",
  "Bookshelves",
  "My Rating",
  "My Review",
  "Date Read",
];
const policies = [
  { value: "skip", label: "Skip duplicates" },
  { value: "add_copy", label: "Add copy to existing edition" },
  { value: "new_work", label: "Create a separate work" },
];
export default function ImportView({
  initialPath,
}: {
  initialPath?: string | null;
}) {
  const t = useT();
  const client = useQueryClient();
  const usedPath = useRef<string | null>(null);
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<FileImportPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [mappedPreview, setMappedPreview] = useState("{}");
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [rowPolicies, setRowPolicies] = useState<Record<number, string>>({});
  const [policy, setPolicy] = useState<"skip" | "add_copy" | "new_work">(
    "skip",
  );
  const [result, setResult] = useState<ImportResult | null>(null);
  const [confirm, setConfirm] = useState(false);
  const load = useMutation({
    mutationFn: ({
      file,
      map,
    }: {
      file: string;
      map: Record<string, string>;
    }) => api.previewImport(file, map),
    onSuccess: (value, vars) => {
      setMappedPreview(JSON.stringify(vars.map));
      setPath(vars.file);
      setPreview(value);
      setSelected(new Set(value.rows.map((r) => r.rowNumber)));
      setResult(null);
      setConfirm(false);
    },
  });
  useEffect(() => {
    if (initialPath && usedPath.current !== initialPath) {
      usedPath.current = initialPath;
      load.mutate({ file: initialPath, map: {} });
    }
  }, [initialPath]);
  const choose = useMutation({
    mutationFn: () => pickFile(["csv", "json"]),
    onSuccess: (file) => {
      if (file) {
        setMapping({});
        load.mutate({ file, map: {} });
      }
    },
  });
  const commit = useMutation({
    mutationFn: () => {
      if (!path) throw new Error(t("Choose a file first"));
      return api.commitImport(path, mapping, {
        rowNumbers: [...selected],
        duplicatePolicy: policy,
        rowPolicies,
      });
    },
    onSuccess: (value) => {
      setResult(value);
      setPreview(null);
      setPath(null);
      setConfirm(false);
      void client.invalidateQueries();
    },
  });
  const error = load.error || choose.error || commit.error;
  return (
    <div className="space-y-6">
      <div>
        <h1>{t("Import")}</h1>
        <p className="mt-4 max-w-2xl text-muted-foreground">
          {t(
            "Bring your collection home. Preview Goodreads or custom CSV files, map your columns, and review duplicates before anything is saved.",
          )}
        </p>
      </div>
      <div className="flex items-center gap-3 text-xs text-muted-foreground">
        <span className="text-primary">01 {t("Choose file")}</span>
        <ArrowRight size={14} />
        <span className={preview ? "text-primary" : ""}>
          02 {t("Review and map")}
        </span>
        <ArrowRight size={14} />
        <span className={confirm || result ? "text-primary" : ""}>
          03 {t("Confirm import")}
        </span>
      </div>
      <Button
        onClick={() => choose.mutate()}
        disabled={choose.isPending || load.isPending || commit.isPending}
      >
        <FileUp />
        {t(load.isPending ? "Loading…" : "Choose file…")}
      </Button>
      {error && (
        <p
          role="alert"
          className="rounded-md border border-destructive p-3 text-sm text-destructive"
        >
          {error.message}
        </p>
      )}
      {preview && (
        <>
          {preview.kind === "csv" && (
            <Card>
              <CardHeader>
                <CardTitle>{t("Column mapping")}</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="mb-4 text-sm text-muted-foreground">
                  {t(
                    "Matching column names are recognized automatically. Map different names below, then refresh the preview.",
                  )}
                </p>
                <div className="grid gap-4 sm:grid-cols-3">
                  {fields.map((field) => (
                    <label key={field} className="text-xs">
                      {t(field)}
                      <select
                        aria-label={`${t("Map column")}: ${field}`}
                        className="mt-1 block w-full rounded-md border bg-background p-2"
                        value={
                          mapping[field] ??
                          (preview.headers.includes(field) ? field : "")
                        }
                        onChange={(e) => {
                          setConfirm(false);
                          setMapping((v) => ({
                            ...v,
                            [field]: e.target.value,
                          }));
                        }}
                      >
                        <option value="">{t("Not mapped")}</option>
                        {preview.headers.map((name) => (
                          <option key={name} value={name}>
                            {name}
                          </option>
                        ))}
                      </select>
                    </label>
                  ))}
                </div>
                <Button
                  className="mt-4"
                  variant="outline"
                  disabled={load.isPending}
                  onClick={() =>
                    path && load.mutate({ file: path, map: mapping })
                  }
                >
                  {t("Refresh preview")}
                </Button>
              </CardContent>
            </Card>
          )}
          <Card>
            <CardHeader>
              <CardTitle>
                {t("Preview")} · {preview.rows.length} {t("rows")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {preview.kind === "snapshot" ? (
                <p className="text-sm text-muted-foreground">
                  {t(
                    "This complete library snapshot keeps IDs, reading history, loans, prices and archives together. Conflicting records stop the import without overwriting your library.",
                  )}
                </p>
              ) : (
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={selected.size === preview.rows.length}
                    onChange={(e) =>
                      setSelected(
                        new Set(
                          e.target.checked
                            ? preview.rows.map((r) => r.rowNumber)
                            : [],
                        ),
                      )
                    }
                  />
                  {t("Select all")}
                </label>
              )}
              {preview.kind === "csv" && (
                <label className="flex items-center gap-3 text-sm">
                  {t("Default duplicate action")}
                  <select
                    className="rounded-md border bg-background p-2"
                    value={policy}
                    onChange={(e) => setPolicy(e.target.value as typeof policy)}
                  >
                    {policies.map((p) => (
                      <option key={p.value} value={p.value}>
                        {t(p.label)}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <div className="max-h-[480px] space-y-2 overflow-y-auto">
                {preview.rows.map((row) => (
                  <div
                    key={row.rowNumber}
                    className="rounded-md border p-4 text-sm"
                  >
                    <label className="flex items-start gap-3">
                      <input
                        type="checkbox"
                        disabled={preview.kind === "snapshot"}
                        className="mt-1"
                        checked={selected.has(row.rowNumber)}
                        onChange={(e) =>
                          setSelected((v) => {
                            const next = new Set(v);
                            if (e.target.checked) next.add(row.rowNumber);
                            else next.delete(row.rowNumber);
                            return next;
                          })
                        }
                      />
                      <span>
                        <strong>{row.title || t("Missing title")}</strong>
                        <span className="mt-1 block text-xs text-muted-foreground">
                          {row.author} {row.isbn13 ? `· ${row.isbn13}` : ""}{" "}
                          {row.dateRead ? `· ${formatDate(row.dateRead)}` : ""}
                        </span>
                        {row.shelf && (
                          <span className="mt-1 block text-xs text-primary">
                            {row.shelf}
                          </span>
                        )}
                        {row.duplicateOf && (
                          <span className="mt-1 block text-xs text-primary">
                            {t("Already in library")}: {row.duplicateOf}
                          </span>
                        )}
                        {row.warnings.map((w) => (
                          <span
                            key={w}
                            className="mt-1 block text-xs text-amber-500"
                          >
                            {t(w)}
                          </span>
                        ))}
                      </span>
                    </label>
                    {row.duplicateOf && preview.kind === "csv" && (
                      <select
                        aria-label={`${t("Duplicate action for row")} ${row.rowNumber}`}
                        className="mt-3 rounded-md border bg-background p-2 text-xs"
                        value={rowPolicies[row.rowNumber] ?? policy}
                        onChange={(e) =>
                          setRowPolicies((v) => ({
                            ...v,
                            [row.rowNumber]: e.target.value,
                          }))
                        }
                      >
                        {policies.map((p) => (
                          <option key={p.value} value={p.value}>
                            {t(p.label)}
                          </option>
                        ))}
                      </select>
                    )}
                  </div>
                ))}
              </div>
              {confirm ? (
                <div className="space-y-3 rounded-md border border-primary/40 p-4">
                  <p className="text-sm">
                    {t(
                      "Save the selected rows to this library? Review your selection before confirming.",
                    )}
                  </p>
                  <div className="flex gap-2">
                    <Button
                      disabled={commit.isPending}
                      onClick={() => commit.mutate()}
                    >
                      <Check />
                      {t(commit.isPending ? "Importing…" : "Confirm import")}
                    </Button>
                    <Button variant="ghost" onClick={() => setConfirm(false)}>
                      {t("Back to preview")}
                    </Button>
                  </div>
                </div>
              ) : (
                <div className="flex justify-end gap-3">
                  <Button variant="ghost" onClick={() => setPreview(null)}>
                    {t("Cancel")}
                  </Button>
                  <Button
                    disabled={
                      JSON.stringify(mapping) !== mappedPreview ||
                      (selected.size === 0 && preview.kind !== "snapshot")
                    }
                    onClick={() => setConfirm(true)}
                  >
                    {t("Review import")} ({selected.size})<ArrowRight />
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        </>
      )}
      {result && (
        <Card>
          <CardHeader>
            <CardTitle>{t("Import complete")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3" role="status">
            <p>
              {result.importedWorks} {t("works")}, {result.importedEditions}{" "}
              {t("editions")}, {result.importedCopies} {t("copies")},{" "}
              {result.skipped} {t("skipped")}
            </p>
            {result.rows?.length > 0 && (
              <details>
                <summary className="cursor-pointer text-sm">
                  {t("Row log")}
                </summary>
                <ul className="mt-3 space-y-2 text-xs">
                  {result.rows.map((row) => (
                    <li key={row.rowNumber}>
                      #{row.rowNumber} · {t(row.status)} {row.message}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            {result.errors.length > 0 && (
              <ul className="list-inside list-disc text-sm text-destructive">
                {result.errors.map((e) => (
                  <li key={e}>{e}</li>
                ))}
              </ul>
            )}
            <p className="text-xs text-muted-foreground">
              {t(
                "The import log is kept with your library and included in JSON exports and backups.",
              )}
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

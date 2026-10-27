import { useT } from "@/lib/i18n";
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Loader2, ScanBarcode, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api, CommandError } from "@/lib/api";
import ScanBarcodeDialog, { type ScanResult } from "./ScanBarcodeDialog";
import { parseMoneyToMinor, todayIso } from "@/lib/format";
import { parseIsbn } from "@/lib/isbn";
import {
  ACQUISITION_LABELS,
  ACQUISITION_TYPES,
  CONDITION_GRADES,
  CONDITION_LABELS,
  LOCATION_TYPES,
  LOCATION_LABELS,
  type AddBookResult,
  type LookupIsbnResult,
  type WorkSummary,
} from "@/types/domain";

interface Draft {
  isbn13: string;
  isbn10: string;
  title: string;
  subtitle: string;
  authorNames: string;
  publisher: string;
  publishedAt: string;
  language: string;
  pageCount: string;
  // copy fields
  conditionGrade: string;
  locationId: string;
  acquisitionType: string;
  acquiredAt: string;
  price: string;
  currency: string;
  store: string;
}

const emptyDraft: Draft = {
  isbn13: "",
  isbn10: "",
  title: "",
  subtitle: "",
  authorNames: "",
  publisher: "",
  publishedAt: "",
  language: "",
  pageCount: "",
  conditionGrade: "good",
  locationId: "",
  acquisitionType: "bought",
  acquiredAt: todayIso(),
  price: "",
  currency: "RON",
  store: "",
};

function applyLookup(d: Draft, result: LookupIsbnResult): Draft {
  return {
    ...d,
    isbn13: result.isbn13 ?? d.isbn13,
    isbn10: result.isbn10 ?? d.isbn10,
    title: result.title ?? d.title,
    subtitle: result.subtitle ?? d.subtitle,
    authorNames: result.authorNames.length
      ? result.authorNames.join("; ")
      : d.authorNames,
    publisher: result.publisher ?? d.publisher,
    publishedAt: result.publishedAt ?? d.publishedAt,
    language: result.language ?? d.language,
    pageCount: result.pageCount ? String(result.pageCount) : d.pageCount,
  };
}

export default function AddBookDialog({
  open,
  onOpenChange,
  onCreated,
  initialScan = false,
}: {
  initialScan?: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (workId: string) => void;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [isbnInput, setIsbnInput] = useState("");
  const [lookupNote, setLookupNote] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [duplicates, setDuplicates] = useState<WorkSummary[] | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [saveResult, setSaveResult] = useState<AddBookResult | null>(null);
  const [wantCopy, setWantCopy] = useState(true);
  const [scanOpen, setScanOpen] = useState(initialScan);

  const { data: locations } = useQuery({
    queryKey: ["locations"],
    queryFn: api.listLocations,
    enabled: open,
  });
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
    enabled: open,
  });
  const preferredCurrency =
    (settings?.["preferences"] as { currency?: string } | undefined)
      ?.currency ?? "RON";

  const set = (patch: Partial<Draft>) => setDraft((d) => ({ ...d, ...patch }));

  const lookup = useMutation({
    mutationFn: (raw: string) => api.lookupIsbn(raw),
    onSuccess: (result) => {
      if (result) {
        setDraft((d) => applyLookup(d, result));
        setLookupNote(
          result.cached
            ? "Found in local cache (Open Library data)."
            : "Metadata fetched from Open Library — review and correct as needed.",
        );
      } else {
        setLookupNote("No provider match. Fill the fields in manually.");
      }
    },
    onError: (err) => {
      const msg =
        err instanceof CommandError && err.kind === "network"
          ? "Lookup failed (offline?). Enter the book manually."
          : (err as Error).message;
      setLookupNote(msg);
    },
  });

  // Filled by the barcode scanner; auto-lookup follows so the user sees
  // metadata immediately (plan: "I scan an ISBN and see the metadata …
  // before adding a copy"). Offline, the lookup note offers manual entry.
  const applyScan = (result: ScanResult) => {
    const thirteen = result.isbn13 ?? "";
    const ten = result.isbn10 ?? "";
    setIsbnInput(thirteen || ten);
    setDraft((d) => ({ ...d, isbn13: thirteen, isbn10: ten }));
    lookup.mutate(thirteen || ten);
  };

  const save = useMutation({
    mutationFn: async (overrideDuplicates: boolean) => {
      // 1. Duplicate check first (friendly warning + user decision).
      if (!overrideDuplicates) {
        const isbn = parseIsbn(draft.isbn13 || draft.isbn10);
        const found = await api.findDuplicates(
          isbn
            ? {
                isbn: isbn.isbn13 ?? isbn.isbn10,
                title: null,
                authorName: null,
              }
            : {
                isbn: null,
                title: draft.title,
                authorName: draft.authorNames.split(";")[0] || null,
              },
        );
        if (found.length > 0) {
          setDuplicates(found);
          return null;
        }
      }
      setDuplicates(null);

      // 2. Price is parsed once, at the IPC edge.
      const priceMinor = draft.price.trim()
        ? parseMoneyToMinor(draft.price, draft.currency)
        : null;
      if (draft.price.trim() && priceMinor === null) {
        throw new CommandError(
          "validation",
          `Could not parse price: ${draft.price}`,
        );
      }

      // 3. ONE transactional command: work + edition + optional copy.
      //    A failure rolls back everything — no orphan work.
      return api.addBook({
        title: draft.title.trim(),
        subtitle: draft.subtitle.trim() || null,
        originalLanguage: draft.language.trim() || null,
        description: null,
        authorNames: draft.authorNames
          .split(";")
          .map((a) => a.trim())
          .filter(Boolean),
        isbn10:
          parseIsbn(draft.isbn10)?.isbn10 ?? (draft.isbn10.trim() || null),
        isbn13:
          parseIsbn(draft.isbn13)?.isbn13 ?? (draft.isbn13.trim() || null),
        publisher: draft.publisher.trim() || null,
        publishedAt: draft.publishedAt.trim() || null,
        language: draft.language.trim() || null,
        pageCount: draft.pageCount ? Number(draft.pageCount) : null,
        binding: null,
        copy: wantCopy
          ? {
              locationId: draft.locationId || null,
              conditionGrade: draft.conditionGrade,
              acquisitionType: draft.acquisitionType || null,
              acquiredAt: draft.acquiredAt || null,
              acquisition:
                priceMinor !== null
                  ? {
                      priceMinor,
                      currency: (
                        draft.currency || preferredCurrency
                      ).toUpperCase(),
                      acquiredAt: draft.acquiredAt || null,
                      acquisitionType: draft.acquisitionType || null,
                      store: draft.store.trim() || null,
                    }
                  : null,
            }
          : null,
        dropConflictingIsbn: overrideDuplicates,
      });
    },
    onSuccess: (result) => {
      if (!result) return;
      void queryClient.invalidateQueries({ queryKey: ["works"] });
      void queryClient.invalidateQueries({ queryKey: ["stats"] });
      if (result.warning) {
        // Keep the dialog open so the user reads what changed.
        setSaveResult(result);
        return;
      }
      reset();
      onOpenChange(false);
      onCreated(result.workId);
    },
    onError: (err) => {
      setFormError((err as Error).message);
    },
  });

  function reset() {
    setIsbnInput("");
    setLookupNote(null);
    setDraft({ ...emptyDraft, currency: preferredCurrency });
    setDuplicates(null);
    setFormError(null);
    setSaveResult(null);
  }

  const canSave = draft.title.trim().length > 0;
  const isbnValid =
    isbnInput.trim() === "" ||
    (parseIsbn(isbnInput) !== null && !lookup.isPending);

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(o) => {
          if (!o) reset();
          onOpenChange(o);
        }}
      >
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{tx("Add Book")}</DialogTitle>
            <DialogDescription>
              {tx(
                "Look up by ISBN (Open Library, works offline via cache or manual entry), then record the owned copy.",
              )}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="flex items-end gap-2">
              <div className="flex-1 space-y-1">
                <Label htmlFor="isbn">{tx("ISBN")}</Label>
                <Input
                  id="isbn"
                  placeholder="978-0-306-40615-7"
                  value={isbnInput}
                  onChange={(e) => setIsbnInput(e.target.value)}
                />
              </div>
              <Button
                variant="secondary"
                disabled={!isbnInput.trim() || !isbnValid || lookup.isPending}
                onClick={() => lookup.mutate(isbnInput)}
              >
                {lookup.isPending ? (
                  <Loader2 className="animate-spin" />
                ) : (
                  <Search />
                )}{" "}
                {tx("Look up")}
              </Button>
              <Button
                variant="secondary"
                aria-label={tx("Scan barcode")}
                title={tx("Scan barcode")}
                onClick={() => setScanOpen(true)}
              >
                <ScanBarcode />
              </Button>
            </div>
            {!isbnValid && (
              <p className="text-xs text-destructive">
                {tx("That ISBN checksum is not valid.")}
              </p>
            )}
            {lookupNote && (
              <p className="text-xs text-muted-foreground">{tx(lookupNote)}</p>
            )}

            {duplicates && duplicates.length > 0 && (
              <div
                className="rounded-md border border-amber-500/50 bg-amber-500/10 p-3 text-sm"
                role="alert"
              >
                <div className="flex items-center gap-2 font-medium">
                  <AlertTriangle className="h-4 w-4" />{" "}
                  {tx("Possible duplicates in your library")}
                </div>
                <ul className="mt-1 list-inside list-disc text-muted-foreground">
                  {duplicates.map((d) => (
                    <li key={d.id}>
                      {d.title} —{" "}
                      {d.authorNames.join(", ") || tx("unknown author")} (
                      {d.copyCount} {tx("copies)")}
                    </li>
                  ))}
                </ul>
                <div className="mt-2 flex gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => setDuplicates(null)}
                  >
                    {tx("Edit entry")}
                  </Button>
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => save.mutate(true)}
                  >
                    {tx("Add as a new work anyway")}
                  </Button>
                </div>
              </div>
            )}

            <div className="grid grid-cols-2 gap-3">
              <div className="col-span-2 space-y-1">
                <Label htmlFor="title">{tx("Title *")}</Label>
                <Input
                  id="title"
                  value={draft.title}
                  onChange={(e) => set({ title: e.target.value })}
                />
              </div>
              <div className="col-span-2 space-y-1">
                <Label htmlFor="subtitle">{tx("Subtitle")}</Label>
                <Input
                  id="subtitle"
                  value={draft.subtitle}
                  onChange={(e) => set({ subtitle: e.target.value })}
                />
              </div>
              <div className="col-span-2 space-y-1">
                <Label htmlFor="authors">
                  {tx("Authors (separate with semicolons)")}
                </Label>
                <Input
                  id="authors"
                  placeholder={tx("Ursula K. Le Guin")}
                  value={draft.authorNames}
                  onChange={(e) => set({ authorNames: e.target.value })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="publisher">{tx("Publisher")}</Label>
                <Input
                  id="publisher"
                  value={draft.publisher}
                  onChange={(e) => set({ publisher: e.target.value })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="publishedAt">
                  {tx("Published (year or date)")}
                </Label>
                <Input
                  id="publishedAt"
                  value={draft.publishedAt}
                  onChange={(e) => set({ publishedAt: e.target.value })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="isbn13">{tx("ISBN-13")}</Label>
                <Input
                  id="isbn13"
                  value={draft.isbn13}
                  onChange={(e) => set({ isbn13: e.target.value })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="isbn10">{tx("ISBN-10")}</Label>
                <Input
                  id="isbn10"
                  value={draft.isbn10}
                  onChange={(e) => set({ isbn10: e.target.value })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="pageCount">{tx("Pages")}</Label>
                <Input
                  id="pageCount"
                  inputMode="numeric"
                  value={draft.pageCount}
                  onChange={(e) =>
                    set({ pageCount: e.target.value.replace(/[^0-9]/g, "") })
                  }
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="language">{tx("Language")}</Label>
                <Input
                  id="language"
                  value={draft.language}
                  onChange={(e) => set({ language: e.target.value })}
                />
              </div>
            </div>

            <fieldset className="rounded-md border p-3">
              <legend className="px-1 text-sm font-medium">
                {tx("Owned copy")}
              </legend>
              <label className="mb-2 flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={wantCopy}
                  onChange={(e) => setWantCopy(e.target.checked)}
                />
                {tx("I own a copy (uncheck for tracking-only / wishlist)")}
              </label>
              {wantCopy && (
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label htmlFor="condition">{tx("Condition")}</Label>
                    <Select
                      value={draft.conditionGrade}
                      onValueChange={(v) => set({ conditionGrade: v })}
                    >
                      <SelectTrigger id="condition">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {CONDITION_GRADES.map((g) => (
                          <SelectItem key={g} value={g}>
                            {tx(CONDITION_LABELS[g])}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="location">{tx("Location")}</Label>
                    <Select
                      value={draft.locationId || "none"}
                      onValueChange={(v) =>
                        set({ locationId: v === "none" ? "" : v })
                      }
                    >
                      <SelectTrigger id="location">
                        <SelectValue placeholder={tx("Not set")} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="none">{tx("Not set")}</SelectItem>
                        {(locations ?? []).map((l) => (
                          <SelectItem key={l.id} value={l.id}>
                            {l.name} (
                            {tx(LOCATION_LABELS[l.type] ?? LOCATION_TYPES[0])})
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="acqType">{tx("Acquisition")}</Label>
                    <Select
                      value={draft.acquisitionType}
                      onValueChange={(v) => set({ acquisitionType: v })}
                    >
                      <SelectTrigger id="acqType">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {ACQUISITION_TYPES.map((t) => (
                          <SelectItem key={t} value={t}>
                            {tx(ACQUISITION_LABELS[t])}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="acquiredAt">{tx("Acquired on")}</Label>
                    <Input
                      id="acquiredAt"
                      type="date"
                      value={draft.acquiredAt}
                      onChange={(e) => set({ acquiredAt: e.target.value })}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="price">{tx("Purchase price")}</Label>
                    <Input
                      id="price"
                      inputMode="decimal"
                      placeholder="0.00"
                      value={draft.price}
                      onChange={(e) => set({ price: e.target.value })}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="currency">{tx("Currency")}</Label>
                    <Input
                      id="currency"
                      maxLength={3}
                      value={draft.currency}
                      onChange={(e) =>
                        set({ currency: e.target.value.toUpperCase() })
                      }
                    />
                  </div>
                  <div className="col-span-2 space-y-1">
                    <Label htmlFor="store">{tx("Store / seller")}</Label>
                    <Input
                      id="store"
                      value={draft.store}
                      onChange={(e) => set({ store: e.target.value })}
                    />
                  </div>
                </div>
              )}
            </fieldset>

            {formError && (
              <p role="alert" className="text-sm text-destructive">
                {formError}
              </p>
            )}
          </div>

          {saveResult?.warning && (
            <p
              role="status"
              className="rounded-md border border-amber-500/50 bg-amber-500/10 p-2 text-sm"
            >
              {saveResult.warning}
            </p>
          )}
          <DialogFooter>
            {saveResult ? (
              <Button
                onClick={() => {
                  const id = saveResult.workId;
                  reset();
                  onOpenChange(false);
                  onCreated(id);
                }}
              >
                {tx("Done")}
              </Button>
            ) : (
              <>
                <Button variant="outline" onClick={() => onOpenChange(false)}>
                  {tx("Cancel")}
                </Button>
                <Button
                  disabled={!canSave || save.isPending}
                  onClick={() => save.mutate(false)}
                >
                  {save.isPending && <Loader2 className="animate-spin" />}{" "}
                  {tx("Save book")}
                </Button>
              </>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ScanBarcodeDialog
        open={scanOpen}
        onOpenChange={setScanOpen}
        onScan={applyScan}
      />
    </>
  );
}

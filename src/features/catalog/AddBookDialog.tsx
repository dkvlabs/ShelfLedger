import { CurrencySelect } from "@/components/ui/currency-select";
import { useT } from "@/lib/i18n";
import { useRef, useState } from "react";
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
  currency: "",
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
  const [librisUrl, setLibrisUrl] = useState("");
  const [lookupNote, setLookupNote] = useState<{ key: string; value?: string } | null>(null);
  const lookupEpoch = useRef(0);
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

  const currency = draft.currency || preferredCurrency;

  const set = (patch: Partial<Draft>) => setDraft((d) => ({ ...d, ...patch }));

  const lookup = useMutation({
    mutationFn: (request: { isbn: string; url?: string; epoch: number }) =>
      request.url
        ? api.lookupLibrisBook(request.url, request.isbn.trim() || null)
        : api.lookupIsbn(request.isbn),
    onSuccess: (result, request) => {
      if (request.epoch !== lookupEpoch.current) return;
      if (result) {
        setDraft((d) => applyLookup(d, result));
        setIsbnInput(result.isbn13 ?? result.isbn10 ?? request.isbn);
        setLookupNote({
          key: result.cached
            ? "Found in local cache ({value}). Review the metadata before saving."
            : "Metadata fetched from {value}. Review and correct it before saving.",
          value: ({ openlibrary: "Open Library", googlebooks: "Google Books", libris: "Libris" } as Record<string, string>)[result.source] ?? result.source,
        });
      } else {
        setLookupNote({ key: request.url
          ? "No book metadata found, or online lookup is disabled. Enter the details manually."
          : "No match in Open Library or Google Books. Try a Libris book link or enter the details manually." });
      }
    },
    onError: (err, request) => {
      if (request.epoch !== lookupEpoch.current) return;
      const msg =
        err instanceof CommandError && err.kind === "network"
          ? "Lookup failed (offline or provider unavailable). Try a Libris book link or enter the details manually."
          : (err as Error).message;
      setLookupNote({ key: msg });
    },
  });

  function updateIsbn(raw: string) {
    lookupEpoch.current++;
    setIsbnInput(raw);
    setLookupNote(null);
    const parsed = parseIsbn(raw);
    setDraft((d) => ({ ...d, isbn13: parsed?.isbn13 ?? "", isbn10: parsed?.isbn10 ?? "" }));
  }

  function startLookup(raw: string, url?: string) {
    setLookupNote(null);
    lookup.mutate({ isbn: raw, url, epoch: ++lookupEpoch.current });
  }

  // Filled by the barcode scanner; auto-lookup follows so the user sees
  // metadata immediately (plan: "I scan an ISBN and see the metadata …
  // before adding a copy"). Offline, the lookup note offers manual entry.
  const applyScan = (result: ScanResult) => {
    const thirteen = result.isbn13 ?? "";
    const ten = result.isbn10 ?? "";
    updateIsbn(thirteen || ten);
    startLookup(thirteen || ten);
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
        ? parseMoneyToMinor(draft.price, currency)
        : null;
      if (draft.price.trim() && priceMinor === null) {
        throw new CommandError(
          "validation",
          tx("Invalid amount"),
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
                      currency: currency.toUpperCase(),
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
    lookupEpoch.current++;
    setIsbnInput("");
    setLibrisUrl("");
    setLookupNote(null);
    setDraft(emptyDraft);
    setDuplicates(null);
    setFormError(null);
    setSaveResult(null);
  }

  const isbnValid =
    isbnInput.trim() === "" ||
    parseIsbn(isbnInput) !== null;
  const canSave = draft.title.trim().length > 0 && isbnValid && !lookup.isPending;

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
                "Look up an ISBN with Open Library and Google Books, or import a Libris book link. Offline, use cached data or enter details manually.",
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
                  onChange={(e) => updateIsbn(e.target.value)}
                />
              </div>
              <Button
                variant="secondary"
                disabled={!isbnInput.trim() || !isbnValid || lookup.isPending}
                onClick={() => startLookup(isbnInput)}
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
            <div className="flex flex-col items-stretch gap-2 sm:flex-row sm:items-end">
              <div className="min-w-0 flex-1 space-y-1">
                <Label htmlFor="libris-url">{tx("Libris book link (optional)")}</Label>
                <Input id="libris-url" type="url" placeholder="https://www.libris.ro/carte/..."
                  value={librisUrl} onChange={(e) => {
                    lookupEpoch.current++;
                    setLookupNote(null);
                    setLibrisUrl(e.target.value);
                  }} />
              </div>
              <Button variant="secondary" disabled={!librisUrl.trim() || !isbnValid || lookup.isPending}
                onClick={() => startLookup(isbnInput, librisUrl.trim())}>
                {tx("Import from Libris")}
              </Button>
            </div>
            {lookupNote && (
              <p className="text-xs text-muted-foreground" role="status">{tx(lookupNote.key, lookupNote.value ? { value: lookupNote.value } : undefined)}</p>
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
                    <CurrencySelect id="currency" value={currency} onValueChange={(currency) => set({ currency })} />
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

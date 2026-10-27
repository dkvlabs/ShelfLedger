import { useT } from "@/lib/i18n";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Archive, BookPlus, BookOpen, Tag } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import {
  formatDate,
  formatMoney,
  parseMoneyToMinor,
  todayIso,
} from "@/lib/format";
import {
  ACQUISITION_LABELS,
  CONDITION_LABELS,
  OWNERSHIP_LABELS,
  READING_STATUS_LABELS,
} from "@/types/domain";
import SessionEditor from "@/features/reading/SessionEditor";
import EditBookDialog, { type EditTarget } from "./EditBookDialog";
import AddCopyDialog from "./AddCopyDialog";

function LendDialog({
  open,
  onOpenChange,
  copyId,
  identity,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  copyId: string;
  identity: string;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const { data: contacts } = useQuery({
    queryKey: ["contacts"],
    queryFn: api.listContacts,
  });
  const [contactId, setContactId] = useState("");
  const [newContactName, setNewContactName] = useState("");
  const [dueAt, setDueAt] = useState("");
  const [error, setError] = useState<string | null>(null);

  const lend = useMutation({
    mutationFn: async () => {
      let cid = contactId === "new" ? "" : contactId;
      if (!cid && newContactName.trim()) {
        const contact = await api.createContact({
          displayName: newContactName.trim(),
        });
        cid = contact.id;
      }
      if (!cid) throw new Error("pick a contact or enter a name");
      return api.lendCopy({
        copyId,
        contactId: cid,
        lentAt: todayIso(),
        dueAt: dueAt || null,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["work"] });
      void queryClient.invalidateQueries({ queryKey: ["loans"] });
      void queryClient.invalidateQueries({ queryKey: ["contacts"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setError(null);
      setNewContactName("");
      setDueAt("");
      onOpenChange(false);
    },
    onError: (err) => setError((err as Error).message),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{tx("Lend this copy")}</DialogTitle>
          <p className="text-sm text-primary">{identity}</p>
          <DialogDescription>
            {tx(
              "The copy becomes unavailable until you record its return. Nothing is sent to anyone — contact details stay local.",
            )}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label htmlFor="lend-contact">{tx("Contact")}</Label>
            <Select value={contactId || "new"} onValueChange={setContactId}>
              <SelectTrigger id="lend-contact">
                <SelectValue placeholder={tx("Choose or add")} />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="new">{tx("+ New contact…")}</SelectItem>
                {(contacts ?? []).map((c) => (
                  <SelectItem key={c.id} value={c.id}>
                    {c.displayName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {contactId === "" || contactId === "new" ? (
            <div className="space-y-1">
              <Label htmlFor="lend-new-contact">{tx("Name")}</Label>
              <Input
                id="lend-new-contact"
                value={newContactName}
                onChange={(e) => setNewContactName(e.target.value)}
              />
            </div>
          ) : null}
          <div className="space-y-1">
            <Label htmlFor="lend-due">{tx("Due date (optional)")}</Label>
            <Input
              id="lend-due"
              type="date"
              value={dueAt}
              onChange={(e) => setDueAt(e.target.value)}
            />
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {tx("Cancel")}
          </Button>
          <Button disabled={lend.isPending} onClick={() => lend.mutate()}>
            {tx("Lend copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ValuationDialog({
  open,
  onOpenChange,
  copyId,
  identity,
  defaultCurrency,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  copyId: string;
  identity: string;
  defaultCurrency: string;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [value, setValue] = useState("");
  const [currency, setCurrency] = useState(defaultCurrency);
  const [sourceType, setSourceType] = useState("manual");
  const [sourceNote, setSourceNote] = useState("");
  const [confidence, setConfidence] = useState("low");
  const [error, setError] = useState<string | null>(null);

  const add = useMutation({
    mutationFn: () => {
      const valueMinor = parseMoneyToMinor(value, currency);
      if (valueMinor == null)
        throw new Error(`could not parse value: ${value}`);
      return api.addValuation({
        copyId,
        valueMinor,
        currency,
        sourceType,
        sourceNote: sourceNote || null,
        observedAt: todayIso(),
        confidence,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["valuations", copyId] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setError(null);
      setValue("");
      setSourceNote("");
      onOpenChange(false);
    },
    onError: (err) => setError((err as Error).message),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{tx("Record a value observation")}</DialogTitle>
          <p className="text-sm text-primary">{identity}</p>
          <DialogDescription>
            {tx(
              "Value is time-bound evidence with a source — never a live quote.",
            )}
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label htmlFor="val-value">{tx("Estimated value")}</Label>
            <Input
              id="val-value"
              inputMode="decimal"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-currency">{tx("Currency")}</Label>
            <Input
              id="val-currency"
              maxLength={3}
              value={currency}
              onChange={(e) => setCurrency(e.target.value.toUpperCase())}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-source">{tx("Source")}</Label>
            <Select value={sourceType} onValueChange={setSourceType}>
              <SelectTrigger id="val-source">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="manual">{tx("My own estimate")}</SelectItem>
                <SelectItem value="comparable">
                  {tx("Comparable sale I verified")}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-confidence">{tx("Confidence")}</Label>
            <Select value={confidence} onValueChange={setConfidence}>
              <SelectTrigger id="val-confidence">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="low">{tx("Low")}</SelectItem>
                <SelectItem value="medium">{tx("Medium")}</SelectItem>
                <SelectItem value="high">{tx("High")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="col-span-2 space-y-1">
            <Label htmlFor="val-note">{tx("Source note (optional)")}</Label>
            <Input
              id="val-note"
              value={sourceNote}
              onChange={(e) => setSourceNote(e.target.value)}
            />
          </div>
          {error && (
            <p role="alert" className="col-span-2 text-sm text-destructive">
              {error}
            </p>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {tx("Cancel")}
          </Button>
          <Button
            disabled={add.isPending || !value.trim()}
            onClick={() => add.mutate()}
          >
            {tx("Save observation")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReadingSection({ workId }: { workId: string }) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [rating, setRating] = useState("");
  const { data: sessions } = useQuery({
    queryKey: ["reading-sessions-work", workId],
    queryFn: () => api.listReadingSessionsForWork(workId),
  });

  const start = useMutation({
    mutationFn: (status: string) =>
      api.createReadingSession({ workId, status, startedAt: null }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ["reading-sessions-work", workId],
      });
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const update = useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: Parameters<typeof api.updateReadingSession>[1];
    }) => api.updateReadingSession(id, input),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ["reading-sessions-work", workId],
      });
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });

  const hasActive = (sessions ?? []).some((s) =>
    ["reading", "want_to_read", "paused"].includes(s.status),
  );

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <CardTitle className="text-sm">{tx("Reading")}</CardTitle>
          {!hasActive && (
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => start.mutate("want_to_read")}
              >
                {tx("Want to read")}
              </Button>
              <Button size="sm" onClick={() => start.mutate("reading")}>
                <BookOpen /> {tx("Start reading")}
              </Button>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        {(start.error || update.error) && (
          <p role="alert" className="text-destructive">
            {(start.error || update.error)?.message}
          </p>
        )}
        {(sessions ?? []).length === 0 && (
          <p className="text-sm text-muted-foreground">
            {tx("No reading sessions yet.")}
          </p>
        )}
        {(sessions ?? []).map((s) => (
          <div key={s.id} className="rounded-md border p-2 text-sm">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <Badge
                  variant={s.status === "reading" ? "default" : "secondary"}
                >
                  {tx(READING_STATUS_LABELS[s.status] ?? s.status)}
                </Badge>{" "}
                <span className="text-xs text-muted-foreground">
                  {s.startedAt
                    ? `${tx("from")} ${formatDate(s.startedAt)}`
                    : tx("not started")}
                  {s.finishedAt
                    ? ` · ${tx("finished")} ${formatDate(s.finishedAt)}`
                    : ""}
                  {s.rating != null ? ` · ${s.rating}/5` : ""}
                </span>
              </div>
              {s.status === "reading" && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    update.mutate({ id: s.id, input: { status: "finished" } })
                  }
                >
                  {tx("Finish")}
                </Button>
              )}
              {s.status === "finished" && s.rating == null && (
                <div className="flex items-center gap-1">
                  <Input
                    className="h-7 w-16"
                    placeholder="0-5"
                    aria-label={`Rating for ${s.workTitle}`}
                    value={rating}
                    onChange={(e) =>
                      setRating(e.target.value.replace(/[^0-5]/g, ""))
                    }
                  />
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={!rating}
                    onClick={() =>
                      update.mutate({
                        id: s.id,
                        input: { rating: Number(rating) },
                      })
                    }
                  >
                    {tx("Rate")}
                  </Button>
                </div>
              )}
            </div>
            <SessionEditor key={`${s.id}:${s.updatedAt}`} session={s} />
            {s.review && (
              <p className="mt-1 text-xs text-muted-foreground">{s.review}</p>
            )}
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

export default function WorkDetail({
  workId,
  onBack,
}: {
  workId: string;
  onBack: () => void;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [editTarget, setEditTarget] = useState<EditTarget | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [addCopyOpen, setAddCopyOpen] = useState(false);
  const [lendCopyId, setLendCopyId] = useState<string | null>(null);
  const [valueCopyId, setValueCopyId] = useState<string | null>(null);
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });
  const defaultCurrency =
    (settings?.["preferences"] as { currency?: string } | undefined)
      ?.currency ?? "RON";
  const { data, isPending, isError, error } = useQuery({
    queryKey: ["work", workId],
    queryFn: () => api.getWork(workId),
  });

  const archive = useMutation({
    mutationFn: (copyId: string) => api.archiveCopy(copyId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["work", workId] });
      void queryClient.invalidateQueries({ queryKey: ["works"] });
      void queryClient.invalidateQueries({ queryKey: ["stats"] });
    },
  });
  const returnLoan = useMutation({
    mutationFn: (loanId: string) =>
      api.returnLoan(loanId, { returnedAt: todayIso() }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["work", workId] });
      void queryClient.invalidateQueries({ queryKey: ["loans"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const { data: loans } = useQuery({
    queryKey: ["loans", todayIso()],
    queryFn: () => api.listLoans(todayIso()),
  });

  if (isPending)
    return (
      <div className="p-6 text-sm text-muted-foreground">{tx("Loading…")}</div>
    );
  if (isError)
    return (
      <div className="p-6">
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft /> {tx("Back")}
        </Button>
        <p role="alert" className="mt-4 text-sm text-destructive">
          {(error as Error).message}
        </p>
      </div>
    );

  const { work, authors, editions } = data;
  const identityFor = (id: string) => {
    const edition = editions.find((e) => e.copies.some((c) => c.id === id));
    const copy = edition?.copies.find((c) => c.id === id);
    return `${data.work.title} · ${edition?.isbn13 || edition?.publisher || edition?.id.slice(0, 8)} · ${copy?.inventoryCode || id.slice(0, 8)}`;
  };
  const totalCopies = editions.reduce(
    (n, e) => n + e.copies.filter((c) => !c.archivedAt).length,
    0,
  );

  return (
    <div className="p-6">
      <Button variant="ghost" className="mb-4" onClick={onBack}>
        <ArrowLeft /> {tx("My Library")}
      </Button>

      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">{work.title}</h1>
          {work.subtitle && (
            <p className="text-muted-foreground">{work.subtitle}</p>
          )}
          <p className="mt-1 text-sm text-muted-foreground">
            {authors.map((a) => a.name).join(", ") || tx("Unknown author")}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => setEditTarget({ kind: "work" })}
          >
            {tx("Edit book")}
          </Button>
          <Button onClick={() => setAddCopyOpen(true)}>
            <BookPlus /> {tx("Add copy")}
          </Button>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="space-y-4 lg:col-span-1">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">{tx("Editions")}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              {editions.length === 0 && (
                <p className="text-muted-foreground">
                  {tx("No editions recorded.")}
                </p>
              )}
              {editions.map((e) => (
                <div key={e.id} className="rounded-md border p-2">
                  <div className="font-medium">
                    {e.publisher ?? tx("Unknown publisher")}
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() =>
                      setEditTarget({ kind: "edition", edition: e })
                    }
                  >
                    {tx("Edit edition")}
                  </Button>
                  <div className="text-xs text-muted-foreground">
                    {e.isbn13 ? `ISBN-13 ${e.isbn13}` : tx("No ISBN")}
                    {e.pageCount ? ` · ${e.pageCount} pp.` : ""}
                    {e.publishedAt ? ` · ${e.publishedAt}` : ""}
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
          <Button
            variant="outline"
            onClick={() => setEditTarget({ kind: "newEdition" })}
          >
            {tx("Add edition")}
          </Button>
          <ReadingSection workId={work.id} />
        </div>

        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">
              {tx("My copies (")}
              {totalCopies})
            </CardTitle>
            <label className="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
              />
              {tx("Show archived history")}
            </label>
          </CardHeader>
          <CardContent className="space-y-3">
            {totalCopies === 0 && (
              <p className="text-sm text-muted-foreground">
                {tx(
                  "No owned copies yet. Use “Add copy” to record one with condition, location, and purchase details.",
                )}
              </p>
            )}
            {editions.flatMap((edition) =>
              edition.copies
                .filter((c) => showArchived || !c.archivedAt)
                .map((copy) => {
                  const activeLoan = loans?.find(
                    (l) => l.copyId === copy.id && !l.returnedAt,
                  );

                  return (
                    <div key={copy.id} className="rounded-md border p-3">
                      <div className="flex items-start justify-between gap-2">
                        <div>
                          <div className="flex items-center gap-2">
                            <Badge variant="secondary">
                              {tx(
                                CONDITION_LABELS[copy.conditionGrade] ??
                                  copy.conditionGrade,
                              )}
                            </Badge>
                            <Badge variant="outline">
                              {copy.archivedAt
                                ? tx("Archived")
                                : (OWNERSHIP_LABELS[copy.ownershipStatus] ??
                                  copy.ownershipStatus)}
                            </Badge>
                            {copy.inventoryCode && (
                              <span className="text-xs text-muted-foreground">
                                #{copy.inventoryCode}
                              </span>
                            )}
                          </div>
                          <div className="mt-2 space-y-0.5 text-sm">
                            <div>
                              <span className="text-muted-foreground">
                                {tx("Location:")}
                              </span>{" "}
                              {copy.locationName ?? tx("Not set")}
                            </div>
                            <div>
                              <span className="text-muted-foreground">
                                {tx("Acquired:")}
                              </span>{" "}
                              {formatDate(
                                copy.acquiredAt ??
                                  copy.acquisition?.acquiredAt ??
                                  null,
                              )}
                              {copy.acquisitionType &&
                                ` (${tx(ACQUISITION_LABELS[copy.acquisitionType] ?? copy.acquisitionType)})`}
                            </div>
                            {copy.acquisition?.priceMinor != null && (
                              <div>
                                <span className="text-muted-foreground">
                                  {tx("Purchase price:")}
                                </span>{" "}
                                {formatMoney(
                                  copy.acquisition.priceMinor,
                                  copy.acquisition.currency,
                                )}
                                {copy.acquisition.store &&
                                  ` — ${copy.acquisition.store}`}
                              </div>
                            )}
                            {activeLoan && (
                              <div>
                                <span className="text-muted-foreground">
                                  {tx("Lent to:")}
                                </span>{" "}
                                {activeLoan.contactName}
                                {activeLoan.dueAt &&
                                  ` (${tx("due")} ${formatDate(activeLoan.dueAt)})`}
                              </div>
                            )}
                          </div>
                        </div>
                        <div className="flex flex-col gap-1">
                          {copy.archivedAt ? (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={async () => {
                                try {
                                  await api.unarchiveCopy(copy.id);
                                  void queryClient.invalidateQueries();
                                } catch (error) {
                                  window.alert((error as Error).message);
                                }
                              }}
                            >
                              {tx("Restore archived copy")}
                            </Button>
                          ) : (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() =>
                                setEditTarget({ kind: "copy", copy })
                              }
                            >
                              {tx("Edit copy")}
                            </Button>
                          )}
                          {copy.ownershipStatus === "owned" &&
                            !copy.archivedAt && (
                              <Button
                                size="sm"
                                variant="outline"
                                onClick={() => setLendCopyId(copy.id)}
                              >
                                <BookOpen /> {tx("Lend")}
                              </Button>
                            )}
                          {activeLoan && (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => returnLoan.mutate(activeLoan.id)}
                            >
                              {tx("Record return")}
                            </Button>
                          )}
                          {!copy.archivedAt && (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => setValueCopyId(copy.id)}
                            >
                              <Tag /> {tx("Value")}
                            </Button>
                          )}
                          {!copy.archivedAt && (
                            <Button
                              size="sm"
                              variant="ghost"
                              title={tx("Archive this copy (history is kept)")}
                              onClick={() => {
                                if (
                                  window.confirm(
                                    tx(
                                      "Archive this copy? Its history is kept and you can restore it.",
                                    ),
                                  )
                                )
                                  archive.mutate(copy.id);
                              }}
                            >
                              <Archive /> {tx("Archive")}
                            </Button>
                          )}
                        </div>
                      </div>
                      {copy.note && (
                        <p className="mt-2 text-sm text-muted-foreground">
                          {copy.note}
                        </p>
                      )}
                    </div>
                  );
                }),
            )}
          </CardContent>
        </Card>
      </div>

      {editTarget && (
        <EditBookDialog
          data={data}
          target={editTarget}
          onClose={() => setEditTarget(null)}
        />
      )}
      <AddCopyDialog
        open={addCopyOpen}
        onOpenChange={setAddCopyOpen}
        editions={editions}
      />
      {lendCopyId && (
        <LendDialog
          open
          onOpenChange={(o) => !o && setLendCopyId(null)}
          copyId={lendCopyId}
          identity={identityFor(lendCopyId)}
        />
      )}
      {valueCopyId && (
        <ValuationDialog
          open
          onOpenChange={(o) => !o && setValueCopyId(null)}
          copyId={valueCopyId}
          identity={identityFor(valueCopyId)}
          defaultCurrency={defaultCurrency}
        />
      )}
    </div>
  );
}

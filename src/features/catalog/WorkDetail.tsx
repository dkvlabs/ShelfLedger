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
import { formatDate, formatMoney, parseMoneyToMinor, todayIso } from "@/lib/format";
import {
  ACQUISITION_LABELS,
  CONDITION_LABELS,
  OWNERSHIP_LABELS,
  READING_STATUS_LABELS,
} from "@/types/domain";
import AddCopyDialog from "./AddCopyDialog";

function LendDialog({
  open,
  onOpenChange,
  copyId,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  copyId: string;
}) {
  const queryClient = useQueryClient();
  const { data: contacts } = useQuery({ queryKey: ["contacts"], queryFn: api.listContacts });
  const [contactId, setContactId] = useState("");
  const [newContactName, setNewContactName] = useState("");
  const [dueAt, setDueAt] = useState("");
  const [error, setError] = useState<string | null>(null);

  const lend = useMutation({
    mutationFn: async () => {
      let cid = contactId;
      if (!cid && newContactName.trim()) {
        const contact = await api.createContact({ displayName: newContactName.trim() });
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
          <DialogTitle>Lend this copy</DialogTitle>
          <DialogDescription>
            The copy becomes unavailable until you record its return. Nothing is sent to
            anyone — contact details stay local.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label htmlFor="lend-contact">Contact</Label>
            <Select value={contactId || "new"} onValueChange={setContactId}>
              <SelectTrigger id="lend-contact">
                <SelectValue placeholder="Choose or add" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="new">+ New contact…</SelectItem>
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
              <Label htmlFor="lend-new-contact">Name</Label>
              <Input
                id="lend-new-contact"
                value={newContactName}
                onChange={(e) => setNewContactName(e.target.value)}
              />
            </div>
          ) : null}
          <div className="space-y-1">
            <Label htmlFor="lend-due">Due date (optional)</Label>
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
            Cancel
          </Button>
          <Button
            disabled={lend.isPending}
            onClick={() => lend.mutate()}
          >
            Lend copy
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
  defaultCurrency,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  copyId: string;
  defaultCurrency: string;
}) {
  const queryClient = useQueryClient();
  const [value, setValue] = useState("");
  const [currency, setCurrency] = useState(defaultCurrency);
  const [sourceType, setSourceType] = useState("manual");
  const [sourceNote, setSourceNote] = useState("");
  const [confidence, setConfidence] = useState("low");
  const [error, setError] = useState<string | null>(null);

  const add = useMutation({
    mutationFn: () => {
      const valueMinor = parseMoneyToMinor(value);
      if (valueMinor == null) throw new Error(`could not parse value: ${value}`);
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
          <DialogTitle>Record a value observation</DialogTitle>
          <DialogDescription>
            Value is time-bound evidence with a source — never a live quote.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label htmlFor="val-value">Estimated value</Label>
            <Input
              id="val-value"
              inputMode="decimal"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-currency">Currency</Label>
            <Input
              id="val-currency"
              maxLength={3}
              value={currency}
              onChange={(e) => setCurrency(e.target.value.toUpperCase())}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-source">Source</Label>
            <Select value={sourceType} onValueChange={setSourceType}>
              <SelectTrigger id="val-source">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="manual">My own estimate</SelectItem>
                <SelectItem value="comparable">Comparable sale I verified</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label htmlFor="val-confidence">Confidence</Label>
            <Select value={confidence} onValueChange={setConfidence}>
              <SelectTrigger id="val-confidence">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="low">Low</SelectItem>
                <SelectItem value="medium">Medium</SelectItem>
                <SelectItem value="high">High</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="col-span-2 space-y-1">
            <Label htmlFor="val-note">Source note (optional)</Label>
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
            Cancel
          </Button>
          <Button disabled={add.isPending || !value.trim()} onClick={() => add.mutate()}>
            Save observation
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReadingSection({ workId }: { workId: string }) {
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
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions-work", workId] });
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: Parameters<typeof api.updateReadingSession>[1] }) =>
      api.updateReadingSession(id, input),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["reading-sessions-work", workId] });
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
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">Reading</CardTitle>
          {!hasActive && (
            <div className="flex gap-2">
              <Button size="sm" variant="outline" onClick={() => start.mutate("want_to_read")}>
                Want to read
              </Button>
              <Button size="sm" onClick={() => start.mutate("reading")}>
                <BookOpen /> Start reading
              </Button>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        {(sessions ?? []).length === 0 && (
          <p className="text-sm text-muted-foreground">No reading sessions yet.</p>
        )}
        {(sessions ?? []).map((s) => (
          <div key={s.id} className="rounded-md border p-2 text-sm">
            <div className="flex items-center justify-between">
              <div>
                <Badge variant={s.status === "reading" ? "default" : "secondary"}>
                  {READING_STATUS_LABELS[s.status] ?? s.status}
                </Badge>{" "}
                <span className="text-xs text-muted-foreground">
                  {s.startedAt ? `from ${formatDate(s.startedAt)}` : "not started"}
                  {s.finishedAt ? ` · finished ${formatDate(s.finishedAt)}` : ""}
                  {s.rating != null ? ` · ${s.rating}/5` : ""}
                </span>
              </div>
              {s.status === "reading" && (
                <Button size="sm" variant="outline" onClick={() => update.mutate({ id: s.id, input: { status: "finished" } })}>
                  Finish
                </Button>
              )}
              {s.status === "finished" && s.rating == null && (
                <div className="flex items-center gap-1">
                  <Input
                    className="h-7 w-16"
                    placeholder="0-5"
                    aria-label={`Rating for ${s.workTitle}`}
                    value={rating}
                    onChange={(e) => setRating(e.target.value.replace(/[^0-5]/g, ""))}
                  />
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={!rating}
                    onClick={() =>
                      update.mutate({ id: s.id, input: { rating: Number(rating) } })
                    }
                  >
                    Rate
                  </Button>
                </div>
              )}
            </div>
            {s.review && <p className="mt-1 text-xs text-muted-foreground">{s.review}</p>}
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
  const queryClient = useQueryClient();
  const [addCopyOpen, setAddCopyOpen] = useState(false);
  const [lendCopyId, setLendCopyId] = useState<string | null>(null);
  const [valueCopyId, setValueCopyId] = useState<string | null>(null);
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const defaultCurrency =
    (settings?.["preferences"] as { currency?: string } | undefined)?.currency ?? "RON";
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
    mutationFn: (loanId: string) => api.returnLoan(loanId, { returnedAt: todayIso() }),
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

  if (isPending) return <div className="p-6 text-sm text-muted-foreground">Loading…</div>;
  if (isError)
    return (
      <div className="p-6">
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft /> Back
        </Button>
        <p role="alert" className="mt-4 text-sm text-destructive">
          {(error as Error).message}
        </p>
      </div>
    );

  const { work, authors, editions } = data;
  const totalCopies = editions.reduce((n, e) => n + e.copies.length, 0);

  return (
    <div className="p-6">
      <Button variant="ghost" className="mb-4" onClick={onBack}>
        <ArrowLeft /> My Library
      </Button>

      <div className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">{work.title}</h1>
          {work.subtitle && <p className="text-muted-foreground">{work.subtitle}</p>}
          <p className="mt-1 text-sm text-muted-foreground">
            {authors.map((a) => a.name).join(", ") || "Unknown author"}
          </p>
        </div>
        <Button onClick={() => setAddCopyOpen(true)}>
          <BookPlus /> Add copy
        </Button>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="space-y-4 lg:col-span-1">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">Editions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              {editions.length === 0 && (
                <p className="text-muted-foreground">No editions recorded.</p>
              )}
              {editions.map((e) => (
                <div key={e.id} className="rounded-md border p-2">
                  <div className="font-medium">{e.publisher ?? "Unknown publisher"}</div>
                  <div className="text-xs text-muted-foreground">
                    {e.isbn13 ? `ISBN-13 ${e.isbn13}` : "No ISBN"}
                    {e.pageCount ? ` · ${e.pageCount} pp.` : ""}
                    {e.publishedAt ? ` · ${e.publishedAt}` : ""}
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
          <ReadingSection workId={work.id} />
        </div>

        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">My copies ({totalCopies})</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {totalCopies === 0 && (
              <p className="text-sm text-muted-foreground">
                No owned copies yet. Use “Add copy” to record one with condition, location,
                and purchase details.
              </p>
            )}
            {editions.flatMap((edition) =>
              edition.copies.map((copy) => {
                const activeLoan = loans?.find(
                  (l) => l.copyId === copy.id && !l.returnedAt,
                );
                return (
                  <div key={copy.id} className="rounded-md border p-3">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <div className="flex items-center gap-2">
                          <Badge variant="secondary">
                            {CONDITION_LABELS[copy.conditionGrade] ?? copy.conditionGrade}
                          </Badge>
                          <Badge variant="outline">
                            {OWNERSHIP_LABELS[copy.ownershipStatus] ?? copy.ownershipStatus}
                          </Badge>
                          {copy.inventoryCode && (
                            <span className="text-xs text-muted-foreground">
                              #{copy.inventoryCode}
                            </span>
                          )}
                        </div>
                        <div className="mt-2 space-y-0.5 text-sm">
                          <div>
                            <span className="text-muted-foreground">Location:</span>{" "}
                            {copy.locationName ?? "Not set"}
                          </div>
                          <div>
                            <span className="text-muted-foreground">Acquired:</span>{" "}
                            {formatDate(copy.acquiredAt ?? copy.acquisition?.acquiredAt ?? null)}
                            {copy.acquisitionType &&
                              ` (${ACQUISITION_LABELS[copy.acquisitionType] ?? copy.acquisitionType})`}
                          </div>
                          {copy.acquisition?.priceMinor != null && (
                            <div>
                              <span className="text-muted-foreground">Purchase price:</span>{" "}
                              {formatMoney(copy.acquisition.priceMinor, copy.acquisition.currency)}
                              {copy.acquisition.store && ` — ${copy.acquisition.store}`}
                            </div>
                          )}
                          {activeLoan && (
                            <div>
                              <span className="text-muted-foreground">Lent to:</span>{" "}
                              {activeLoan.contactName}
                              {activeLoan.dueAt && ` (due ${formatDate(activeLoan.dueAt)})`}
                            </div>
                          )}
                        </div>
                      </div>
                      <div className="flex flex-col gap-1">
                        {copy.ownershipStatus === "owned" && !copy.archivedAt && (
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setLendCopyId(copy.id)}
                          >
                            <BookOpen /> Lend
                          </Button>
                        )}
                        {activeLoan && (
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => returnLoan.mutate(activeLoan.id)}
                          >
                            Record return
                          </Button>
                        )}
                        {!copy.archivedAt && (
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setValueCopyId(copy.id)}
                          >
                            <Tag /> Value
                          </Button>
                        )}
                        {!copy.archivedAt && (
                          <Button
                            size="sm"
                            variant="ghost"
                            title="Archive this copy (history is kept)"
                            onClick={() => archive.mutate(copy.id)}
                          >
                            <Archive /> Archive
                          </Button>
                        )}
                      </div>
                    </div>
                    {copy.note && <p className="mt-2 text-sm text-muted-foreground">{copy.note}</p>}
                  </div>
                );
              }),
            )}
          </CardContent>
        </Card>
      </div>

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
        />
      )}
      {valueCopyId && (
        <ValuationDialog
          open
          onOpenChange={(o) => !o && setValueCopyId(null)}
          copyId={valueCopyId}
          defaultCurrency={defaultCurrency}
        />
      )}
    </div>
  );
}

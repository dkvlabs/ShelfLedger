import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Scale } from "lucide-react";
import { api } from "@/lib/api";
import {
  formatDate,
  minorToDecimal,
  parseMoneyToMinor,
  todayIso,
} from "@/lib/format";
import { useFormatMoney, useT } from "@/lib/i18n";
import { olxSearch, openOlx, simulateNet, validOlxUrl } from "@/lib/market";
import { CONDITION_LABELS } from "@/types/domain";
import type { EditionDto, MarketComparable } from "@/types/domain";
import { Button } from "@/components/ui/button";
import { CurrencySelect } from "@/components/ui/currency-select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function MarketComparisonDialog({
  edition,
  title,
  authors,
  onClose,
}: {
  edition: EditionDto;
  title: string;
  authors: string;
  onClose: () => void;
}) {
  const t = useT();
  const money = useFormatMoney();
  const client = useQueryClient();
  const key = ["market-comparisons", edition.id];
  const report = useQuery({
    queryKey: key,
    queryFn: () => api.listMarketComparisons(edition.id),
  });
  const [url, setUrl] = useState("");
  const [price, setPrice] = useState("");
  const [currency, setCurrency] = useState("RON");
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState("");
  const [checked, setChecked] = useState(false);
  const [externalError, setExternalError] = useState("");
  const [selectedCurrency, setSelectedCurrency] = useState("RON");
  const summaryCurrency = selectedCurrency;
  const summary = report.data?.summaries.find(
    (s) => s.currency === summaryCurrency,
  );
  const [asking, setAsking] = useState("");
  const [fee, setFee] = useState("");
  const [shipping, setShipping] = useState("");
  const copies = edition.copies.filter(
    (c) =>
      !c.archivedAt && ["owned", "lent", "listed"].includes(c.ownershipStatus),
  );
  const [copyId, setCopyId] = useState(copies[0]?.id ?? "");
  const [confidence, setConfidence] = useState<"low" | "medium" | "high">(
    "low",
  );
  const values = useQuery({
    queryKey: ["valuations", copyId],
    queryFn: () => api.listValuations(copyId),
    enabled: !!copyId,
  });
  const amount = parseMoneyToMinor(price, currency);
  const net = simulateNet(asking, fee, shipping, summaryCurrency);
  const reset = () => {
    setUrl("");
    setPrice("");
    setNote("");
    setChecked(false);
  };
  const save = useMutation({
    mutationFn: () =>
      api.saveMarketComparison({
        editionId: edition.id,
        listingUrl: url,
        askingMinor: amount!,
        currency,
        observedAt: date,
        conditionNote: note.trim() || null,
        editionChecked: checked,
      }),
    onSuccess: async () => {
      reset();
      await client.invalidateQueries({ queryKey: key });
    },
  });
  const include = useMutation({
    mutationFn: ({ id, included }: { id: string; included: boolean }) =>
      api.setMarketComparisonIncluded(id, included),
    onSuccess: () => client.invalidateQueries({ queryKey: key }),
  });
  const estimate = useMutation({
    mutationFn: () =>
      api.saveMarketEstimate({
        editionId: edition.id,
        copyId,
        currency: summaryCurrency,
        confidence,
      }),
    onSuccess: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["valuations", copyId] }),
        client.invalidateQueries({ queryKey: ["sell-queue"] }),
        client.invalidateQueries({ queryKey: ["finance-report"] }),
        client.invalidateQueries({ queryKey: ["dashboard"] }),
      ]);
    },
  });
  const launch = (value: string) => {
    setExternalError("");
    void openOlx(value).catch((e: Error) => setExternalError(e.message));
  };
  const edit = (row: MarketComparable) => {
    setUrl(row.listingUrl);
    setPrice(minorToDecimal(row.askingMinor, row.currency));
    setCurrency(row.currency);
    setDate(row.observedAt);
    setNote(row.conditionNote ?? "");
    setChecked(false);
    save.reset();
  };
  const busy = save.isPending || include.isPending || report.isFetching;
  const saved = values.data?.find(
    (v) =>
      v.sourceType === "comparable" &&
      v.sourceNote?.startsWith("OLX advertised"),
  );

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-4xl p-4 sm:p-7">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 pe-6">
            <Scale className="size-5 shrink-0" />
            {t("Compare local prices")}
          </DialogTitle>
          <DialogDescription>
            {title} · {edition.publisher ?? t("Unknown publisher")} ·{" "}
            {edition.isbn13 ?? edition.isbn10 ?? t("No ISBN")}
            {[edition.publishedAt, edition.language, edition.binding]
              .filter(Boolean)
              .map((v) => ` · ${v}`)
              .join("")}
          </DialogDescription>
        </DialogHeader>
        <div className="rounded-lg border border-primary/25 bg-primary/5 p-4">
          <p className="text-sm font-medium">
            {t("Advertised prices, not completed sales.")}
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            {t(
              "Search opens OLX in your browser. Check the edition, language, binding and condition before saving. Comparisons stay available offline.",
            )}
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={!edition.isbn13 && !edition.isbn10}
              onClick={() =>
                launch(olxSearch(edition.isbn13 ?? edition.isbn10 ?? ""))
              }
            >
              <ExternalLink />
              {t("Search OLX by ISBN")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => launch(olxSearch(`${title} ${authors}`))}
            >
              <ExternalLink />
              {t("Search OLX by title")}
            </Button>
          </div>
          {externalError && (
            <p role="alert" className="mt-2 text-sm text-destructive">
              {externalError}
            </p>
          )}
        </div>
        <div className="grid gap-6 md:grid-cols-2">
          <section
            className="min-w-0 space-y-3"
            aria-labelledby="comparison-entry"
          >
            <h2 id="comparison-entry" className="font-semibold">
              {t("Record a checked listing")}
            </h2>
            <form
              className="space-y-3"
              onSubmit={(event) => {
                event.preventDefault();
                if (
                  checked &&
                  amount !== null &&
                  amount >= 0 &&
                  validOlxUrl(url) &&
                  !busy
                )
                  save.mutate();
              }}
            >
              <div className="space-y-1">
                <Label htmlFor="market-url">{t("OLX listing URL")}</Label>
                <Input
                  id="market-url"
                  type="url"
                  required
                  maxLength={2048}
                  value={url}
                  onChange={(e) => {
                    setUrl(e.target.value);
                    setChecked(false);
                  }}
                  placeholder="https://www.olx.ro/d/oferta/…html"
                  dir="ltr"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-1">
                  <Label htmlFor="market-price">{t("Advertised price")}</Label>
                  <Input
                    id="market-price"
                    inputMode="decimal"
                    required
                    value={price}
                    onChange={(e) => setPrice(e.target.value)}
                    aria-invalid={!!price && (amount === null || amount < 0)}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="market-currency">{t("Currency")}</Label>
                  <CurrencySelect
                    id="market-currency"
                    value={currency}
                    onValueChange={setCurrency}
                  />
                </div>
              </div>
              <div className="space-y-1">
                <Label htmlFor="market-date">{t("Observed on")}</Label>
                <Input
                  id="market-date"
                  type="date"
                  required
                  value={date}
                  onChange={(e) => setDate(e.target.value)}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="market-note">
                  {t("Edition and condition notes")}
                </Label>
                <Input
                  id="market-note"
                  maxLength={2000}
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
              </div>
              <label className="flex items-start gap-2 text-xs leading-relaxed">
                <input
                  className="mt-1 shrink-0"
                  type="checkbox"
                  checked={checked}
                  onChange={(e) => setChecked(e.target.checked)}
                />
                {t("I checked the edition, language and condition.")}
              </label>
              {url && !validOlxUrl(url) && (
                <p className="text-xs text-destructive">
                  {t("Enter a public HTTPS OLX Romania listing URL.")}
                </p>
              )}
              <div className="flex flex-wrap gap-2">
                <Button
                  type="submit"
                  disabled={
                    busy ||
                    !checked ||
                    amount === null ||
                    amount < 0 ||
                    !date ||
                    !validOlxUrl(url)
                  }
                >
                  {save.isPending ? t("Saving…") : t("Save comparison")}
                </Button>
                <Button type="button" variant="ghost" onClick={reset}>
                  {t("Clear")}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                {t(
                  "Saving the same URL updates its observation. Exclude listings that no longer match.",
                )}
              </p>
            </form>
            <div className="space-y-2">
              <h3 className="text-sm font-medium">{t("Saved comparisons")}</h3>
              {report.isPending && <p className="text-sm">{t("Loading…")}</p>}
              {report.data?.comparables.length === 0 && (
                <p className="text-sm text-muted-foreground">
                  {t("No comparisons yet.")}
                </p>
              )}
              {report.data?.comparables.map((row) => (
                <article
                  key={row.id}
                  className="min-w-0 rounded-md border p-3 text-sm"
                >
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <bdi className="font-semibold">
                      {money(row.askingMinor, row.currency)}
                    </bdi>
                    <span className="text-xs text-muted-foreground">
                      {formatDate(row.observedAt)}
                    </span>
                  </div>
                  <button
                    className="mt-1 block max-w-full truncate text-start text-xs text-primary underline"
                    title={row.listingUrl}
                    onClick={() => launch(row.listingUrl)}
                  >
                    {row.listingUrl}
                  </button>
                  {row.conditionNote && (
                    <p className="mt-1 break-words text-xs text-muted-foreground">
                      {row.conditionNote}
                    </p>
                  )}
                  <div className="mt-2 flex items-center justify-between gap-2">
                    <label className="flex items-center gap-2 text-xs">
                      <input
                        type="checkbox"
                        checked={row.included}
                        disabled={busy || estimate.isPending}
                        onChange={(e) =>
                          include.mutate({
                            id: row.id,
                            included: e.target.checked,
                          })
                        }
                      />
                      {t("Include in estimate")}
                    </label>
                    <Button size="sm" variant="ghost" onClick={() => edit(row)}>
                      {t("Edit")}
                    </Button>
                  </div>
                </article>
              ))}
            </div>
          </section>
          <section
            className="min-w-0 space-y-4"
            aria-labelledby="comparison-summary"
          >
            <h2 id="comparison-summary" className="font-semibold">
              {t("Comparison summary")}
            </h2>
            {report.data?.summaries.length === 0 && (
              <p className="text-sm text-muted-foreground">
                {t("Include a checked listing to calculate an estimate.")}
              </p>
            )}
            {report.data?.summaries.map((s) => (
              <div
                key={s.currency}
                className="rounded-lg border bg-muted/30 p-4"
              >
                <p className="text-xs text-muted-foreground">
                  {s.currency} · {t("Included listings")}: {s.count}
                </p>
                <p className="mt-2 text-xs text-muted-foreground">
                  {t("Median asking price")}
                </p>
                <p className="mt-1 text-2xl font-semibold">
                  <bdi>{money(s.medianMinor, s.currency)}</bdi>
                </p>
                <p className="mt-2 text-xs">
                  {t("Price range")}:{" "}
                  <bdi>
                    {money(s.minMinor, s.currency)} –{" "}
                    {money(s.maxMinor, s.currency)}
                  </bdi>
                </p>
                {s.medianRounded && (
                  <p className="mt-2 text-xs text-muted-foreground">
                    {t("Median rounded down to the smallest currency unit.")}
                  </p>
                )}
              </div>
            ))}
            <div className="space-y-3 rounded-lg border p-4">
              <h3 className="text-sm font-medium">
                {t("Simulate an asking price")}
              </h3>
              <div className="space-y-1">
                <Label htmlFor="simulation-currency">{t("Currency")}</Label>
                <CurrencySelect
                  id="simulation-currency"
                  value={summaryCurrency}
                  onValueChange={(v) => {
                    setSelectedCurrency(v);
                    setAsking("");
                    setFee("");
                    setShipping("");
                    estimate.reset();
                  }}
                />
              </div>
              <Button
                size="sm"
                variant="outline"
                disabled={!summary || busy}
                onClick={() =>
                  setAsking(
                    minorToDecimal(summary!.medianMinor, summaryCurrency),
                  )
                }
              >
                {t("Use median")}
              </Button>
              <div className="space-y-1">
                <Label htmlFor="simulation-asking">{t("Asking price")}</Label>
                <Input
                  id="simulation-asking"
                  inputMode="decimal"
                  value={asking}
                  onChange={(e) => setAsking(e.target.value)}
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-1">
                  <Label htmlFor="simulation-fee">{t("Selling fee")}</Label>
                  <Input
                    id="simulation-fee"
                    inputMode="decimal"
                    value={fee}
                    placeholder="0"
                    onChange={(e) => setFee(e.target.value)}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="simulation-shipping">
                    {t("Shipping cost")}
                  </Label>
                  <Input
                    id="simulation-shipping"
                    inputMode="decimal"
                    value={shipping}
                    placeholder="0"
                    onChange={(e) => setShipping(e.target.value)}
                  />
                </div>
              </div>
              <p className="text-xs text-muted-foreground">
                {t(
                  "Blank costs mean zero. This simulation does not change your records.",
                )}
              </p>
              <p className="text-sm font-semibold" aria-live="polite">
                {t("Estimated net proceeds")}:{" "}
                <bdi>{money(net, summaryCurrency)}</bdi>
              </p>
              {(asking || fee || shipping) && net === null && (
                <p role="alert" className="text-xs text-destructive">
                  {t(
                    "Enter valid nonnegative amounts within the currency precision.",
                  )}
                </p>
              )}
            </div>
            <div className="space-y-3 rounded-lg border p-4">
              <h3 className="text-sm font-medium">
                {t("Save a comparison estimate")}
              </h3>
              <p className="text-xs text-muted-foreground">
                {t(
                  "Saves the included listings’ median, with source URLs, observation dates and your confidence.",
                )}
              </p>
              {copies.length === 0 ? (
                <p className="text-xs">
                  {t("Add an owned copy of this edition to save an estimate.")}
                </p>
              ) : (
                <>
                  <div className="space-y-1">
                    <Label htmlFor="estimate-copy">{t("Copy")}</Label>
                    <select
                      id="estimate-copy"
                      className="h-10 w-full rounded-md border bg-background px-3 text-sm"
                      value={copyId}
                      onChange={(e) => {
                        setCopyId(e.target.value);
                        estimate.reset();
                      }}
                    >
                      {copies.map((c) => (
                        <option key={c.id} value={c.id}>
                          {c.inventoryCode || c.id.slice(0, 8)} ·{" "}
                          {c.conditionGrade
                            ? t(
                                CONDITION_LABELS[
                                  c.conditionGrade as keyof typeof CONDITION_LABELS
                                ] ?? c.conditionGrade,
                              )
                            : t("Unknown")}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="estimate-confidence">
                      {t("Confidence")}
                    </Label>
                    <select
                      id="estimate-confidence"
                      className="h-10 w-full rounded-md border bg-background px-3 text-sm"
                      value={confidence}
                      onChange={(e) =>
                        setConfidence(e.target.value as typeof confidence)
                      }
                    >
                      <option value="low">{t("Low")}</option>
                      <option value="medium">{t("Medium")}</option>
                      <option value="high">{t("High")}</option>
                    </select>
                  </div>
                  <Button
                    disabled={!summary || !copyId || busy || estimate.isPending}
                    onClick={() => estimate.mutate()}
                  >
                    {estimate.isPending
                      ? t("Saving…")
                      : t("Save median estimate")}
                  </Button>
                  {estimate.isSuccess && (
                    <p role="status" className="text-xs text-primary">
                      {t("Estimate saved with its evidence.")}
                    </p>
                  )}
                </>
              )}
              {saved && (
                <details className="text-xs">
                  <summary className="cursor-pointer">
                    {t("Saved estimate evidence")} ·{" "}
                    {money(saved.valueMinor, saved.currency)} ·{" "}
                    {formatDate(saved.observedAt)} ·{" "}
                    {t(
                      { low: "Low", medium: "Medium", high: "High" }[
                        saved.confidence
                      ] ?? saved.confidence,
                    )}
                  </summary>
                  <p
                    className="mt-2 whitespace-pre-wrap break-words text-muted-foreground"
                    dir="auto"
                  >
                    {saved.sourceNote}
                  </p>
                </details>
              )}
            </div>
          </section>
        </div>
        {[report.error, save.error, include.error, estimate.error, values.error]
          .filter(Boolean)
          .map((error, index) => (
            <p key={index} role="alert" className="text-sm text-destructive">
              {(error as Error).message}
            </p>
          ))}
      </DialogContent>
    </Dialog>
  );
}

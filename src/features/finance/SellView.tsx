import { useT } from "@/lib/i18n";
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

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
import type { SellQueueItem } from "@/types/domain";

const DECISION_BADGES: Record<
  string,
  {
    label: string;
    variant: "default" | "secondary" | "destructive" | "outline";
  }
> = {
  sell_candidate: { label: "Consider selling", variant: "default" },
  keep_candidate: { label: "Consider keeping", variant: "secondary" },
  donate_candidate: { label: "Consider donating", variant: "outline" },
  needs_review: { label: "Insufficient data", variant: "destructive" },
};

/** What-if calculator: net = asking − fee − seller-paid shipping. */
function WhatIfCalculator({ item }: { item: SellQueueItem }) {
  const tx = useT();
  const [asking, setAsking] = useState("");
  const [fee, setFee] = useState("");
  const [shipping, setShipping] = useState("");
  const currency =
    item.latestValuation?.currency ?? item.historicalCost?.currency ?? "RON";
  const askingMinor = asking ? parseMoneyToMinor(asking, currency) : null;
  const feeMinor = fee ? parseMoneyToMinor(fee, currency) : null;
  const shipMinor = shipping ? parseMoneyToMinor(shipping, currency) : null;

  const net =
    askingMinor != null
      ? askingMinor - (feeMinor ?? 0) - (shipMinor ?? 0)
      : null;
  const gain =
    net != null &&
    item.historicalCost &&
    item.historicalCost.currency === currency
      ? net - item.historicalCost.minor
      : null;

  return (
    <div className="mt-3 rounded-md border p-3">
      <div className="text-xs font-medium">
        {tx("What-if sale calculator (your inputs only)")}
      </div>
      <div className="mt-2 grid grid-cols-3 gap-2">
        <div className="space-y-1">
          <Label htmlFor={`ask-${item.copyId}`}>{tx("Asking price")}</Label>
          <Input
            id={`ask-${item.copyId}`}
            inputMode="decimal"
            placeholder="0.00"
            value={asking}
            onChange={(e) => setAsking(e.target.value)}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor={`fee-${item.copyId}`}>{tx("Selling fee")}</Label>
          <Input
            id={`fee-${item.copyId}`}
            inputMode="decimal"
            placeholder="0.00"
            value={fee}
            onChange={(e) => setFee(e.target.value)}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor={`ship-${item.copyId}`}>
            {tx("Shipping (you pay)")}
          </Label>
          <Input
            id={`ship-${item.copyId}`}
            inputMode="decimal"
            placeholder="0.00"
            value={shipping}
            onChange={(e) => setShipping(e.target.value)}
          />
        </div>
      </div>
      <div className="mt-2 text-sm">
        {net == null ? (
          <span className="text-muted-foreground">
            {tx("Enter an asking price to see the estimate.")}
          </span>
        ) : (
          <>
            {tx("Estimated net:")}
            <strong>{formatMoney(net, currency)}</strong>
            {gain != null && (
              <>
                {" "}
                {tx("· Estimated gain/loss:")}{" "}
                <strong>{formatMoney(gain, currency)}</strong>
              </>
            )}
            {gain == null && item.historicalCost && (
              <span className="text-muted-foreground">
                {" "}
                {tx("· gain/loss unknown (cost currency differs)")}
              </span>
            )}
            <div className="mt-1 text-xs text-muted-foreground">
              {tx(
                "This is not a verified market price — it is your own input arithmetic.",
              )}
            </div>
          </>
        )}
      </div>
      <SellActions item={item} currency={currency} />
    </div>
  );
}

function ListForSaleDialog({
  item,
  currency,
  open,
  onOpenChange,
}: {
  item: SellQueueItem;
  currency: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [asking, setAsking] = useState("");
  const [platform, setPlatform] = useState("");
  const [error, setError] = useState<string | null>(null);

  const list = useMutation({
    mutationFn: () => {
      const askingMinor = parseMoneyToMinor(asking, currency);
      if (askingMinor == null || askingMinor < 0) {
        throw new Error(`could not parse asking price: ${asking}`);
      }
      return api.createListing({
        copyId: item.copyId,
        askingPriceMinor: askingMinor,
        currency,
        platform: platform.trim() || null,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sell-queue"] });
      setError(null);
      onOpenChange(false);
    },
    onError: (err) => setError((err as Error).message),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {tx("List for sale —")}
            {item.workTitle}
          </DialogTitle>
          <DialogDescription>
            {tx(
              "The asking price is your hoped-for price, not revenue. Record the actual sale afterwards so proceeds stay truthful.",
            )}
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label htmlFor="list-asking">
              {tx("Asking price (")}
              {currency})
            </Label>
            <Input
              id="list-asking"
              inputMode="decimal"
              value={asking}
              onChange={(e) => setAsking(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="list-platform">{tx("Platform (optional)")}</Label>
            <Input
              id="list-platform"
              placeholder={tx("e.g. local marketplace")}
              value={platform}
              onChange={(e) => setPlatform(e.target.value)}
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
            disabled={list.isPending || !asking.trim()}
            onClick={() => list.mutate()}
          >
            {tx("Create listing")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function SellActions({
  item,
  currency,
}: {
  item: SellQueueItem;
  currency: string;
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [listOpen, setListOpen] = useState(false);
  const [disposition, setDisposition] = useState("sold");
  const [salePrice, setSalePrice] = useState("");
  const [saleFee, setSaleFee] = useState("");
  const [saleShipping, setSaleShipping] = useState("");
  const [error, setError] = useState<string | null>(null);

  const record = useMutation({
    mutationFn: () => {
      const priceMinor =
        disposition === "sold" ? parseMoneyToMinor(salePrice, currency) : null;
      return api.recordDisposal({
        copyId: item.copyId,
        disposition: disposition as "sold" | "donated" | "traded" | "lost",
        date: todayIso(),
        salePriceMinor: priceMinor,
        feeMinor: saleFee ? parseMoneyToMinor(saleFee, currency) : null,
        shippingMinor: saleShipping
          ? parseMoneyToMinor(saleShipping, currency)
          : null,
        currency: priceMinor != null ? currency : null,
        notes: null,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sell-queue"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setOpen(false);
    },
    onError: (err) => setError((err as Error).message),
  });

  const override = useMutation({
    mutationFn: (value: string | null) =>
      api.setSellOverride(item.copyId, value),
    onSuccess: () =>
      void queryClient.invalidateQueries({ queryKey: ["sell-queue"] }),
  });

  const withdraw = useMutation({
    mutationFn: () => {
      if (!item.activeListing) throw new Error("no active listing");
      return api.withdrawListing(item.activeListing.id);
    },
    onSuccess: () =>
      void queryClient.invalidateQueries({ queryKey: ["sell-queue"] }),
    onError: (err) => setError((err as Error).message),
  });

  return (
    <>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={() => override.mutate("keep")}
        >
          {tx("Mark: keep")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => override.mutate("sell")}
        >
          {tx("Mark: sell")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => override.mutate("donate")}
        >
          {tx("Mark: donate")}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => override.mutate(null)}>
          {tx("Clear override")}
        </Button>
        {item.activeListing ? (
          <Button
            size="sm"
            variant="secondary"
            disabled={withdraw.isPending}
            onClick={() => withdraw.mutate()}
          >
            {tx("Withdraw listing (")}
            {formatMoney(
              item.activeListing.askingPriceMinor,
              item.activeListing.currency,
            )}
            )
          </Button>
        ) : (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setListOpen(true)}
          >
            {tx("List for sale…")}
          </Button>
        )}
        <Button
          size="sm"
          onClick={() => setOpen(true)}
          disabled={item.decision === "needs_review"}
        >
          {tx("Record sale/disposal…")}
        </Button>
      </div>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {tx("Record disposal —")}
              {item.workTitle}
            </DialogTitle>
            <DialogDescription>
              {tx(
                "The copy stays in your history with all its records; only its status changes.",
              )}
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label htmlFor="disposition">{tx("Disposition")}</Label>
              <Select value={disposition} onValueChange={setDisposition}>
                <SelectTrigger id="disposition">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="sold">{tx("Sold")}</SelectItem>
                  <SelectItem value="donated">{tx("Donated")}</SelectItem>
                  <SelectItem value="traded">{tx("Traded")}</SelectItem>
                  <SelectItem value="lost">{tx("Lost")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            {disposition === "sold" && (
              <>
                <div className="space-y-1">
                  <Label htmlFor="salePrice">
                    {tx("Sale price (")}
                    {currency})
                  </Label>
                  <Input
                    id="salePrice"
                    inputMode="decimal"
                    value={salePrice}
                    onChange={(e) => setSalePrice(e.target.value)}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="saleFee">{tx("Fee")}</Label>
                  <Input
                    id="saleFee"
                    inputMode="decimal"
                    value={saleFee}
                    onChange={(e) => setSaleFee(e.target.value)}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="saleShipping">{tx("Shipping paid")}</Label>
                  <Input
                    id="saleShipping"
                    inputMode="decimal"
                    value={saleShipping}
                    onChange={(e) => setSaleShipping(e.target.value)}
                  />
                </div>
              </>
            )}
            {error && (
              <p role="alert" className="col-span-2 text-sm text-destructive">
                {error}
              </p>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              {tx("Cancel")}
            </Button>
            <Button
              disabled={
                record.isPending ||
                (disposition === "sold" && !salePrice.trim())
              }
              onClick={() => record.mutate()}
            >
              {tx("Record")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ListForSaleDialog
        item={item}
        currency={currency}
        open={listOpen}
        onOpenChange={setListOpen}
      />
    </>
  );
}

export default function SellView() {
  const tx = useT();
  const {
    data: queue,
    isPending,
    isError,
    error,
  } = useQuery({
    queryKey: ["sell-queue"],
    queryFn: api.getSellQueue,
  });

  const groups: { decision: string; label: string }[] = [
    { decision: "sell_candidate", label: "Consider selling" },
    { decision: "donate_candidate", label: "Consider donating" },
    { decision: "needs_review", label: "Insufficient data" },
    { decision: "keep_candidate", label: "Consider keeping" },
  ];

  return (
    <div className="max-w-3xl space-y-6 p-6">
      <h1 className="text-xl font-semibold">{tx("Sell & Value")}</h1>
      <p className="text-sm text-muted-foreground">
        {tx(
          "Transparent rules over your own data — no AI, no invented prices. Every suggestion lists its reasons and can be overridden.",
        )}
      </p>
      {isError && (
        <p role="alert" className="text-sm text-destructive">
          {(error as Error).message}
        </p>
      )}
      {isPending && (
        <p className="text-sm text-muted-foreground">{tx("Loading…")}</p>
      )}
      {queue &&
        groups.map(({ decision, label }) => {
          const items = queue.filter((q) => q.decision === decision);
          if (items.length === 0) return null;
          return (
            <section key={decision} className="space-y-3">
              <h2 className="text-sm font-semibold">
                {label} ({items.length})
              </h2>
              {items.map((item) => (
                <Card key={item.copyId}>
                  <CardHeader className="pb-2">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <CardTitle className="text-sm">
                          {item.workTitle}
                        </CardTitle>
                        <div className="text-xs text-muted-foreground">
                          {item.authorNames.join(", ")} {tx("· copy")}
                          {item.copyNumber} {tx("of")} {item.totalCopiesOfWork}{" "}
                          · {item.conditionGrade.replace("_", " ")}
                          {item.historicalCost
                            ? ` · cost ${formatMoney(item.historicalCost.minor, item.historicalCost.currency)}`
                            : tx(" · cost unknown")}
                          {item.latestValuation
                            ? ` · latest estimate ${formatMoney(item.latestValuation.valueMinor, item.latestValuation.currency)} (${item.latestValuation.sourceType}, ${formatDate(item.latestValuation.observedAt)})`
                            : ""}
                        </div>
                      </div>
                      <Badge
                        variant={
                          DECISION_BADGES[decision]?.variant ?? "outline"
                        }
                      >
                        {tx(DECISION_BADGES[decision]?.label ?? decision)}
                      </Badge>
                    </div>
                  </CardHeader>
                  <CardContent>
                    <ul className="list-inside list-disc text-xs text-muted-foreground">
                      {item.reasons.map((r) => (
                        <li key={r}>{tx(r)}</li>
                      ))}
                    </ul>
                    <WhatIfCalculator item={item} />
                  </CardContent>
                </Card>
              ))}
            </section>
          );
        })}
    </div>
  );
}

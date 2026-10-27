import { useT } from "@/lib/i18n";
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";

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
import { parseMoneyToMinor, todayIso } from "@/lib/format";
import type { EditionDto } from "@/types/domain";
import {
  ACQUISITION_LABELS,
  ACQUISITION_TYPES,
  CONDITION_GRADES,
  CONDITION_LABELS,
  LOCATION_TYPES,
  LOCATION_LABELS,
} from "@/types/domain";

export default function AddCopyDialog({
  open,
  onOpenChange,
  editions,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  editions: EditionDto[];
}) {
  const tx = useT();
  const queryClient = useQueryClient();
  const [editionId, setEditionId] = useState<string>(editions[0]?.id ?? "");
  const [conditionGrade, setConditionGrade] = useState("good");
  const [locationId, setLocationId] = useState("");
  const [acquisitionType, setAcquisitionType] = useState("bought");
  const [acquiredAt, setAcquiredAt] = useState(todayIso());
  const [price, setPrice] = useState("");
  const [store, setStore] = useState("");
  const [error, setError] = useState<string | null>(null);

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
  const currency =
    (settings?.["preferences"] as { currency?: string } | undefined)
      ?.currency ?? "RON";

  const save = useMutation({
    mutationFn: async () => {
      const targetEdition =
        editions.find((e) => e.id === editionId) ?? editions[0];
      if (!targetEdition) {
        throw new CommandError("validation", "This work has no edition yet.");
      }
      const priceMinor = price.trim()
        ? parseMoneyToMinor(price, currency)
        : null;
      if (price.trim() && priceMinor === null) {
        throw new CommandError("validation", `Could not parse price: ${price}`);
      }
      await api.createCopy({
        editionId: targetEdition.id,
        locationId: locationId || null,
        conditionGrade,
        acquisitionType: acquisitionType || null,
        acquiredAt: acquiredAt || null,
        acquisition:
          priceMinor !== null
            ? {
                priceMinor,
                currency,
                acquiredAt: acquiredAt || null,
                acquisitionType,
                store: store.trim() || null,
              }
            : null,
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["work"] });
      void queryClient.invalidateQueries({ queryKey: ["works"] });
      void queryClient.invalidateQueries({ queryKey: ["stats"] });
      setError(null);
      setPrice("");
      setStore("");
      onOpenChange(false);
    },
    onError: (err) => setError((err as Error).message),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{tx("Add copy")}</DialogTitle>
          <DialogDescription>
            {tx(
              "Record another physical copy of this work with its condition and purchase details.",
            )}
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          {editions.length > 1 && (
            <div className="col-span-2 space-y-1">
              <Label htmlFor="edition">{tx("Edition")}</Label>
              <Select value={editionId} onValueChange={setEditionId}>
                <SelectTrigger id="edition">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {editions.map((e) => (
                    <SelectItem key={e.id} value={e.id}>
                      {e.publisher ?? tx("Unknown")}{" "}
                      {e.isbn13 ? `(${e.isbn13})` : tx("(no ISBN)")}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}
          <div className="space-y-1">
            <Label htmlFor="copy-condition">{tx("Condition")}</Label>
            <Select value={conditionGrade} onValueChange={setConditionGrade}>
              <SelectTrigger id="copy-condition">
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
            <Label htmlFor="copy-location">{tx("Location")}</Label>
            <Select
              value={locationId || "none"}
              onValueChange={(v) => setLocationId(v === "none" ? "" : v)}
            >
              <SelectTrigger id="copy-location">
                <SelectValue placeholder={tx("Not set")} />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">{tx("Not set")}</SelectItem>
                {(locations ?? []).map((l) => (
                  <SelectItem key={l.id} value={l.id}>
                    {l.name} ({tx(LOCATION_LABELS[l.type] ?? LOCATION_TYPES[0])}
                    )
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label htmlFor="copy-acqType">{tx("Acquisition")}</Label>
            <Select value={acquisitionType} onValueChange={setAcquisitionType}>
              <SelectTrigger id="copy-acqType">
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
            <Label htmlFor="copy-acquiredAt">{tx("Acquired on")}</Label>
            <Input
              id="copy-acquiredAt"
              type="date"
              value={acquiredAt}
              onChange={(e) => setAcquiredAt(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="copy-price">
              {tx("Purchase price (")}
              {currency})
            </Label>
            <Input
              id="copy-price"
              inputMode="decimal"
              placeholder="0.00"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="copy-store">{tx("Store / seller")}</Label>
            <Input
              id="copy-store"
              value={store}
              onChange={(e) => setStore(e.target.value)}
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
            disabled={save.isPending || editions.length === 0}
            onClick={() => save.mutate()}
          >
            {save.isPending && <Loader2 className="animate-spin" />}{" "}
            {tx("Save copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

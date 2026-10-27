import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/lib/api";
import { currencies, currencyName, minorToDecimal, parseMoneyToMinor } from "@/lib/format";
import { useT, useLanguage } from "@/lib/i18n";
import {
  CONDITION_GRADES,
  CONDITION_LABELS,
  ACQUISITION_TYPES,
  ACQUISITION_LABELS,
  type WorkDetail,
  type EditionDto,
  type CopyDto,
} from "@/types/domain";

type Target =
  | { kind: "work" }
  | { kind: "edition"; edition: EditionDto }
  | { kind: "newEdition" }
  | { kind: "copy"; copy: CopyDto };
export default function EditBookDialog({
  data,
  target,
  onClose,
}: {
  data: WorkDetail;
  target: Target;
  onClose: () => void;
}) {
  const t = useT();
  const language = useLanguage();
  const client = useQueryClient();
  const e = target.kind === "edition" ? target.edition : undefined;
  const c = target.kind === "copy" ? target.copy : undefined;
  const a = c?.acquisition;
  const [values, setValues] = useState<Record<string, string>>({
    title: data.work.title,
    subtitle: data.work.subtitle ?? "",
    authors: data.authors.map((a) => a.name).join("; "),
    originalLanguage: data.work.originalLanguage ?? "",
    description: data.work.description ?? "",
    isbn10: e?.isbn10 ?? "",
    isbn13: e?.isbn13 ?? "",
    publisher: e?.publisher ?? "",
    publishedAt: e?.publishedAt ?? "",
    language: e?.language ?? "",
    pageCount: e?.pageCount == null ? "" : String(e.pageCount),
    binding: e?.binding ?? "",
    inventoryCode: c?.inventoryCode ?? "",
    locationId: c?.locationId ?? "",
    conditionGrade: c?.conditionGrade ?? "good",
    acquiredAt: c?.acquiredAt?.slice(0, 10) ?? "",
    acquisitionType: c?.acquisitionType ?? "",
    note: c?.note ?? "",
    sentimentalScore:
      c?.sentimentalScore == null ? "" : String(c.sentimentalScore),
    currency: a?.currency ?? "RON",
    price:
      a?.priceMinor == null
        ? ""
        : minorToDecimal(a.priceMinor, a.currency ?? "RON"),
    shipping:
      a?.shippingMinor == null
        ? ""
        : minorToDecimal(a.shippingMinor, a.currency ?? "RON"),
    tax:
      a?.taxMinor == null
        ? ""
        : minorToDecimal(a.taxMinor, a.currency ?? "RON"),
    store: a?.store ?? "",
  });
  const { data: locations } = useQuery({
    queryKey: ["locations"],
    queryFn: api.listLocations,
  });
  const [tags, setTags] = useState<string | null>(null);
  const { data: oldTags } = useQuery({
    queryKey: ["work-tags", data.work.id],
    queryFn: () => api.listWorkTags(data.work.id),
    enabled: target.kind === "work",
  });
  const set = (key: string, value: string) =>
    setValues((v) => ({ ...v, [key]: value }));
  const nullable = (key: string) => values[key]?.trim() || null;
  const amount = (key: string) => {
    if (!values[key]) return null;
    const value = parseMoneyToMinor(values[key], values.currency);
    if (value === null || value < 0)
      throw new Error(
        t("Enter a valid non-negative amount for the selected currency."),
      );
    return value;
  };
  const save = useMutation({
    mutationFn: async () => {
      if (target.kind === "work") {
        await api.updateWork(data.work.id, {
          title: values.title,
          subtitle: nullable("subtitle"),
          authorNames: values.authors
            .split(";")
            .map((v) => v.trim())
            .filter(Boolean),
          originalLanguage: nullable("originalLanguage"),
          description: nullable("description"),
        });
        await api.setWorkTags(
          data.work.id,
          (tags ?? oldTags?.join(", ") ?? "")
            .split(",")
            .map((v) => v.trim())
            .filter(Boolean),
        );
      }
      if (target.kind === "edition" || target.kind === "newEdition") {
        const input = {
          workId: data.work.id,
          isbn10: nullable("isbn10"),
          isbn13: nullable("isbn13"),
          publisher: nullable("publisher"),
          publishedAt: nullable("publishedAt"),
          language: nullable("language"),
          pageCount: values.pageCount ? Number(values.pageCount) : null,
          binding: nullable("binding"),
        };
        if (target.kind === "edition")
          await api.updateEdition(target.edition.id, input);
        else await api.createEdition(input);
      }
      if (target.kind === "copy")
        await api.updateCopyDetails(target.copy.id, {
          editionId: target.copy.editionId,
          inventoryCode: nullable("inventoryCode"),
          locationId: nullable("locationId"),
          conditionGrade: values.conditionGrade,
          acquisitionType: nullable("acquisitionType"),
          acquiredAt: nullable("acquiredAt"),
          note: nullable("note"),
          sentimentalScore: values.sentimentalScore
            ? Number(values.sentimentalScore)
            : null,
          acquisition: {
            acquiredAt: nullable("acquiredAt"),
            priceMinor: amount("price"),
            shippingMinor: amount("shipping"),
            taxMinor: amount("tax"),
            currency: values.currency,
            store: nullable("store"),
            acquisitionType: nullable("acquisitionType"),
          },
        });
    },
    onSuccess: () => {
      void client.invalidateQueries();
      onClose();
    },
  });
  const field = (key: string, label: string, type = "text") => (
    <Label key={key} className="space-y-1">
      {t(label)}
      <Input
        type={type}
        value={values[key]}
        onChange={(ev) => set(key, ev.target.value)}
        required={key === "title"}
      />
    </Label>
  );
  const select = (
    key: string,
    label: string,
    options: { value: string; label: string }[],
  ) => (
    <Label key={key} className="space-y-1">
      {t(label)}
      <select
        className="block w-full rounded-md border bg-background p-2 text-sm"
        value={values[key]}
        onChange={(ev) => set(key, ev.target.value)}
      >
        <option value="">{t("Not set")}</option>
        {options.map((v) => (
          <option key={v.value} value={v.value}>
            {t(v.label)}
          </option>
        ))}
      </select>
    </Label>
  );
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <form
          onSubmit={(ev) => {
            ev.preventDefault();
            save.mutate();
          }}
        >
          <DialogHeader>
            <DialogTitle>
              {t(
                target.kind === "work"
                  ? "Edit book"
                  : target.kind === "copy"
                    ? "Edit copy"
                    : target.kind === "newEdition"
                      ? "Add edition"
                      : "Edit edition",
              )}{" "}
              — {data.work.title}
            </DialogTitle>
            <DialogDescription>
              {t(
                "Changes stay on this device. Ownership and reading history are kept separately.",
              )}
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 grid gap-4">
            {target.kind === "work" ? (
              <>
                {field("title", "Title")}
                {field("subtitle", "Subtitle")}
                {field("authors", "Authors (separate with semicolons)")}
                {field("originalLanguage", "Original language")}
                <Label>
                  {t("Description")}
                  <Textarea
                    value={values.description}
                    onChange={(e) => set("description", e.target.value)}
                  />
                </Label>
                <Label>
                  {t("Tags (comma separated)")}
                  <Input
                    value={tags ?? oldTags?.join(", ") ?? ""}
                    onChange={(e) => setTags(e.target.value)}
                  />
                </Label>
              </>
            ) : target.kind === "copy" ? (
              <>
                {field("inventoryCode", "Inventory code")}
                {select(
                  "conditionGrade",
                  "Condition",
                  CONDITION_GRADES.map((v) => ({
                    value: v,
                    label: CONDITION_LABELS[v],
                  })),
                )}
                {select(
                  "locationId",
                  "Location",
                  (locations ?? []).map((v) => ({
                    value: v.id,
                    label: v.name,
                  })),
                )}
                {field("acquiredAt", "Acquired", "date")}
                {select(
                  "acquisitionType",
                  "Acquisition type",
                  ACQUISITION_TYPES.map((v) => ({
                    value: v,
                    label: ACQUISITION_LABELS[v],
                  })),
                )}
                {select(
                  "currency",
                  "Currency",
                  Array.from(new Set([values.currency, ...currencies])).filter(Boolean).map((v) => ({ value: v, label: `${v} — ${currencyName(v, language)}` })),
                )}
                <div className="grid grid-cols-2 gap-3">
                  {field("price", "Purchase price")}
                  {field("shipping", "Shipping")}
                  {field("tax", "Tax")}
                  {field("store", "Store")}
                </div>
                {select(
                  "sentimentalScore",
                  "Sentimental score",
                  [0, 1, 2, 3, 4, 5].map((v) => ({
                    value: String(v),
                    label: String(v),
                  })),
                )}
                <Label>
                  {t("Note")}
                  <Textarea
                    value={values.note}
                    onChange={(e) => set("note", e.target.value)}
                  />
                </Label>
              </>
            ) : (
              <div className="grid grid-cols-2 gap-4">
                {field("isbn13", "ISBN-13")}
                {field("isbn10", "ISBN-10")}
                {field("publisher", "Publisher")}
                {field(
                  "publishedAt",
                  "Publication date (YYYY, YYYY-MM or YYYY-MM-DD)",
                )}
                {field("language", "Language")}
                {field("pageCount", "Page count", "number")}
                {field("binding", "Binding")}
              </div>
            )}
            {save.error && (
              <p role="alert" className="text-sm text-destructive">
                {save.error.message}
              </p>
            )}
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t("Cancel")}
            </Button>
            <Button disabled={save.isPending} type="submit">
              {t("Save changes")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
export type { Target as EditTarget };

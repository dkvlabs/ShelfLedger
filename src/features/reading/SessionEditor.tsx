import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  READING_STATUSES,
  READING_STATUS_LABELS,
  type ReadingSessionDto,
} from "@/types/domain";
import { api } from "@/lib/api";
import { useT } from "@/lib/i18n";

export default function SessionEditor({
  session,
}: {
  session: ReadingSessionDto;
}) {
  const t = useT();
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [status, setStatus] = useState(session.status);
  const [rating, setRating] = useState(
    session.rating == null ? "" : String(session.rating),
  );
  const [review, setReview] = useState(session.review ?? "");
  const [start, setStart] = useState(session.startedAt?.slice(0, 10) ?? "");
  const [end, setEnd] = useState(session.finishedAt?.slice(0, 10) ?? "");
  const save = useMutation({
    mutationFn: () =>
      api.updateReadingSession(session.id, {
        status,
        rating: rating === "" ? null : Number(rating),
        review,
        startedAt: start || null,
        finishedAt: end || null,
      }),
    onSuccess: () => {
      void client.invalidateQueries();
      setEditing(false);
    },
  });
  return (
    <div className="mt-3">
      {!editing ? (
        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
            {t("Edit reading session")}
          </Button>
          {(session.status === "want_to_read" ||
            session.status === "paused") && (
            <Button
              size="sm"
              onClick={() => {
                setStatus("reading");
                setEditing(true);
              }}
            >
              {t(
                session.status === "paused"
                  ? "Resume reading"
                  : "Start reading",
              )}
            </Button>
          )}
        </div>
      ) : (
        <form
          className="grid gap-3 rounded-md border p-3"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate();
          }}
        >
          <Label>
            {t("Status")}
            <select
              aria-label={t("Reading status")}
              className="mt-1 block w-full rounded-md border bg-background p-2"
              value={status}
              onChange={(e) => setStatus(e.target.value)}
            >
              {READING_STATUSES.map((v) => (
                <option key={v} value={v}>
                  {t(READING_STATUS_LABELS[v])}
                </option>
              ))}
            </select>
          </Label>
          <div className="grid grid-cols-2 gap-3">
            <Label>
              {t("Started")}
              <Input
                type="date"
                value={start}
                onChange={(e) => setStart(e.target.value)}
              />
            </Label>
            <Label>
              {t("Finished")}
              <Input
                type="date"
                value={end}
                onChange={(e) => setEnd(e.target.value)}
              />
            </Label>
          </div>
          <Label>
            {t("Rating")}
            <select
              className="mt-1 block w-full rounded-md border bg-background p-2"
              value={rating}
              onChange={(e) => setRating(e.target.value)}
            >
              <option value="">{t("Not rated")}</option>
              {[0, 1, 2, 3, 4, 5].map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
          </Label>
          <Label>
            {t("Review")}
            <Textarea
              value={review}
              onChange={(e) => setReview(e.target.value)}
            />
          </Label>
          {save.error && (
            <p role="alert" className="text-sm text-destructive">
              {save.error.message}
            </p>
          )}
          <div className="flex gap-2">
            <Button type="submit" size="sm" disabled={save.isPending}>
              {t("Save session")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => setEditing(false)}
            >
              {t("Cancel")}
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}

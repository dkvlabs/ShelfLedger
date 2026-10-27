import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import { formatDate } from "@/lib/format";
import { READING_STATUSES, READING_STATUS_LABELS } from "@/types/domain";

const STATUS_FILTERS = ["reading", "want_to_read", "finished", "paused", "dnf"] as const;

function SessionCard({ sessionId }: { sessionId: string }) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["reading-sessions"] });
    void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
  };

  const { data: sessions } = useQuery({
    queryKey: ["reading-sessions"],
    queryFn: () => api.listReadingSessions(null),
  });
  const session = sessions?.find((s) => s.id === sessionId);
  const [page, setPage] = useState("");

  const addProgress = useMutation({
    mutationFn: () =>
      api.addReadingProgress(sessionId, {
        page: page ? Number(page) : null,
        percent: null,
      }),
    onSuccess: () => {
      setPage("");
      invalidate();
    },
  });
  const finish = useMutation({
    mutationFn: (status: string) => api.updateReadingSession(sessionId, { status }),
    onSuccess: invalidate,
  });

  if (!session) return null;
  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{session.workTitle}</CardTitle>
          <Badge variant={session.status === "reading" ? "default" : "secondary"}>
            {READING_STATUS_LABELS[session.status] ?? session.status}
          </Badge>
        </div>
        <div className="text-xs text-muted-foreground">
          {session.startedAt ? `Started ${formatDate(session.startedAt)}` : "Not started"}
          {session.finishedAt ? ` · Finished ${formatDate(session.finishedAt)}` : ""}
          {session.rating != null ? ` · ${session.rating}/5` : ""}
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {session.review && <p className="text-sm text-muted-foreground">{session.review}</p>}
        {session.progressEvents.length > 0 && (
          <ul className="space-y-0.5 text-xs text-muted-foreground">
            {session.progressEvents.slice(-5).map((p) => (
              <li key={p.id}>
                {formatDate(p.occurredAt)} —{" "}
                {p.page != null ? `page ${p.page}` : `${p.percent}%`}
                {p.note ? ` (${p.note})` : ""}
              </li>
            ))}
          </ul>
        )}
        {session.status === "reading" && (
          <div className="flex items-end gap-2">
            <div className="w-24 space-y-1">
              <Label htmlFor={`page-${sessionId}`}>Page</Label>
              <Input
                id={`page-${sessionId}`}
                inputMode="numeric"
                value={page}
                onChange={(e) => setPage(e.target.value.replace(/[^0-9]/g, ""))}
              />
            </div>
            <Button
              size="sm"
              variant="secondary"
              disabled={!page || addProgress.isPending}
              onClick={() => addProgress.mutate()}
            >
              Log progress
            </Button>
            <Button size="sm" variant="outline" onClick={() => finish.mutate("paused")}>
              Pause
            </Button>
            <Button size="sm" onClick={() => finish.mutate("finished")}>
              Finish
            </Button>
          </div>
        )}
        {session.status === "paused" && (
          <Button size="sm" variant="secondary" onClick={() => finish.mutate("reading")}>
            Resume reading
          </Button>
        )}
      </CardContent>
    </Card>
  );
}

export default function ReadingView() {
  const [statusFilter, setStatusFilter] = useState<string>("reading");
  const { data: sessions, isPending } = useQuery({
    queryKey: ["reading-sessions", statusFilter],
    queryFn: () =>
      api.listReadingSessions(statusFilter === "all" ? null : statusFilter),
  });

  return (
    <div className="max-w-3xl space-y-4 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Reading</h1>
        <Select value={statusFilter} onValueChange={setStatusFilter}>
          <SelectTrigger className="w-44" aria-label="Filter by status">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All sessions</SelectItem>
            {STATUS_FILTERS.map((s) => (
              <SelectItem key={s} value={s}>
                {READING_STATUS_LABELS[s]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <p className="text-sm text-muted-foreground">
        Start or restart a book from its page in My Library — rereads and DNF restarts are
        kept as separate sessions.
      </p>
      {isPending ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : !sessions || sessions.length === 0 ? (
        <p className="text-sm text-muted-foreground">No sessions with this status.</p>
      ) : (
        <div className="space-y-3">
          {sessions.map((s) => (
            <SessionCard key={s.id} sessionId={s.id} />
          ))}
        </div>
      )}
    </div>
  );
}

// Re-exported for the work detail screen.
export { READING_STATUSES };

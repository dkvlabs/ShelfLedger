import { useT } from "@/lib/i18n";
import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatMoney, todayIso } from "@/lib/format";
import { StatCard } from "@/features/insights/InsightsView";

export default function DashboardView({
  onOpenWork,
  onNavigate,
}: {
  onOpenWork: (workId: string) => void;
  onNavigate: (view: "library" | "loans" | "sell" | "insights") => void;
}) {
  const tx = useT();
  const today = todayIso();
  const { data, isPending, isError, error } = useQuery({
    queryKey: ["dashboard", today],
    queryFn: () => api.getDashboard(today),
  });

  if (isPending)
    return (
      <div className="p-6 text-sm text-muted-foreground">{tx("Loading…")}</div>
    );
  if (isError)
    return (
      <div className="p-6">
        <p role="alert" className="text-sm text-destructive">
          {(error as Error).message}
        </p>
      </div>
    );

  return (
    <div className="space-y-6 p-6">
      <h1 className="text-xl font-semibold">{tx("Dashboard")}</h1>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard
          label={tx("Works cataloged")}
          value={data.workCount}
          onClick={() => onNavigate("library")}
        />
        <StatCard
          label={tx("Copies owned")}
          value={data.ownedCopyCount}
          onClick={() => onNavigate("library")}
        />
        <StatCard
          label={tx("Unread backlog")}
          value={data.unreadCount}
          onClick={() => onNavigate("insights")}
        />
        <StatCard
          label={`${tx("Active loans")}${data.overdueLoans > 0 ? ` (${data.overdueLoans} ${tx("overdue")})` : ""}`}
          value={data.activeLoans}
          onClick={() => onNavigate("loans")}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">{tx("Currently reading")}</CardTitle>
          </CardHeader>
          <CardContent>
            {data.currentlyReading.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                {tx(
                  "Nothing in progress. Start reading from any work in My Library.",
                )}
              </p>
            ) : (
              <ul className="space-y-1 text-sm">
                {data.currentlyReading.map((r) => (
                  <li key={r.sessionId}>
                    <button
                      type="button"
                      className="font-medium hover:underline"
                      onClick={() => onOpenWork(r.workId)}
                    >
                      {r.workTitle}
                    </button>
                    {r.latestPage != null && (
                      <span className="text-muted-foreground">
                        {" "}
                        {tx("— page")}
                        {r.latestPage}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">
              {tx("Money at a glance (per currency)")}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3 text-sm">
            <div>
              <div className="text-xs font-medium text-muted-foreground">
                {tx("Purchase spending")}
              </div>
              {data.spendingByCurrency.length === 0 ? (
                <span className="text-muted-foreground">
                  {tx("No costs recorded.")}
                </span>
              ) : (
                <div className="flex flex-wrap gap-1">
                  {data.spendingByCurrency.map((b) => (
                    <Badge key={b.currency} variant="secondary">
                      {formatMoney(b.totalMinor, b.currency)}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground">
                {tx("Sale proceeds")}
              </div>
              {data.proceedsByCurrency.length === 0 ? (
                <span className="text-muted-foreground">
                  {tx("No sales yet.")}
                </span>
              ) : (
                <div className="flex flex-wrap gap-1">
                  {data.proceedsByCurrency.map((b) => (
                    <Badge key={b.currency} variant="secondary">
                      {formatMoney(b.totalMinor, b.currency)}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground">
                {tx("Estimated value (owned)")}
              </div>
              {data.estimatedValueByCurrency.length === 0 ? (
                <span className="text-muted-foreground">
                  {tx("No estimates recorded.")}
                </span>
              ) : (
                <div className="flex flex-wrap gap-1">
                  {data.estimatedValueByCurrency.map((b) => (
                    <Badge key={b.currency} variant="outline">
                      {formatMoney(b.totalMinor, b.currency)}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      <p className="text-xs text-muted-foreground">
        {tx(
          "Estimated value reflects your own recorded observations with source and date — never a live market quote. See",
        )}
        <strong>{tx("Sell & Value")}</strong> {tx("for the decision queue.")}
      </p>
    </div>
  );
}

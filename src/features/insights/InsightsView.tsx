import { useQuery } from "@tanstack/react-query";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";

function MoneyBuckets({
  buckets,
  emptyLabel,
}: {
  buckets: { currency: string; totalMinor: number }[];
  emptyLabel: string;
}) {
  if (buckets.length === 0) {
    return <p className="text-sm text-muted-foreground">{emptyLabel}</p>;
  }
  return (
    <ul className="space-y-1 text-sm">
      {buckets.map((b) => (
        <li key={b.currency} className="flex justify-between">
          <span className="text-muted-foreground">{b.currency}</span>
          <span className="font-medium">{formatMoney(b.totalMinor, b.currency)}</span>
        </li>
      ))}
    </ul>
  );
}

export default function InsightsView() {
  const { data: report } = useQuery({
    queryKey: ["finance-report"],
    queryFn: api.getFinanceReport,
  });

  return (
    <div className="max-w-3xl space-y-6 p-6">
      <h1 className="text-xl font-semibold">Insights</h1>
      <p className="text-sm text-muted-foreground">
        Currencies are never summed together — every total is per currency.
      </p>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">Purchase spending by year</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {!report || report.spending.length === 0 ? (
              <p className="text-sm text-muted-foreground">No purchase costs recorded yet.</p>
            ) : (
              report.spending.map((y) => (
                <div key={y.year}>
                  <div className="text-xs font-medium text-muted-foreground">{y.year}</div>
                  <MoneyBuckets buckets={y.buckets} emptyLabel="" />
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">Sale proceeds by year</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {!report || report.proceeds.length === 0 ? (
              <p className="text-sm text-muted-foreground">No sales recorded yet.</p>
            ) : (
              report.proceeds.map((y) => (
                <div key={y.year}>
                  <div className="text-xs font-medium text-muted-foreground">{y.year}</div>
                  <MoneyBuckets buckets={y.buckets} emptyLabel="" />
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">Estimated collectible value (owned copies)</CardTitle>
          </CardHeader>
          <CardContent>
            <MoneyBuckets
              buckets={report?.estimatedValue ?? []}
              emptyLabel="No value estimates recorded yet."
            />
            <p className="mt-2 text-xs text-muted-foreground">
              Based on your most recent per-copy observations only — never a live quote.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

// Small stat card re-used by the dashboard view.
export function StatCard({
  label,
  value,
  onClick,
}: {
  label: string;
  value: number | string;
  onClick?: () => void;
}) {
  return (
    <Card
      className={onClick ? "cursor-pointer transition-colors hover:bg-accent" : undefined}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
      onClick={onClick}
      onKeyDown={(e) => {
        if (onClick && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onClick();
        }
      }}
    >
      <CardContent className="p-4">
        <div className="text-2xl font-semibold">{value}</div>
        <div className="text-xs text-muted-foreground">{label}</div>
      </CardContent>
    </Card>
  );
}

import { useEffect, useMemo, useState } from "react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import {
  BookOpen,
  Library,
  Settings as SettingsIcon,
  HandCoins,
  BookMarked,
  LayoutDashboard,
  FileUp,
} from "lucide-react";

import { api } from "@/lib/api";
import { applyTheme, type Theme } from "@/lib/theme";
import { translate, useLanguage } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import DashboardView from "@/features/dashboard/DashboardView";
import LibraryView from "@/features/catalog/LibraryView";
import WorkDetail from "@/features/catalog/WorkDetail";
import SettingsView from "@/features/settings/SettingsView";
import ReadingView from "@/features/reading/ReadingView";
import LoansView from "@/features/loans/LoansView";
import SellView from "@/features/finance/SellView";
import InsightsView from "@/features/insights/InsightsView";
import ImportView from "@/features/imports/ImportView";

type View =
  | { name: "dashboard" }
  | { name: "library" }
  | { name: "work"; workId: string }
  | { name: "reading" }
  | { name: "loans" }
  | { name: "sell" }
  | { name: "insights" }
  | { name: "import" }
  | { name: "settings" };

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5_000 },
  },
});

function SidebarButton({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
        active
          ? "bg-secondary text-secondary-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      )}
    >
      {icon}
      {label}
    </button>
  );
}

function Shell() {
  const [view, setView] = useState<View>({ name: "dashboard" });
  const language = useLanguage();
  const t = (key: string) => translate(language, key);

  // Apply the stored theme on mount and on change.
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  useEffect(() => {
    const theme = (settings?.["preferences"] as { theme?: Theme } | undefined)?.theme;
    applyTheme(theme ?? "system");
  }, [settings]);

  // Settings can hand off a chosen import file to the import screen.
  useEffect(() => {
    const handler = () => setView({ name: "import" });
    window.addEventListener("shelfledger:open-import", handler);
    return () => window.removeEventListener("shelfledger:open-import", handler);
  }, []);

  const navItems = useMemo(
    () =>
      [
        { key: "dashboard", label: t("nav.dashboard"), icon: <LayoutDashboard />, view: { name: "dashboard" } as View },
        { key: "library", label: t("nav.library"), icon: <Library />, view: { name: "library" } as View },
        { key: "reading", label: t("nav.reading"), icon: <BookOpen />, view: { name: "reading" } as View },
        { key: "loans", label: t("nav.loans"), icon: <BookMarked />, view: { name: "loans" } as View },
        { key: "sell", label: t("nav.sell"), icon: <HandCoins />, view: { name: "sell" } as View },
        { key: "insights", label: t("nav.insights"), icon: <BookOpen />, view: { name: "insights" } as View },
        { key: "import", label: t("settings.import"), icon: <FileUp />, view: { name: "import" } as View },
        { key: "settings", label: t("nav.settings"), icon: <SettingsIcon />, view: { name: "settings" } as View },
      ] satisfies { key: string; label: string; icon: React.ReactNode; view: View }[],
    [t],
  );

  const activeKey = view.name === "work" ? "library" : view.name;

  return (
    <div className="flex h-full">
      <nav aria-label="Main navigation" className="flex w-56 shrink-0 flex-col gap-1 border-r p-3">
        <div className="mb-4 px-3 pt-1">
          <div className="text-base font-bold tracking-tight">{t("app.title")}</div>
          <div className="text-xs text-muted-foreground">{t("app.tagline")}</div>
        </div>
        {navItems.map((item) => (
          <SidebarButton
            key={item.key}
            active={activeKey === item.key}
            onClick={() => setView(item.view)}
            icon={item.icon}
            label={item.label}
          />
        ))}
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto">
        {view.name === "dashboard" && (
          <DashboardView
            onOpenWork={(workId) => setView({ name: "work", workId })}
            onNavigate={(name) => setView({ name } as View)}
          />
        )}
        {view.name === "library" && (
          <LibraryView onOpenWork={(id) => setView({ name: "work", workId: id })} />
        )}
        {view.name === "work" && (
          <WorkDetail workId={view.workId} onBack={() => setView({ name: "library" })} />
        )}
        {view.name === "reading" && <ReadingView />}
        {view.name === "loans" && <LoansView />}
        {view.name === "sell" && <SellView />}
        {view.name === "insights" && <InsightsView />}
        {view.name === "import" && <ImportView />}
        {view.name === "settings" && <SettingsView />}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Shell />
    </QueryClientProvider>
  );
}

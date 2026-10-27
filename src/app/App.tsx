import { DirectionProvider } from "@radix-ui/react-direction";
import { useT } from "@/lib/i18n";
import { lazy, Suspense, useEffect, useState } from "react";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from "@tanstack/react-query";
import {
  BookOpen,
  Library,
  Settings as SettingsIcon,
  LayoutDashboard,
  FileUp,
  Menu,
  X,
  Users,
  Coins,
  Tag,
} from "lucide-react";

import { api } from "@/lib/api";
import { applyTheme, type Theme } from "@/lib/theme";
import { translate, useLanguage, languageInfo } from "@/lib/i18n";
import { cn } from "@/lib/utils";
const DashboardView = lazy(() => import("@/features/dashboard/DashboardView"));
const LibraryView = lazy(() => import("@/features/catalog/LibraryView"));
const WorkDetail = lazy(() => import("@/features/catalog/WorkDetail"));
const SettingsView = lazy(() => import("@/features/settings/SettingsView"));
const ReadingView = lazy(() => import("@/features/reading/ReadingView"));
const LoansView = lazy(() => import("@/features/loans/LoansView"));
const SellView = lazy(() => import("@/features/finance/SellView"));
const InsightsView = lazy(() => import("@/features/insights/InsightsView"));
const ImportView = lazy(() => import("@/features/imports/ImportView"));

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
        "sidebar-link flex w-full items-center gap-3 rounded-md text-sm font-medium transition-colors",
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
  const tx = useT();
  const [view, setView] = useState<View>({ name: "library" });
  const [mobileOpen, setMobileOpen] = useState(false);
  const [importPath, setImportPath] = useState<string | null>(null);
  const language = useLanguage();
  const t = (key: string) => translate(language, key);

  useEffect(() => {
    document.documentElement.lang = languageInfo(language).locale;
    document.documentElement.dir = languageInfo(language).direction;
  }, [language]);

  // Apply the stored theme on mount and on change.
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });
  useEffect(() => {
    const theme = (settings?.["preferences"] as { theme?: Theme } | undefined)
      ?.theme;
    return applyTheme(theme ?? "dark");
  }, [settings]);

  // Settings can hand off a chosen import file to the import screen.
  useEffect(() => {
    const handler = (event: Event) => {
      setImportPath(
        (event as CustomEvent<{ file?: string }>).detail?.file ?? null,
      );
      setView({ name: "import" });
    };
    window.addEventListener("shelfledger:open-import", handler);
    return () => window.removeEventListener("shelfledger:open-import", handler);
  }, []);

  const navItems = [
    {
      key: "dashboard",
      label: t("nav.dashboard"),
      icon: <LayoutDashboard />,
      view: { name: "dashboard" } as View,
    },
    {
      key: "library",
      label: t("nav.library"),
      icon: <Library />,
      view: { name: "library" } as View,
    },
    {
      key: "reading",
      label: t("nav.reading"),
      icon: <BookOpen />,
      view: { name: "reading" } as View,
    },
    {
      key: "loans",
      label: t("nav.loans"),
      icon: <Users />,
      view: { name: "loans" } as View,
    },
    {
      key: "sell",
      label: t("nav.sell"),
      icon: <Tag />,
      view: { name: "sell" } as View,
    },
    {
      key: "insights",
      label: t("nav.insights"),
      icon: <Coins />,
      view: { name: "insights" } as View,
    },
    {
      key: "import",
      label: t("settings.import"),
      icon: <FileUp />,
      view: { name: "import" } as View,
    },
    {
      key: "settings",
      label: t("nav.settings"),
      icon: <SettingsIcon />,
      view: { name: "settings" } as View,
    },
  ] satisfies {
    key: string;
    label: string;
    icon: React.ReactNode;
    view: View;
  }[];

  const activeKey = view.name === "work" ? "library" : view.name;

  return (
    <DirectionProvider dir={languageInfo(language).direction}>
    <div className="flex h-full flex-col">
      <div className="mobile-bar">
        <span className="brand font-display">{tx("ShelfLedger")}</span>
        <button
          aria-label={tx(mobileOpen ? "Close navigation" : "Open navigation")}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen(!mobileOpen)}
        >
          {mobileOpen ? <X /> : <Menu />}
        </button>
      </div>
      <div className="flex min-h-0 flex-1">
        <nav
          aria-label={tx("Main navigation")}
          data-open={mobileOpen}
          className="app-sidebar flex shrink-0 flex-col gap-2"
        >
          <div className="brand font-display mb-6 flex items-center gap-3 px-3">
            <BookOpen className="h-8 w-8 text-primary" strokeWidth={1} />
            <span>{tx("ShelfLedger")}</span>
          </div>
          {navItems.map((item) => (
            <SidebarButton
              key={item.key}
              active={activeKey === item.key}
              onClick={() => {
                setView(item.view);
                setMobileOpen(false);
              }}
              icon={item.icon}
              label={item.label}
            />
          ))}
          <div className="mt-auto border-t px-3 pt-6 text-xs leading-relaxed text-muted-foreground">
            <span className="me-2 text-primary">●</span>
            {t("Offline — all data on this device")}
            <p className="ms-5 mt-1 opacity-70">
              {t("Your library stays private.")}
            </p>
          </div>
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto">
          <Suspense fallback={<p className="p-8">{t("common.loading")}</p>}>
            <section key={view.name} className="page-enter">
              {view.name === "dashboard" && (
                <DashboardView
                  onOpenWork={(workId) => setView({ name: "work", workId })}
                  onNavigate={(name) => setView({ name } as View)}
                />
              )}
              {view.name === "library" && (
                <LibraryView
                  onOpenWork={(id) => setView({ name: "work", workId: id })}
                />
              )}
              {view.name === "work" && (
                <WorkDetail
                  workId={view.workId}
                  onBack={() => setView({ name: "library" })}
                />
              )}
              {view.name === "reading" && <ReadingView />}
              {view.name === "loans" && <LoansView />}
              {view.name === "sell" && <SellView />}
              {view.name === "insights" && <InsightsView />}
              {view.name === "import" && (
                <ImportView initialPath={importPath} />
              )}
              {view.name === "settings" && <SettingsView />}
            </section>
          </Suspense>
        </main>
      </div>
    </div>
    </DirectionProvider>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Shell />
    </QueryClientProvider>
  );
}

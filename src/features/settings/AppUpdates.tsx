import { useMutation, useQuery } from "@tanstack/react-query";
import { Download, ExternalLink, Loader2, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { api } from "@/lib/api";
import { useT } from "@/lib/i18n";
import { openUpdatePage, type UpdatePage } from "@/lib/updates";

export default function AppUpdates() {
  const t = useT();
  const info = useQuery({
    queryKey: ["app-info"],
    queryFn: api.getAppInfo,
    staleTime: Infinity,
    networkMode: "always",
    retry: false,
  });
  const check = useMutation({
    mutationFn: api.checkForUpdates,
    // IPC must run even when Android reports offline, so failures can be shown.
    networkMode: "always",
    retry: false,
  });
  const open = useMutation({
    mutationFn: (page: UpdatePage) => openUpdatePage(page),
    networkMode: "always",
    retry: false,
  });
  const result = check.data;
  const android = info.data?.platform === "android";

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("About ShelfLedger")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="space-y-1">
            <p className="text-sm text-muted-foreground">{t("Installed version")}</p>
            <p className="text-lg font-semibold" dir="ltr">
              {info.data ? `v${info.data.version}` : "—"}
            </p>
          </div>
          <Button
            variant="outline"
            disabled={check.isPending || !info.data}
            onClick={() => { open.reset(); check.mutate(); }}
          >
            {check.isPending ? <Loader2 className="animate-spin" /> : <RefreshCw />}
            {t(check.isPending ? "Checking for updates…" : "Check for updates")}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">
          {t("Checks GitHub only when you press the button. No library data is sent.")}
        </p>
        {check.isPending && <p role="status" className="text-sm">{t("Checking for updates…")}</p>}
        {result && !check.isPending && !check.isError && (
          <div className="space-y-3 rounded-md border p-3">
            <p role="status" className="text-sm font-medium">
              {result.updateAvailable
                ? t("Version {value} is available.", { value: result.latestVersion })
                : t("You're up to date.")}
            </p>
            <p className="text-xs text-muted-foreground">
              {t("Latest release: {value}", { value: result.latestVersion })}
            </p>
            {!result.downloadAvailable && (
              <p className="text-sm text-muted-foreground">
                {t(android
                  ? "No Android APK is published for this release. See Android builds for downloads."
                  : "A download for this device is not published in this release.")}
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              <Button variant="secondary" disabled={open.isPending} onClick={() => open.mutate("release")}>
                <ExternalLink />{t("View release")}
              </Button>
              {android && !result.downloadAvailable && (
                <Button variant="outline" disabled={open.isPending} onClick={() => open.mutate("android_builds")}>
                  <Download />{t("Android builds")}
                </Button>
              )}
            </div>
            <p className="text-xs text-muted-foreground">
              {t("Download and install updates from GitHub.")}
            </p>
          </div>
        )}
        {info.isError && <p role="alert" className="text-sm text-destructive">{t("Could not read the installed version.")}</p>}
        {check.isError && (
          <p role="alert" className="text-sm text-destructive">
            {t("Could not check for updates. Check your internet connection and try again.")}
          </p>
        )}
        {open.isError && <p role="alert" className="text-sm text-destructive">{t("Could not open the download page. Try again.")}</p>}
      </CardContent>
    </Card>
  );
}

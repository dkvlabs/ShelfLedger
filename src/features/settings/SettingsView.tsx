import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, Plus, Trash2 } from "lucide-react";

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
import { api, isDesktop } from "@/lib/api";
import { downloadExport, pickFile as chooseFile } from "@/lib/files";
import { applyTheme, type Theme } from "@/lib/theme";
import { translate, useLanguage, LANGUAGES, type Language } from "@/lib/i18n";
import { LOCATION_LABELS, LOCATION_TYPES, type LocationDto } from "@/types/domain";

const CURRENCIES = ["RON", "EUR", "USD", "GBP"];

export default function SettingsView() {
  const queryClient = useQueryClient();
  const language = useLanguage();
  const t = (key: string) => translate(language, key);
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const { data: locations } = useQuery({ queryKey: ["locations"], queryFn: api.listLocations });

  const preferences = (settings?.["preferences"] as
    | { currency?: string; theme?: Theme }
    | undefined) ?? {};
  const [newLocationName, setNewLocationName] = useState("");
  const [newLocationType, setNewLocationType] = useState<(typeof LOCATION_TYPES)[number]>("room");
  const [newLocationParent, setNewLocationParent] = useState("");
  const [locationError, setLocationError] = useState<string | null>(null);
  const [dataNote, setDataNote] = useState<string | null>(null);
  const [dataError, setDataError] = useState<string | null>(null);
  const [restorePath, setRestorePath] = useState<string | null>(null);
  // Last export/backup produced in this session; in the web version it is
  // offered as a browser download (the file lives on the local server).
  const [lastExport, setLastExport] = useState<string | null>(null);

  const setPreferences = useMutation({
    mutationFn: (prefs: Record<string, unknown>) => {
      const merged = {
        currency: preferences.currency ?? "RON",
        theme: preferences.theme ?? "system",
        language: language,
        ...prefs,
      };
      return api.setSetting("preferences", merged);
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  const setCurrency = useMutation({
    mutationFn: (currency: string) => setPreferences.mutateAsync({ currency }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  const setTheme = useMutation({
    mutationFn: (theme: Theme) => {
      applyTheme(theme);
      return setPreferences.mutateAsync({ theme });
    },
  });

  const setLanguage = useMutation({
    mutationFn: (lang: Language) => setPreferences.mutateAsync({ language: lang }),
  });

  const addLocation = useMutation({
    mutationFn: () =>
      api.createLocation({
        name: newLocationName,
        type: newLocationType,
        parentId: newLocationParent || null,
        sortOrder: null,
      }),
    onSuccess: () => {
      setNewLocationName("");
      setNewLocationParent("");
      setLocationError(null);
      void queryClient.invalidateQueries({ queryKey: ["locations"] });
    },
    onError: (err) => setLocationError((err as Error).message),
  });

  const deleteLocation = useMutation({
    mutationFn: (id: string) => api.deleteLocation(id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["locations"] }),
    onError: (err) => setLocationError((err as Error).message),
  });

  const locationsById = new Map((locations ?? []).map((l) => [l.id, l]));
  const locationLabel = (l: LocationDto) => {
    const parts = [l.name];
    let parent = l.parentId ? locationsById.get(l.parentId) : undefined;
    while (parent) {
      parts.unshift(parent.name);
      parent = parent.parentId ? locationsById.get(parent.parentId) : undefined;
    }
    return parts.join(" › ");
  };

  return (
    <div className="max-w-3xl space-y-6 p-6">
      <h1 className="text-xl font-semibold">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Preferences</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="max-w-xs space-y-1">
            <Label htmlFor="currency">Preferred currency</Label>
            <Select value={preferences.currency ?? "RON"} onValueChange={(v) => setCurrency.mutate(v)}>
              <SelectTrigger id="currency">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CURRENCIES.map((c) => (
                  <SelectItem key={c} value={c}>
                    {c}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              Used as the default for purchase entries. Any ISO 4217 code is accepted
              per-record; currencies are never summed together.
            </p>
          </div>
          <div className="flex gap-4">
            <div className="max-w-xs space-y-1">
              <Label htmlFor="language">{t("settings.language")}</Label>
              <Select
                value={language}
                onValueChange={(v) => setLanguage.mutate(v as Language)}
              >
                <SelectTrigger id="language">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {LANGUAGES.map((l) => (
                    <SelectItem key={l.value} value={l.value}>
                      {l.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="max-w-xs space-y-1">
              <Label htmlFor="theme">{t("settings.theme")}</Label>
              <Select
                value={preferences.theme ?? "system"}
                onValueChange={(v) => setTheme.mutate(v as Theme)}
              >
                <SelectTrigger id="theme">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="system">{t("settings.theme.system")}</SelectItem>
                  <SelectItem value="light">{t("settings.theme.light")}</SelectItem>
                  <SelectItem value="dark">{t("settings.theme.dark")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">{t("settings.data")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              onClick={async () => {
                try {
                  const res = await api.exportLibraryCsv();
                  setLastExport(res.path);
                  setDataNote(`CSV written to ${res.path}`);
                  setDataError(null);
                } catch (err) {
                  setDataError((err as Error).message);
                }
              }}
            >
              {t("settings.exportCsv")}
            </Button>
            <Button
              variant="outline"
              onClick={async () => {
                try {
                  const res = await api.exportLibraryJson();
                  setLastExport(res.path);
                  setDataNote(`JSON written to ${res.path}`);
                  setDataError(null);
                } catch (err) {
                  setDataError((err as Error).message);
                }
              }}
            >
              {t("settings.exportJson")}
            </Button>
            <Button
              variant="outline"
              onClick={async () => {
                try {
                  const res = await api.createBackup(null);
                  setLastExport(res.path);
                  setDataNote(`Backup written to ${res.path}`);
                  setDataError(null);
                } catch (err) {
                  setDataError((err as Error).message);
                }
              }}
            >
              {t("settings.backup")}
            </Button>
            <Button
              variant="outline"
              onClick={async () => {
                const file = await chooseFile(["zip"]);
                if (file) setRestorePath(file);
              }}
            >
              {t("settings.restore")}
            </Button>
            <Button
              variant="outline"
              onClick={async () => {
                const file = await chooseFile(["csv", "json"]);
                if (file) {
                  window.dispatchEvent(
                    new CustomEvent("shelfledger:open-import", { detail: file }),
                  );
                }
              }}
            >
              {t("settings.import")}
            </Button>
            {lastExport && !isDesktop() && (
              <Button variant="secondary" onClick={() => downloadExport(lastExport)}>
                <Download /> Download
              </Button>
            )}
          </div>
          {dataNote && (
            <p className="rounded-md border p-2 text-xs" role="status">
              {dataNote}
            </p>
          )}
          {dataError && (
            <p role="alert" className="text-xs text-destructive">
              {dataError}
            </p>
          )}
          {restorePath && (
            <div className="rounded-md border border-amber-500/50 bg-amber-500/10 p-3 text-sm">
              <p className="font-medium">Restore this archive?</p>
              <p className="mt-1 text-xs text-muted-foreground">{restorePath}</p>
              <div className="mt-2 flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={async () => {
                    try {
                      const res = await api.restoreBackup(restorePath, "replace");
                      setDataNote(res.message + " Please restart the app.");
                      setDataError(null);
                    } catch (err) {
                      setDataError((err as Error).message);
                    }
                    setRestorePath(null);
                  }}
                >
                  Replace current library
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={async () => {
                    try {
                      const res = await api.restoreBackup(restorePath, "separate");
                      setDataNote(res.message);
                      setDataError(null);
                    } catch (err) {
                      setDataError((err as Error).message);
                    }
                    setRestorePath(null);
                  }}
                >
                  Restore as separate library
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setRestorePath(null)}>
                  {t("common.cancel")}
                </Button>
              </div>
            </div>
          )}
          <p className="text-xs text-muted-foreground">
            Backups are crash-safe SQLite snapshots plus a hashed manifest, zipped.
            Restoring replaces the current library only after your explicit confirmation.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Physical locations</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-xs text-muted-foreground">
            Build the hierarchy room › bookcase › shelf › box by choosing a parent.
            A location with sub-locations cannot be deleted.
          </p>
          <ul className="space-y-1">
            {(locations ?? []).map((l) => (
              <li key={l.id} className="flex items-center justify-between rounded-md border px-3 py-2 text-sm">
                <span>
                  {locationLabel(l)}{" "}
                  <span className="text-muted-foreground">({LOCATION_LABELS[l.type] ?? l.type})</span>
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Delete ${locationLabel(l)}`}
                  onClick={() => deleteLocation.mutate(l.id)}
                >
                  <Trash2 />
                </Button>
              </li>
            ))}
            {locations && locations.length === 0 && (
              <li className="text-sm text-muted-foreground">No locations yet.</li>
            )}
          </ul>
          <div className="flex items-end gap-2">
            <div className="w-64 space-y-1">
              <Label htmlFor="loc-name">Name</Label>
              <Input
                id="loc-name"
                value={newLocationName}
                onChange={(e) => setNewLocationName(e.target.value)}
              />
            </div>
            <div className="w-40 space-y-1">
              <Label htmlFor="loc-type">Type</Label>
              <Select value={newLocationType} onValueChange={(v) => setNewLocationType(v as typeof newLocationType)}>
                <SelectTrigger id="loc-type">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {LOCATION_TYPES.map((t) => (
                    <SelectItem key={t} value={t}>
                      {LOCATION_LABELS[t]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="w-56 space-y-1">
              <Label htmlFor="loc-parent">Inside</Label>
              <Select value={newLocationParent || "none"} onValueChange={(v) => setNewLocationParent(v === "none" ? "" : v)}>
                <SelectTrigger id="loc-parent">
                  <SelectValue placeholder="Top level" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Top level</SelectItem>
                  {(locations ?? []).map((l) => (
                    <SelectItem key={l.id} value={l.id}>
                      {locationLabel(l)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              disabled={!newLocationName.trim() || addLocation.isPending}
              onClick={() => addLocation.mutate()}
            >
              <Plus /> Add
            </Button>
          </div>
          {locationError && (
            <p role="alert" className="text-sm text-destructive">
              {locationError}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

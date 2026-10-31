import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Cloud, Loader2, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { api } from "@/lib/api";
import { pickFile, saveExport } from "@/lib/files";
import { formatDate } from "@/lib/format";
import { useT } from "@/lib/i18n";
import type { CloudHistory, CloudLibrary, CloudResolution, CloudSyncResult } from "@/types/domain";

export default function CloudBackups({ profileId }: { profileId: string }) {
  const t = useT();
  const client = useQueryClient();
  const status = useQuery({ queryKey: ["cloud-status", profileId], queryFn: api.getCloudStatus, networkMode: "always", retry: false });
  const [libraries, setLibraries] = useState<CloudLibrary[] | null>(null);
  const [selectedLibrary, setSelectedLibrary] = useState("");
  const [name, setName] = useState("");
  const [result, setResult] = useState<CloudSyncResult | null>(null);
  const [selectedHead, setSelectedHead] = useState("");
  const [history, setHistory] = useState<CloudHistory | null>(null);
  const [historyRevision, setHistoryRevision] = useState("");
  const [confirmation, setConfirmation] = useState<CloudResolution | null>(null);
  const [restorePath, setRestorePath] = useState<string | null>(null);
  const [backupPath, setBackupPath] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState(false);
  const work = useMutation({
    mutationFn: async (operation: () => Promise<void>) => { setError(false); setNote(null); await operation(); },
    networkMode: "always", retry: false,
    onError: () => setError(true),
    onSettled: () => { void client.invalidateQueries({ queryKey: ["cloud-status"] }); },
  });
  const busy = work.isPending;
  const acceptResult = (next: CloudSyncResult) => {
    setResult(next);
    setHistory(null);
    setSelectedHead("");
    setConfirmation(null);
    if (next.status === "synced") {
      setNote(next.pendingLocalChanges ? "New local changes are waiting. Press Sync now again." : "Sync complete.");
      void client.invalidateQueries();
    }
  };
  const runSync = async (resolution: CloudResolution | null = null) => acceptResult(await api.syncCloudLibrary(resolution));
  const choose = (choice: CloudResolution["choice"]) => {
    if (!result?.conflict) return;
    setConfirmation({ choice, localDigest: result.conflict.localDigest, expectedHeads: result.conflict.heads.map(h => h.id) });
  };
  const cloudHeads = result?.conflict?.heads ?? [];
  const head = selectedHead || cloudHeads[0]?.id || "";
  const backupId = selectedLibrary || (libraries?.length === 1 ? libraries[0].id : "");
  const findBackups = async () => {
    setLibraries(await api.connectGoogleDrive()); setSelectedLibrary("");setResult(null);setHistory(null);
  };
  const showHistory = async () => {
    setHistory(await api.listCloudBackups());setHistoryRevision("");
  };
  const save = async (path: string) => {
    setNote(await saveExport(path) ? "Backup saved. Your file provider manages the cloud upload." : "Save cancelled. Your backup is ready; use Save file to try again.");
  };
  const date = (iso: string) => `${formatDate(iso)} ${new Date(iso).toLocaleTimeString([], { timeZone: "UTC", hour: "2-digit", minute: "2-digit", second: "2-digit" })} UTC`;

  return <Card aria-busy={busy}>
    <CardHeader><CardTitle className="flex items-center gap-2"><Cloud className="h-4 w-4" />{t("Cloud sync & backup")}</CardTitle></CardHeader>
    <CardContent className="space-y-6">
      <section className="space-y-3" aria-label={t("Google Drive")}>
        <h3 className="text-sm font-semibold">{t("Google Drive")}</h3>
        <p className="text-sm text-muted-foreground">{t("Sync only when you press Sync now. Your complete library, including contacts and finances, is stored in your Google Drive. Device preferences stay local.")}</p>
        <p className="text-sm text-muted-foreground">{t("Choose your Google account. Backups belong to your account and can be restored on your other devices.")}</p>
        {status.data && !status.data.nativeAvailable && <p className="text-sm text-muted-foreground">{t("Google Drive sync requires the installed app.")}</p>}
        {status.data?.nativeAvailable && !status.data.configured && <p className="text-sm text-muted-foreground">{t("Google Drive sign-in is not configured in this build.")}</p>}
        {status.data?.configured && <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={busy} onClick={() => work.mutate(findBackups)}>{t("Sign in to Google Drive")}</Button>
          {status.data.signedIn && !status.data.link && libraries === null && <Button variant="outline" disabled={busy} onClick={() => work.mutate(async () => setLibraries(await api.listCloudLibraries()))}>{t("Find my backup")}</Button>}
        </div>}
        {status.data?.signedIn && libraries !== null && !status.data.link && <div className="space-y-3 rounded-md border p-3">
          {libraries.length === 0 && <p className="text-sm text-muted-foreground">{t("No backup yet. Sync now will create one in your Google Drive.")}</p>}
          {libraries.length === 1 && <p className="break-words text-sm">{t("Backup found: {value}", {value:libraries[0].name})}</p>}
          {libraries.length > 1 && <div className="space-y-2">
            <Label htmlFor="cloud-library">{t("Cloud library")}</Label>
            <select id="cloud-library" className="flex h-12 w-full rounded-md border bg-background px-3 text-base" value={selectedLibrary} disabled={busy} onChange={e => setSelectedLibrary(e.target.value)}>
              <option value="">{t("Choose a cloud library")}</option>
              {libraries.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
            </select>
          </div>}
          <div className="flex flex-wrap gap-2">
            <Button disabled={busy || (libraries.length > 1 && !backupId)} onClick={() => work.mutate(async () => acceptResult(await api.startCloudSync(profileId,backupId || null)))}><RefreshCw />{t("Sync now")}</Button>
            {libraries.length > 0 && <Button variant="outline" disabled={busy || !backupId} onClick={() => work.mutate(async () => {
              await api.linkCloudLibrary(backupId);await showHistory();
            })}>{t("Restore from Google Drive")}</Button>}
          </div>
          <details className="space-y-2">
            <summary className="cursor-pointer text-sm text-muted-foreground">{t("Use a separate cloud backup")}</summary>
            <div className="space-y-2 pt-2">
            <Label htmlFor="new-cloud-library">{t("New cloud library")}</Label>
            <Input id="new-cloud-library" maxLength={80} value={name} disabled={busy} onChange={e => setName(e.target.value)} />
            <Button variant="outline" disabled={busy || !name.trim()} onClick={() => work.mutate(async () => {
              const created = await api.createCloudLibrary(name.trim());
              setLibraries([...(libraries ?? []), created]); setName("");
              await api.linkCloudLibrary(created.id);
            })}>{t("Create cloud library")}</Button>
            </div>
          </details>
        </div>}
        {status.data?.link && <div className="space-y-3 rounded-md border p-3">
          <p className="break-words text-sm font-medium">{status.data.link.libraryName}</p>
          {status.data.link.lastSynced && <p className="text-xs text-muted-foreground">{t("Last synced: {value}", { value: date(status.data.link.lastSynced) })}</p>}
          <div className="flex flex-wrap gap-2">
            <Button disabled={busy || !status.data.signedIn} onClick={() => work.mutate(() => runSync())}><RefreshCw />{t("Sync now")}</Button>
            <Button variant="outline" disabled={busy} onClick={() => work.mutate(async () => {
              await api.disconnectCloudLibrary(); setLibraries(null); setResult(null); setHistory(null); setSelectedLibrary("");
            })}>{t("Disconnect sync")}</Button>
          </div>
          <Button variant="outline" disabled={busy || !status.data.signedIn} onClick={() => work.mutate(showHistory)}>{t("Restore from Google Drive")}</Button>
        </div>}
        {history && <div className="space-y-2 rounded-md border p-3">
          <Label htmlFor="cloud-history">{t("Cloud backup")}</Label>
          <select id="cloud-history" className="h-12 w-full rounded-md border bg-background px-3 text-base" value={historyRevision} disabled={busy} onChange={e => setHistoryRevision(e.target.value)}>
            <option value="">{t("Choose a cloud backup")}</option>
            {history.backups.map((h, i) => <option key={h.id} value={h.id}>{i + 1} · {date(h.createdAt)}</option>)}
          </select>
          <Button variant="secondary" disabled={busy || !historyRevision} onClick={() => setConfirmation({
            choice:{kind:"use_cloud",revision:historyRevision},localDigest:history.localDigest,expectedHeads:history.expectedHeads,
          })}>{t("Use cloud version")}</Button>
        </div>}
        {result?.status === "conflict" && <div className="space-y-3 rounded-md border border-amber-500/50 p-3">
          <p role="status" className="text-sm font-medium">{t("Cloud changes need a choice.")}</p>
          <p className="text-sm text-muted-foreground">{t("Both versions are preserved. Choose which complete library to keep; records are not merged.")}</p>
          {cloudHeads.length > 1 && <div className="space-y-2">
            <Label htmlFor="cloud-revision">{t("Cloud backup")}</Label>
            <select id="cloud-revision" className="h-12 w-full rounded-md border bg-background px-3 text-base" value={head} disabled={busy} onChange={e => setSelectedHead(e.target.value)}>
              {cloudHeads.map((h, i) => <option key={h.id} value={h.id}>{i + 1} · {date(h.createdAt)}</option>)}
            </select>
          </div>}
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" disabled={busy} onClick={() => choose({ kind: "keep_device" })}>{t("Keep this device")}</Button>
            <Button variant="secondary" disabled={busy || !head} onClick={() => choose({ kind: "use_cloud", revision: head })}>{t("Use cloud version")}</Button>
          </div>
        </div>}
        {result?.localBackupPath && <Button variant="outline" disabled={busy} onClick={() => work.mutate(() => save(result.localBackupPath!))}>{t("Recovery backup")}</Button>}
      </section>
      <section className="space-y-3 border-t pt-4" aria-label={t("iCloud & file backups")}>
        <h3 className="text-sm font-semibold">{t("iCloud & file backups")}</h3>
        <p className="text-sm text-muted-foreground">{t("Choose iCloud Drive or another cloud provider in the system file picker. Upload availability depends on that provider.")}</p>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={busy} onClick={() => work.mutate(async () => {
            const backup = await api.createBackup(); setBackupPath(backup.path); await save(backup.path);
          })}>{t("Back up to cloud file")}</Button>
          <Button variant="outline" disabled={busy} onClick={() => work.mutate(async () => {
            const path = await pickFile(["zip"]); if (path) setRestorePath(path);
          })}>{t("Open cloud backup")}</Button>
          {backupPath && <Button variant="ghost" disabled={busy} onClick={() => work.mutate(() => save(backupPath))}>{t("Save file")}</Button>}
        </div>
      </section>
      {busy && <p role="status" className="flex items-center gap-2 text-sm"><Loader2 className="h-4 w-4 animate-spin" />{t("Working…")}</p>}
      {!busy && note && <p role="status" className="text-sm">{t(note)}</p>}
      {(error || status.isError) && <p role="alert" className="text-sm text-destructive">{t("Cloud operation failed. Check your connection and sign in again if needed.")}</p>}
    </CardContent>
    <Dialog open={confirmation !== null} onOpenChange={open => { if (!open && !busy) setConfirmation(null); }}>
      <DialogContent><DialogHeader>
        <DialogTitle>{t("Confirm sync choice")}</DialogTitle>
        <DialogDescription>{t(confirmation?.choice.kind === "use_cloud" ? "Replace this device with the selected cloud version? A recovery backup is saved first." : "Use this device as the cloud version? Existing cloud backups are kept.")}</DialogDescription>
      </DialogHeader><DialogFooter className="gap-2">
        <Button variant="outline" disabled={busy} onClick={() => setConfirmation(null)}>{t("Cancel")}</Button>
        <Button disabled={busy} onClick={() => { if (confirmation) work.mutate(() => runSync(confirmation)); }}>{t("Confirm sync choice")}</Button>
      </DialogFooter></DialogContent>
    </Dialog>
    <Dialog open={restorePath !== null} onOpenChange={open => { if (!open && !busy) setRestorePath(null); }}>
      <DialogContent><DialogHeader><DialogTitle>{t("Restore this archive?")}</DialogTitle>
        <DialogDescription>{t("Backups are crash-safe SQLite snapshots plus a hashed manifest, zipped. Restoring replaces the current library only after your explicit confirmation.")}</DialogDescription>
      </DialogHeader><div className="flex flex-wrap gap-2">
        {(["separate", "replace"] as const).map(mode => <Button key={mode} variant={mode === "replace" ? "destructive" : "secondary"} disabled={busy} onClick={() => work.mutate(async () => {
          const restored = await api.restoreBackup(restorePath!, mode); setRestorePath(null); setResult(null); setNote(restored.message); void client.invalidateQueries();
        })}>{t(mode === "separate" ? "Restore as separate library" : "Replace current library")}</Button>)}
        <Button variant="outline" disabled={busy} onClick={() => setRestorePath(null)}>{t("Cancel")}</Button>
      </div></DialogContent>
    </Dialog>
  </Card>;
}

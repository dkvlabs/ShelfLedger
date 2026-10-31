import { createElement } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider, onlineManager } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import CloudBackups from "@/features/settings/CloudBackups";
import { api } from "@/lib/api";
import { pickFile, saveExport } from "@/lib/files";
import type { CloudStatus, CloudSyncResult } from "@/types/domain";

vi.mock("@/lib/api", () => ({ api: {
  getCloudStatus: vi.fn(), connectGoogleDrive: vi.fn(), listCloudLibraries: vi.fn(), createCloudLibrary: vi.fn(),
  linkCloudLibrary: vi.fn(), syncCloudLibrary: vi.fn(), disconnectCloudLibrary: vi.fn(), createBackup: vi.fn(), restoreBackup: vi.fn(), listCloudBackups: vi.fn(),
} }));
vi.mock("@/lib/files", () => ({ pickFile: vi.fn(), saveExport: vi.fn() }));
vi.mock("@/lib/i18n", () => ({ useT: () => (s: string, vars?: {value: string}) => vars ? s.replace("{value}",vars.value) : s }));
const linked: CloudStatus = { nativeAvailable:true,configured:true,signedIn:true,link: {
  libraryId:"library-1",libraryName:"Shared books",revision:"rev-1",digest:"hash",lastSynced:null,
} };
const synced: CloudSyncResult = { status:"synced",lastSynced:"2026-10-31T12:00:00Z",localBackupPath:null,conflict:null,pendingLocalChanges:false };
const conflicted: CloudSyncResult = { ...synced,status:"conflict",conflict:{localDigest:"device-hash",heads:[{id:"rev-2",createdAt:"2026-10-31T12:00:00Z"}]} };
let client: QueryClient;
beforeEach(() => {
  client = new QueryClient({defaultOptions:{queries:{retry:false},mutations:{retry:false}}});
  vi.mocked(api.getCloudStatus).mockResolvedValue(linked);
  vi.mocked(api.syncCloudLibrary).mockResolvedValue(synced);
  vi.mocked(api.connectGoogleDrive).mockResolvedValue([{id:"library-1",name:"Shared books"}]);
  vi.mocked(api.createBackup).mockResolvedValue({path:"/app/backups/cloud.zip"});
  vi.mocked(saveExport).mockResolvedValue(true);
});
afterEach(() => { cleanup();client.clear();onlineManager.setOnline(true);vi.resetAllMocks(); });
function mount() { return render(createElement(QueryClientProvider,{client},createElement(CloudBackups,{profileId:"main"}))); }
async function syncButton() { return screen.findByRole("button",{name:"Sync now"}); }

it("loads local status offline without connecting, syncing or uploading on mount or refresh", async () => {
  onlineManager.setOnline(false);mount();await syncButton();await client.invalidateQueries();
  expect(api.connectGoogleDrive).not.toHaveBeenCalled();expect(api.syncCloudLibrary).not.toHaveBeenCalled();expect(api.createBackup).not.toHaveBeenCalled();
  expect(screen.queryByRole("button",{name:"Choose a cloud library"})).not.toBeInTheDocument();
});

it("explains an unconfigured build while keeping file backups available", async () => {
  vi.mocked(api.getCloudStatus).mockResolvedValue({...linked,configured:false,signedIn:false,link:null});
  mount();await screen.findByText("Google Drive sign-in is not configured in this build.");
  expect(screen.queryByRole("button",{name:"Sign in to Google Drive"})).not.toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Back up to cloud file"})).toBeEnabled();
});

it("connects and chooses an existing cloud library without sending library content automatically", async () => {
  vi.mocked(api.getCloudStatus).mockResolvedValue({...linked,link:null,signedIn:false});
  mount();fireEvent.click(await screen.findByRole("button",{name:"Sign in to Google Drive"}));
  await waitFor(() => expect(api.connectGoogleDrive).toHaveBeenCalledTimes(1));
  vi.mocked(api.getCloudStatus).mockResolvedValue({...linked,link:null});
  await client.invalidateQueries();
  const select = await screen.findByLabelText("Cloud library");
  fireEvent.change(select,{target:{value:"library-1"}});
  fireEvent.click(screen.getByRole("button",{name:"Use selected library"}));
  await waitFor(() => expect(api.linkCloudLibrary).toHaveBeenCalledWith("library-1"));
  expect(api.syncCloudLibrary).not.toHaveBeenCalled();
});

it("syncs only on request and allows a new attempt after an offline failure", async () => {
  onlineManager.setOnline(false);
  vi.mocked(api.syncCloudLibrary).mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(synced);
  mount();fireEvent.click(await syncButton());
  expect(await screen.findByRole("alert")).toHaveTextContent("Cloud operation failed.");
  await waitFor(() => expect(screen.getByRole("button",{name:"Sync now"})).toBeEnabled());
  fireEvent.click(await syncButton());
  await screen.findByText("Sync complete.");
  expect(api.syncCloudLibrary).toHaveBeenCalledTimes(2);
});

it("requires review of a conflict choice and sends the exact versions reviewed", async () => {
  vi.mocked(api.syncCloudLibrary).mockResolvedValueOnce(conflicted).mockResolvedValueOnce(synced);
  mount();fireEvent.click(await syncButton());
  await screen.findByText("Cloud changes need a choice.");
  fireEvent.click(screen.getByRole("button",{name:"Use cloud version"}));
  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByText(/A recovery backup is saved first/)).toBeInTheDocument();
  expect(api.syncCloudLibrary).toHaveBeenCalledTimes(1);
  fireEvent.click(within(dialog).getByRole("button",{name:"Confirm sync choice"}));
  await waitFor(() => expect(api.syncCloudLibrary).toHaveBeenLastCalledWith({choice:{kind:"use_cloud",revision:"rev-2"},localDigest:"device-hash",expectedHeads:["rev-2"]}));
  await screen.findByText("Sync complete.");
});

it("cancelled conflict review leaves all versions unchanged", async () => {
  vi.mocked(api.syncCloudLibrary).mockResolvedValue(conflicted);
  mount();fireEvent.click(await syncButton());
  fireEvent.click(await screen.findByRole("button",{name:"Keep this device"}));
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button",{name:"Cancel"}));
  expect(api.syncCloudLibrary).toHaveBeenCalledTimes(1);
  expect(screen.getByText("Cloud changes need a choice.")).toBeInTheDocument();
});

it("restores an older cloud backup only after review against the current versions", async () => {
  vi.mocked(api.listCloudBackups).mockResolvedValue({localDigest:"current-hash",expectedHeads:["rev-3"],backups:[{id:"rev-1",createdAt:"2026-10-30T12:00:00Z"}]});
  mount();fireEvent.click(await screen.findByRole("button",{name:"Cloud backup history"}));
  fireEvent.change(await screen.findByLabelText("Cloud backup",{exact:true}),{target:{value:"rev-1"}});
  fireEvent.click(screen.getByRole("button",{name:"Use cloud version"}));
  expect(api.syncCloudLibrary).not.toHaveBeenCalled();
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button",{name:"Confirm sync choice"}));
  await waitFor(() => expect(api.syncCloudLibrary).toHaveBeenCalledWith({choice:{kind:"use_cloud",revision:"rev-1"},localDigest:"current-hash",expectedHeads:["rev-3"]}));
});

it("retries a cancelled cloud file save using the existing generated backup", async () => {
  vi.mocked(saveExport).mockResolvedValueOnce(false).mockResolvedValueOnce(true);
  mount();fireEvent.click(screen.getByRole("button",{name:"Back up to cloud file"}));
  await screen.findByText("Save cancelled. Your backup is ready; use Save file to try again.");
  fireEvent.click(screen.getByRole("button",{name:"Save file"}));
  await screen.findByText("Backup saved. Your file provider manages the cloud upload.");
  expect(api.createBackup).toHaveBeenCalledTimes(1);expect(saveExport).toHaveBeenLastCalledWith("/app/backups/cloud.zip");
});

it("stages a cloud archive and restores only after an explicit restore choice", async () => {
  vi.mocked(pickFile).mockResolvedValue("/app/uploads/selected.zip");
  vi.mocked(api.restoreBackup).mockResolvedValue({requiresRestart:false,message:"Restored as separate library."});
  mount();fireEvent.click(screen.getByRole("button",{name:"Open cloud backup"}));
  const dialog = await screen.findByRole("dialog");
  expect(pickFile).toHaveBeenCalledWith(["zip"]);expect(api.restoreBackup).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button",{name:"Restore as separate library"}));
  await waitFor(() => expect(api.restoreBackup).toHaveBeenCalledWith("/app/uploads/selected.zip","separate"));
});

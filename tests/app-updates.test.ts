import { createElement } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, onlineManager } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AppUpdates from "@/features/settings/AppUpdates";
import { api } from "@/lib/api";
import { openUpdatePage } from "@/lib/updates";
import type { UpdateCheck } from "@/types/domain";

vi.mock("@/lib/api", () => ({ api: { getAppInfo: vi.fn(), checkForUpdates: vi.fn() } }));
vi.mock("@/lib/updates", () => ({ openUpdatePage: vi.fn() }));
vi.mock("@/lib/i18n", () => ({
  useT: () => (key: string, values?: { value: string }) => values ? key.replace("{value}", values.value) : key,
}));

const release: UpdateCheck = {
  currentVersion: "0.4.0", latestVersion: "0.5.0", updateAvailable: true, downloadAvailable: true,
};
let client: QueryClient;
beforeEach(() => {
  vi.mocked(api.getAppInfo).mockResolvedValue({ version: "0.4.0", platform: "macos" });
  vi.mocked(api.checkForUpdates).mockResolvedValue(release);
  vi.mocked(openUpdatePage).mockResolvedValue();
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
});
afterEach(() => { cleanup(); client.clear(); onlineManager.setOnline(true); vi.resetAllMocks(); });
function mount() {
  render(createElement(QueryClientProvider, { client }, createElement(AppUpdates)));
}
async function checkButton() {
  await screen.findByText("v0.4.0");
  return screen.getByRole("button", { name: "Check for updates" });
}

it("shows the installed binary version offline and never checks releases on mount or invalidation", async () => {
  onlineManager.setOnline(false);
  mount();
  await screen.findByText("v0.4.0");
  expect(api.checkForUpdates).not.toHaveBeenCalled();
  await client.invalidateQueries();
  expect(api.checkForUpdates).not.toHaveBeenCalled();
  expect(openUpdatePage).not.toHaveBeenCalled();
});

it("checks on click, prevents duplicate submissions, and opens downloads only on another click", async () => {
  let complete!: (result: UpdateCheck) => void;
  vi.mocked(api.checkForUpdates).mockImplementation(() => new Promise(resolve => { complete = resolve; }));
  mount();
  fireEvent.click(await checkButton());
  await waitFor(() => expect(complete).toBeDefined());
  const checking = screen.getByRole("button", { name: "Checking for updates…" });
  expect(checking).toBeDisabled();
  fireEvent.click(checking);
  expect(api.checkForUpdates).toHaveBeenCalledTimes(1);
  complete(release);
  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Version 0.5.0 is available."));
  expect(screen.getByText("Latest release: 0.5.0")).toBeInTheDocument();
  expect(openUpdatePage).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "View release" }));
  await waitFor(() => expect(openUpdatePage).toHaveBeenCalledWith("release"));
});

it("reports up to date without offering an automatic installation", async () => {
  vi.mocked(api.checkForUpdates).mockResolvedValue({ ...release, latestVersion: "0.4.0", updateAvailable: false });
  mount();
  fireEvent.click(await checkButton());
  expect(await screen.findByRole("status")).toHaveTextContent("You're up to date.");
  expect(openUpdatePage).not.toHaveBeenCalled();
});

it("runs an offline attempt, removes stale success on failure, and allows retry", async () => {
  onlineManager.setOnline(false);
  vi.mocked(api.checkForUpdates)
    .mockResolvedValueOnce({ ...release, updateAvailable: false })
    .mockRejectedValueOnce(new Error("service unavailable"))
    .mockResolvedValueOnce(release);
  mount();
  fireEvent.click(await checkButton());
  await screen.findByText("You're up to date.");
  fireEvent.click(await checkButton());
  expect(await screen.findByRole("alert")).toHaveTextContent("Check your internet connection and try again.");
  expect(screen.queryByText("You're up to date.")).not.toBeInTheDocument();
  expect(screen.getByText("v0.4.0")).toBeInTheDocument();
  fireEvent.click(await checkButton());
  expect(await screen.findByRole("status")).toHaveTextContent("Version 0.5.0 is available.");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("explains a missing Android APK and opens the Android builds destination", async () => {
  vi.mocked(api.getAppInfo).mockResolvedValue({ version: "0.4.0", platform: "android" });
  vi.mocked(api.checkForUpdates).mockResolvedValue({ ...release, downloadAvailable: false });
  mount();
  fireEvent.click(await checkButton());
  await screen.findByText("No Android APK is published for this release. See Android builds for downloads.");
  fireEvent.click(screen.getByRole("button", { name: "Android builds" }));
  await waitFor(() => expect(openUpdatePage).toHaveBeenCalledWith("android_builds"));
});

it("does not advertise downloads for an unsupported desktop architecture", async () => {
  vi.mocked(api.checkForUpdates).mockResolvedValue({ ...release, downloadAvailable: false });
  mount(); fireEvent.click(await checkButton());
  await screen.findByText("A download for this device is not published in this release.");
  expect(screen.queryByRole("button", { name: "Android builds" })).not.toBeInTheDocument();
});

it("does not invent a version when reading the installed version fails", async () => {
  vi.mocked(api.getAppInfo).mockRejectedValue(new Error("IPC failed"));
  mount();
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not read the installed version.");
  expect(screen.getByRole("button", { name: "Check for updates" })).toBeDisabled();
  expect(api.checkForUpdates).not.toHaveBeenCalled();
});

it("shows an opener failure without discarding the release result", async () => {
  vi.mocked(openUpdatePage).mockRejectedValue(new Error("no handler"));
  mount(); fireEvent.click(await checkButton());
  fireEvent.click(await screen.findByRole("button", { name: "View release" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the download page.");
  expect(screen.getByRole("status")).toHaveTextContent("Version 0.5.0 is available.");
});

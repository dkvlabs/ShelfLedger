import { createElement } from "react";
import {
  render,
  screen,
  fireEvent,
  cleanup,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "@/app/App";
import { api } from "@/lib/api";
import { pickFile } from "@/lib/files";
vi.mock("@/lib/i18n", () => ({
  useT: () => (s: string) => s,
  useLanguage: () => "en",
  translate: (_: string, s: string) => s,
  LANGUAGES: [{ value: "en", label: "English" }],
}));
vi.mock("@/lib/theme", () => ({ applyTheme: vi.fn() }));
vi.mock("@/features/catalog/LibraryView", () => ({
  default: () => createElement("p", null, "Library ready"),
}));
vi.mock("@/lib/files", () => ({ pickFile: vi.fn(), downloadExport: vi.fn() }));
vi.mock("@/lib/api", () => ({
  isDesktop: () => false,
  api: {
    getSettings: vi.fn(async () => ({})),
    listLibraries: vi.fn(async () => []),
    listLocations: vi.fn(async () => []),
    previewImport: vi.fn(),
    commitImport: vi.fn(),
    restoreBackup: vi.fn(),
  },
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("carries the Settings picker result through the real app shell and previews it exactly once", async () => {
  vi.mocked(pickFile).mockResolvedValue("/app-data/uploaded.csv");
  vi.mocked(api.previewImport).mockResolvedValue({
    kind: "csv",
    headers: ["Title"],
    rows: [
      {
        rowNumber: 1,
        title: "Chosen book",
        author: "",
        isbn13: null,
        shelf: null,
        rating: null,
        dateRead: null,
        warnings: [],
        duplicateOf: null,
      },
    ],
  });
  render(createElement(App));
  await screen.findByText("Library ready");
  fireEvent.click(screen.getByRole("button", { name: "nav.settings" }));
  await screen.findByRole("heading", { name: "Settings" });
  fireEvent.click(
    within(screen.getByRole("main")).getByRole("button", {
      name: "settings.import",
    }),
  );
  await screen.findByText("Chosen book");
  expect(pickFile).toHaveBeenCalledExactlyOnceWith(["csv", "json"]);
  expect(api.previewImport).toHaveBeenCalledExactlyOnceWith(
    "/app-data/uploaded.csv",
    {},
  );
  expect(api.commitImport).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: /Review import/ })).toBeEnabled(),
  );
});

it("shows picker errors and requires confirmation before a live restore that refreshes queries", async () => {
  render(createElement(App));
  await screen.findByText("Library ready");
  fireEvent.click(screen.getByRole("button", { name: "nav.settings" }));
  await screen.findByRole("heading", { name: "Settings" });
  const settings = within(screen.getByRole("main"));
  vi.mocked(pickFile).mockRejectedValueOnce(new Error("Upload failed"));
  fireEvent.click(settings.getByRole("button", { name: "settings.restore" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Upload failed");
  vi.mocked(pickFile).mockResolvedValueOnce("/app-data/archive.zip");
  vi.mocked(api.restoreBackup).mockResolvedValueOnce({
    message: "Library restored.",
    requiresRestart: false,
  });
  fireEvent.click(settings.getByRole("button", { name: "settings.restore" }));
  await screen.findByText("Restore this archive?");
  expect(api.restoreBackup).not.toHaveBeenCalled();
  const before = vi.mocked(api.getSettings).mock.calls.length;
  fireEvent.click(
    settings.getByRole("button", { name: "Replace current library" }),
  );
  expect(await screen.findByRole("status")).toHaveTextContent(
    "Library restored.",
  );
  expect(screen.queryByText(/Please restart/)).not.toBeInTheDocument();
  expect(api.restoreBackup).toHaveBeenCalledExactlyOnceWith(
    "/app-data/archive.zip",
    "replace",
  );
  await waitFor(() =>
    expect(vi.mocked(api.getSettings).mock.calls.length).toBeGreaterThan(
      before,
    ),
  );
});

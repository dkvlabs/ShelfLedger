import { afterEach, expect, it, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { pickFile, saveExport } from "@/lib/files";
import { call } from "@/lib/api";
vi.mock("@/lib/api", () => ({
  webToken: async () => "token",
  call: vi.fn(),
  CommandError: class extends Error {
    constructor(
      public kind: string,
      message: string,
    ) {
      super(message);
    }
  },
}));
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

it("uses native document commands for Android content URIs and preserves cancellation", async () => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  vi.mocked(call).mockResolvedValueOnce("/app-data/uploads/staged.zip").mockResolvedValueOnce(null);
  expect(await pickFile(["zip"])).toBe("/app-data/uploads/staged.zip");
  expect(call).toHaveBeenCalledWith("pick_import_file", { extensions: ["zip"] });
  expect(await pickFile(["csv", "json"])).toBeNull();
  expect(fetch).not.toHaveBeenCalled();
});

it("saves through the native picker and reports cancellation and failures", async () => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  vi.mocked(call).mockResolvedValueOnce(true).mockResolvedValueOnce(false).mockRejectedValueOnce(new Error("Storage full"));
  expect(await saveExport("/app-data/backups/library.zip")).toBe(true);
  expect(call).toHaveBeenCalledWith("save_export", { path: "/app-data/backups/library.zip" });
  expect(await saveExport("/app-data/backups/library.zip")).toBe(false);
  await expect(saveExport("/app-data/backups/library.zip")).rejects.toThrow("Storage full");
});
it("keeps the file input attached until selection and uploads only the selected file", async () => {
  vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => {});
  const fetch = vi.fn(
    async () =>
      new Response(JSON.stringify({ path: "/app-data/import.csv" }), {
        status: 200,
      }),
  );
  vi.stubGlobal("fetch", fetch);
  const selected = pickFile(["csv", "json"]);
  const input = document.querySelector("input[type=file]") as HTMLInputElement;
  expect(input).toBeInTheDocument();
  expect(input.accept).toBe(".csv,.json");
  const file = new File(["Title\nBook"], "books.csv", { type: "text/csv" });
  fireEvent.change(input, { target: { files: [file] } });
  expect(await selected).toBe("/app-data/import.csv");
  expect(input).not.toBeInTheDocument();
  expect(fetch).toHaveBeenCalledWith("/api/upload?name=books.csv", {
    method: "POST",
    headers: { "X-Shelfledger-Token": "token" },
    body: file,
  });
});
it("cancels without uploading and removes its temporary input", async () => {
  vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => {});
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  const selected = pickFile(["zip"]);
  const input = document.querySelector("input[type=file]") as HTMLInputElement;
  fireEvent(input, new Event("cancel"));
  expect(await selected).toBeNull();
  expect(input).not.toBeInTheDocument();
  expect(fetch).not.toHaveBeenCalled();
});
it("propagates upload errors while cleaning up the picker", async () => {
  vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => {});
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response(
          JSON.stringify({ kind: "validation", message: "Too large" }),
          { status: 413 },
        ),
    ),
  );
  const selected = pickFile(["csv"]);
  const input = document.querySelector("input[type=file]") as HTMLInputElement;
  fireEvent.change(input, {
    target: { files: [new File(["x"], "books.csv")] },
  });
  await expect(selected).rejects.toThrow("Too large");
  expect(input).not.toBeInTheDocument();
});

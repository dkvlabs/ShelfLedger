// File picking across transports. Native document streams are staged inside
// app data (including Android content URIs); the web version uploads the chosen
// file to the local server (`/api/upload`), which stores it under a generated
// name inside the app data directory and returns that path — so the backend
// commands read plain paths in both modes. Native saves copy generated files
// to an OS-selected destination. Downloads in web mode go through `/api/download`,
// which the server restricts to the exports/ and backups/ directories.

import { call, CommandError, webToken } from "@/lib/api";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function uploadFile(file: File): Promise<string> {
  let res: Response;
  try {
    res = await fetch(`/api/upload?name=${encodeURIComponent(file.name)}`, {
      method: "POST",
      headers: { "X-Shelfledger-Token": await webToken() },
      body: file,
    });
  } catch (err) {
    throw new CommandError(
      "network",
      `could not upload the file: ${(err as Error).message}`,
    );
  }
  const text = await res.text();
  const body: unknown = text ? JSON.parse(text) : null;
  if (!res.ok || !body || typeof body !== "object" || !("path" in body)) {
    const e = body as { kind?: string; message?: string } | null;
    throw new CommandError(
      e?.kind ?? "network",
      e?.message ?? `upload failed (HTTP ${res.status})`,
    );
  }
  return String((body as { path: unknown }).path);
}

/**
 * Open a file chooser and resolve to a path the backend commands can read,
 * or null when the user cancels. Errors (e.g. a failed upload) reject.
 */
export function pickFile(extensions: string[]): Promise<string | null> {
  if (inTauri()) {
    return call<string | null>("pick_import_file", { extensions });
  }
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = extensions.map((e) => `.${e}`).join(",");
    input.hidden = true;
    document.body.append(input);
    const cleanup = () => input.remove();
    input.addEventListener("change", () => {
      const file = input.files?.[0] ?? null;
      cleanup();
      if (!file) {
        resolve(null);
        return;
      }
      uploadFile(file).then(resolve, reject);
    });
    input.addEventListener("cancel", () => {
      cleanup();
      resolve(null);
    });
    input.click();
  });
}

/**
 * In the web version, hand the browser a file the export commands wrote to
 * the local data directory. No-op meaning on desktop: the file is already on
 * disk where the app said it was written.
 */
export function downloadExport(path: string): void {
  window.location.href = `/api/download?path=${encodeURIComponent(path)}`;
}

/** Save a generated file using the native document picker, or download it in
 * the browser. Android content URIs are resolved in Rust, not treated as paths. */
export async function saveExport(path: string): Promise<boolean> {
  if (inTauri()) return call<boolean>("save_export", { path });
  downloadExport(path);
  return true;
}

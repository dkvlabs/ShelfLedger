import { call, isDesktop } from "./api";

export type UpdatePage = "release" | "android_builds";

const pages: Record<UpdatePage, string> = {
  release: "https://github.com/dkvlabs/ShelfLedger/releases/latest",
  android_builds: "https://github.com/dkvlabs/ShelfLedger/actions/workflows/android.yml",
};

/** Invoked only by a click; native apps independently validate the destination. */
export async function openUpdatePage(page: UpdatePage): Promise<void> {
  if (isDesktop()) return call<void>("open_update_page", { page });
  const opened = window.open(pages[page], "_blank", "noopener,noreferrer");
  // With noopener, a browser can return null even when the tab was opened.
  opened?.focus();
}

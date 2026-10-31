import { afterEach, expect, it, vi } from "vitest";
import { call, isDesktop } from "@/lib/api";
import { openUpdatePage } from "@/lib/updates";

vi.mock("@/lib/api", () => ({ call: vi.fn(), isDesktop: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.resetAllMocks(); });

it("uses the named native command for desktop and Android downloads", async () => {
  vi.mocked(isDesktop).mockReturnValue(true);
  await openUpdatePage("android_builds");
  expect(call).toHaveBeenCalledWith("open_update_page", { page: "android_builds" });
});

it("opens only fixed project pages in the web version", async () => {
  vi.mocked(isDesktop).mockReturnValue(false);
  const open = vi.spyOn(window, "open").mockReturnValue(null);
  await openUpdatePage("release");
  expect(open).toHaveBeenCalledWith("https://github.com/dkvlabs/ShelfLedger/releases/latest", "_blank", "noopener,noreferrer");
  expect(call).not.toHaveBeenCalled();
});

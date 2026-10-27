import { afterEach, describe, expect, it, vi } from "vitest";

import { api, CommandError, isDesktop } from "@/lib/api";

function mockFetch(status: number, body: unknown) {
  const fn = vi.fn(
    async (_url: string, _init?: RequestInit): Promise<Response> =>
      new Response(
        _url.endsWith("/session")
          ? JSON.stringify({ token: "test-token" })
          : body === undefined
            ? null
            : JSON.stringify(body),
        {
          status: _url.endsWith("/session") ? 200 : status,
          headers: { "Content-Type": "application/json" },
        },
      ),
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

describe("web transport (browser mode)", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("is not the desktop transport in tests", () => {
    expect(isDesktop()).toBe(false);
  });

  it("posts each command with the same argument object as invoke()", async () => {
    const fn = mockFetch(200, { workCount: 2, copyCount: 3 });
    const stats = await api.getLibraryStats();
    expect(stats).toEqual({ workCount: 2, copyCount: 3 });
    expect(fn.mock.calls.some(([url]) => url === "/api/session")).toBe(true);
    const [url, init] = fn.mock.calls.find(
      ([url]) => url !== "/api/session",
    ) as unknown as [string, RequestInit];
    expect(url).toBe("/api/get_library_stats");
    expect(init.method).toBe("POST");
    expect(init.headers).toMatchObject({ "X-Shelfledger-Token": "test-token" });
    expect(JSON.parse(String(init.body))).toEqual({});
  });

  it("sends camelCase argument keys", async () => {
    const fn = mockFetch(200, []);
    await api.listLoans("2026-10-27");
    const [, init] = fn.mock.calls.find(
      ([url]) => url !== "/api/session",
    ) as unknown as [string, RequestInit];
    expect(JSON.parse(String(init.body))).toEqual({ localToday: "2026-10-27" });
  });

  it("maps {kind, message} error bodies onto CommandError", async () => {
    mockFetch(409, { kind: "conflict", message: "copy is already lent out" });
    const err = (await api.getSettings().catch((e) => e)) as CommandError;
    expect(err).toBeInstanceOf(CommandError);
    expect(err.kind).toBe("conflict");
    expect(err.message).toBe("copy is already lent out");
  });

  it("treats 204 responses as empty results", async () => {
    mockFetch(204, undefined);
    await expect(api.archiveCopy("copy-1")).resolves.toBeNull();
  });

  it("wraps unreachable servers as a network error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      }),
    );
    const err = (await api.getSettings().catch((e) => e)) as CommandError;
    expect(err).toBeInstanceOf(CommandError);
    expect(err.kind).toBe("network");
  });
});

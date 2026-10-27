import { createElement } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, expect, it, vi } from "vitest";
import AddBookDialog from "@/features/catalog/AddBookDialog";
import { api } from "@/lib/api";

vi.mock("@/lib/api", async (original) => ({
  ...await original<typeof import("@/lib/api")>(),
  api: {
    getSettings: vi.fn(),
    listLocations: vi.fn(async () => []),
    findDuplicates: vi.fn(async () => []),
    addBook: vi.fn(async () => ({ workId: "test-work", warning: null })),
  },
}));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("uses asynchronously loaded currency preferences on first open and saves its exact scale", async () => {
  let loadSettings!: (value: Record<string, unknown>) => void;
  vi.mocked(api.getSettings).mockImplementation(() => new Promise((resolve) => { loadSettings = resolve; }));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(createElement(QueryClientProvider, { client }, createElement(AddBookDialog, {
    open: true, onOpenChange: vi.fn(), onCreated: vi.fn(),
  })));
  await waitFor(() => expect(loadSettings).toBeDefined());
  loadSettings({ preferences: { currency: "CLF", language: "en" } });
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Currency" })).toHaveTextContent("CLF"));
  fireEvent.change(screen.getByLabelText("Title *"), { target: { value: "Currency regression fixture" } });
  fireEvent.change(screen.getByLabelText("Purchase price"), { target: { value: "12.3456" } });
  fireEvent.click(screen.getByRole("button", { name: "Save book" }));
  await waitFor(() => expect(api.addBook).toHaveBeenCalledWith(expect.objectContaining({
    copy: expect.objectContaining({ acquisition: expect.objectContaining({ currency: "CLF", priceMinor: 123456 }) }),
  })));
});

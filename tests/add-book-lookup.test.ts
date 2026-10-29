import { createElement } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AddBookDialog from "@/features/catalog/AddBookDialog";
import { api, CommandError } from "@/lib/api";
import type { LookupIsbnResult } from "@/types/domain";

vi.mock("@/lib/api", async (original) => ({
  ...await original<typeof import("@/lib/api")>(),
  api: {
    getSettings: vi.fn(async () => ({ preferences: { language: "en" } })),
    listLocations: vi.fn(async () => []),
    findDuplicates: vi.fn(async () => []),
    addBook: vi.fn(async () => ({ workId: "test-work", warning: null })),
    lookupIsbn: vi.fn(), lookupLibrisBook: vi.fn(),
  },
}));
const ISBN = "9786067882629";
const URL = "https://www.libris.ro/carte/mitologie-nordica-neil-gaiman/1258850";
const book: LookupIsbnResult = {
  source: "libris", isbn13: ISBN, isbn10: "6067882620", title: "Mitologie nordica",
  subtitle: null, publisher: "GRUPUL EDITORIAL ART", publishedAt: "2017", language: "ro",
  pageCount: 250, authorNames: ["Neil Gaiman"], cached: false,
};
beforeEach(() => { vi.mocked(api.lookupIsbn).mockResolvedValue(null); });
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function openDialog() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(createElement(QueryClientProvider, { client }, createElement(AddBookDialog, {
    open: true, onOpenChange: vi.fn(), onCreated: vi.fn(),
  })));
}
function enterIsbn(value = ISBN) {
  fireEvent.change(screen.getByLabelText("ISBN"), { target: { value } });
}
async function saveManually() {
  fireEvent.change(screen.getByLabelText("Title *"), { target: { value: "Mitologie nordica" } });
  fireEvent.click(screen.getByRole("button", { name: "Save book" }));
  await waitFor(() => expect(api.addBook).toHaveBeenCalledWith(expect.objectContaining({ isbn13: ISBN, isbn10: "6067882620" })));
  expect(api.findDuplicates).toHaveBeenCalledWith({ isbn: ISBN, title: null, authorName: null });
}
it("keeps a typed Romanian ISBN after no match and when skipping lookup", async () => {
  openDialog();
  enterIsbn("978-606-788-262-9");
  fireEvent.click(screen.getByRole("button", { name: "Look up" }));
  await screen.findByText(/No match in Open Library or Google Books/);
  await saveManually();
});
it("keeps a typed ISBN when saving directly without any provider request", async () => {
  openDialog(); enterIsbn(); await saveManually();
  expect(api.lookupIsbn).not.toHaveBeenCalled();
});
it("distinguishes a Libris page without book metadata from disabled lookup", async () => {
  vi.mocked(api.lookupLibrisBook).mockResolvedValue(null);
  openDialog(); enterIsbn();
  fireEvent.change(screen.getByLabelText("Libris book link (optional)"), { target: { value: URL } });
  fireEvent.click(screen.getByRole("button", { name: "Import from Libris" }));
  await screen.findByText("No book metadata found on this Libris page. Enter the details manually.");
  expect(screen.queryByText(/Online lookup is disabled/)).not.toBeInTheDocument();
  await saveManually();
});
it("does not show an invalid-checksum warning while a valid lookup is pending", async () => {
  let complete!: (book: LookupIsbnResult | null) => void;
  vi.mocked(api.lookupIsbn).mockImplementation(() => new Promise((resolve) => { complete = resolve; }));
  openDialog(); enterIsbn();
  fireEvent.click(screen.getByRole("button", { name: "Look up" }));
  await waitFor(() => expect(complete).toBeDefined());
  expect(screen.queryByText("That ISBN checksum is not valid.")).not.toBeInTheDocument();
  await act(async () => complete(null));
  await screen.findByText(/No match in Open Library or Google Books/);
});
it("does not apply a stale result after the ISBN changes", async () => {
  let complete!: (book: LookupIsbnResult) => void;
  vi.mocked(api.lookupIsbn).mockImplementation(() => new Promise((resolve) => { complete = resolve; }));
  openDialog(); enterIsbn(); fireEvent.click(screen.getByRole("button", { name: "Look up" }));
  await waitFor(() => expect(complete).toBeDefined());
  enterIsbn("9780306406157");
  await act(async () => complete(book));
  expect(screen.getByLabelText("Title *")).toHaveValue("");
  expect(screen.getByLabelText("ISBN")).toHaveValue("9780306406157");
});
it("imports the supplied Libris page, attributes its source, and saves the edition", async () => {
  vi.mocked(api.lookupLibrisBook).mockResolvedValue(book);
  openDialog(); enterIsbn();
  fireEvent.change(screen.getByLabelText("Libris book link (optional)"), { target: { value: URL } });
  fireEvent.click(screen.getByRole("button", { name: "Import from Libris" }));
  await screen.findByText("Metadata fetched from Libris. Review and correct it before saving.");
  expect(api.lookupLibrisBook).toHaveBeenCalledWith(URL, ISBN);
  expect(screen.getByLabelText("Title *")).toHaveValue("Mitologie nordica");
  fireEvent.click(screen.getByRole("button", { name: "Save book" }));
  await waitFor(() => expect(api.addBook).toHaveBeenCalledWith(expect.objectContaining({
    isbn13: ISBN, title: "Mitologie nordica", authorNames: ["Neil Gaiman"],
    publisher: "GRUPUL EDITORIAL ART", publishedAt: "2017", language: "ro", pageCount: 250,
  })));
});
it("shows the actual cached provider rather than always saying Open Library", async () => {
  vi.mocked(api.lookupIsbn).mockResolvedValue({ ...book, source: "googlebooks", cached: true });
  openDialog(); enterIsbn(); fireEvent.click(screen.getByRole("button", { name: "Look up" }));
  await screen.findByText("Found in local cache (Google Books). Review the metadata before saving.");
});
it("keeps manual entry available after a provider quota or network error", async () => {
  vi.mocked(api.lookupIsbn).mockRejectedValue(new CommandError("network", "HTTP 429"));
  openDialog(); enterIsbn(); fireEvent.click(screen.getByRole("button", { name: "Look up" }));
  await screen.findByText(/Lookup failed \(offline or provider unavailable\)/);
  await saveManually();
});
it.each(["isbn", "libris"])("explains a disabled %s lookup without claiming a provider miss", async (provider) => {
  const error = new CommandError("metadata_disabled", "backend diagnostic");
  vi.mocked(api.lookupIsbn).mockRejectedValue(error);
  vi.mocked(api.lookupLibrisBook).mockRejectedValue(error);
  openDialog(); enterIsbn();
  if (provider === "libris") {
    fireEvent.change(screen.getByLabelText("Libris book link (optional)"), { target: { value: URL } });
  }
  fireEvent.click(screen.getByRole("button", { name: provider === "isbn" ? "Look up" : "Import from Libris" }));
  await screen.findByText("Online lookup is disabled on this device and no cached metadata was found. Enter details manually or import a library backup with cached metadata.");
  expect(screen.queryByText(/No match in Open Library or Google Books/)).not.toBeInTheDocument();
  await saveManually();
});

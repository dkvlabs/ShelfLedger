import { createElement } from "react";
import { afterEach, expect, it, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import ImportView from "@/features/imports/ImportView";
import SessionEditor from "@/features/reading/SessionEditor";
import { api } from "@/lib/api";
import type { ReadingSessionDto } from "@/types/domain";
vi.mock("@/lib/i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("@/lib/api", () => ({
  api: {
    previewImport: vi.fn(),
    commitImport: vi.fn(),
    updateReadingSession: vi.fn(),
  },
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
function wrapper({ children }: { children: React.ReactNode }) {
  return createElement(
    QueryClientProvider,
    {
      client: new QueryClient({
        defaultOptions: { queries: { retry: false } },
      }),
    },
    children,
  );
}
it("previews the Settings-selected file once and writes only after review and confirmation", async () => {
  vi.mocked(api.previewImport).mockResolvedValue({
    kind: "csv",
    headers: ["Title"],
    rows: [
      {
        rowNumber: 1,
        title: "Book",
        author: "Writer",
        warnings: [],
        isbn13: null,
        shelf: null,
        rating: null,
        dateRead: null,
        duplicateOf: null,
      },
    ],
  });
  vi.mocked(api.commitImport).mockResolvedValue({
    rows: [{ rowNumber: 1, status: "imported", message: null }],
    importedWorks: 1,
    importedEditions: 1,
    importedCopies: 0,
    skipped: 0,
    errors: [],
  });
  render(createElement(ImportView, { initialPath: "/app-data/chosen.csv" }), {
    wrapper,
  });
  await screen.findByText("Book");
  expect(api.previewImport).toHaveBeenCalledExactlyOnceWith(
    "/app-data/chosen.csv",
    {},
  );
  expect(api.commitImport).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: /Review import/ }));
  expect(api.commitImport).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirm import" }));
  await screen.findByText("Import complete");
  expect(api.commitImport).toHaveBeenCalledWith(
    "/app-data/chosen.csv",
    {},
    { rowNumbers: [1], duplicatePolicy: "skip", rowPolicies: {} },
  );
});
it("moves a wanted book into reading, then preserves explicit cleared dates and review", async () => {
  vi.mocked(api.updateReadingSession).mockResolvedValue(
    {} as ReadingSessionDto,
  );
  const session = {
    id: "session",
    status: "want_to_read",
    rating: null,
    review: null,
    startedAt: null,
    finishedAt: null,
  } as ReadingSessionDto;
  render(createElement(SessionEditor, { session }), { wrapper });
  fireEvent.click(screen.getByRole("button", { name: "Start reading" }));
  expect(screen.getByLabelText("Reading status")).toHaveValue("reading");
  fireEvent.change(screen.getByLabelText("Review"), {
    target: { value: "Notes before I start" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save session" }));
  await waitFor(() =>
    expect(api.updateReadingSession).toHaveBeenCalledWith("session", {
      status: "reading",
      rating: null,
      review: "Notes before I start",
      startedAt: null,
      finishedAt: null,
    }),
  );
});

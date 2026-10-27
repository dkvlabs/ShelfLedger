import { createElement } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MarketComparisonDialog } from "@/features/finance/MarketComparisonDialog";
import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import type { EditionDto, MarketReport } from "@/types/domain";

vi.mock("@/lib/i18n", () => ({
  useT: () => (key: string) => key,
  useFormatMoney: () => formatMoney,
  useLanguage: () => "en",
}));
vi.mock("@/lib/api", () => ({
  api: {
    listMarketComparisons: vi.fn(),
    saveMarketComparison: vi.fn(),
    setMarketComparisonIncluded: vi.fn(),
    saveMarketEstimate: vi.fn(),
    listValuations: vi.fn(),
  },
}));
let report: MarketReport;
const edition = {
  id: "ed",
  publisher: "QA publisher",
  isbn13: "9780306406157",
  copies: [
    {
      id: "copy",
      inventoryCode: "QA-1",
      ownershipStatus: "owned",
      conditionGrade: "good",
      archivedAt: null,
    },
  ],
} as EditionDto;
beforeEach(() => {
  report = { comparables: [], summaries: [] };
  vi.mocked(api.listMarketComparisons).mockImplementation(async () =>
    structuredClone(report),
  );
  vi.mocked(api.listValuations).mockResolvedValue([]);
  vi.mocked(api.saveMarketComparison).mockResolvedValue();
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
function mount(selected = edition) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    createElement(
      QueryClientProvider,
      { client },
      createElement(MarketComparisonDialog, {
        edition: selected,
        title: "QA fixture, not market data",
        authors: "QA author",
        onClose: vi.fn(),
      }),
    ),
  );
}
it("requires verification and valid money, then saves the edition-scoped observation", async () => {
  mount();
  await screen.findByText("No comparisons yet.");
  const save = screen.getByRole("button", { name: "Save comparison" });
  fireEvent.change(screen.getByLabelText("OLX listing URL"), {
    target: { value: "https://www.olx.ro/d/oferta/qa-IDtest.html" },
  });
  fireEvent.change(screen.getByLabelText("Advertised price"), {
    target: { value: "12.50" },
  });
  expect(save).toBeDisabled();
  fireEvent.click(
    screen.getByLabelText("I checked the edition, language and condition."),
  );
  expect(save).toBeEnabled();
  fireEvent.click(save);
  await waitFor(() =>
    expect(api.saveMarketComparison).toHaveBeenCalledWith(
      expect.objectContaining({
        editionId: "ed",
        askingMinor: 1250,
        currency: "RON",
        editionChecked: true,
      }),
    ),
  );
  await waitFor(() =>
    expect(screen.getByLabelText("Advertised price")).toHaveValue(""),
  );
  expect(api.saveMarketEstimate).not.toHaveBeenCalled();
});
it("refetches excluded listings, simulates without writing, and saves the median explicitly", async () => {
  report = {
    comparables: [
      {
        id: "comp",
        editionId: "ed",
        listingUrl: "https://www.olx.ro/d/oferta/qa-IDtest.html",
        askingMinor: 4000,
        currency: "RON",
        observedAt: "2026-10-27",
        conditionNote: "QA fixture",
        included: true,
        createdAt: "2026-10-27",
        updatedAt: "2026-10-27",
      },
    ],
    summaries: [
      {
        currency: "RON",
        count: 1,
        minMinor: 4000,
        maxMinor: 4000,
        medianMinor: 4000,
        medianRounded: false,
      },
    ],
  };
  vi.mocked(api.setMarketComparisonIncluded).mockImplementation(
    async (_id, included) => {
      report.comparables[0].included = included;
      report.summaries = included
        ? [
            {
              currency: "RON",
              count: 1,
              minMinor: 4000,
              maxMinor: 4000,
              medianMinor: 4000,
              medianRounded: false,
            },
          ]
        : [];
    },
  );
  vi.mocked(api.saveMarketEstimate).mockResolvedValue({} as never);
  mount();
  const include = await screen.findByLabelText("Include in estimate");
  fireEvent.click(screen.getByRole("button", { name: "Use median" }));
  expect(screen.getByLabelText("Asking price")).toHaveValue("40.00");
  fireEvent.change(screen.getByLabelText("Selling fee"), {
    target: { value: "2.50" },
  });
  fireEvent.change(screen.getByLabelText("Shipping cost"), {
    target: { value: "5" },
  });
  expect(screen.getByText(/Estimated net proceeds/)).toHaveTextContent(
    formatMoney(3250, "RON").replace(/\s/g, " "),
  );
  expect(api.saveMarketEstimate).not.toHaveBeenCalled();
  fireEvent.click(include);
  await screen.findByText(
    "Include a checked listing to calculate an estimate.",
  );
  expect(
    screen.getByRole("button", { name: "Save median estimate" }),
  ).toBeDisabled();
  await waitFor(() => expect(include).toBeEnabled());
  fireEvent.click(include);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Save median estimate" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Save median estimate" }));
  await screen.findByText("Estimate saved with its evidence.");
  expect(api.saveMarketEstimate).toHaveBeenCalledExactlyOnceWith({
    editionId: "ed",
    copyId: "copy",
    currency: "RON",
    confidence: "low",
  });
});
it("supports Goodreads editions without inventing owned copies", async () => {
  mount({ ...edition, copies: [] });
  await screen.findByText("No comparisons yet.");
  expect(
    screen.getByText("Add an owned copy of this edition to save an estimate."),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Save median estimate" }),
  ).not.toBeInTheDocument();
  expect(api.listValuations).not.toHaveBeenCalled();
});
it("never reinterprets an existing simulation when another currency is added", async () => {
  mount();
  await screen.findByText("No comparisons yet.");
  fireEvent.change(screen.getByLabelText("Asking price"), {
    target: { value: "40" },
  });
  expect(screen.getByText(/Estimated net proceeds/)).toHaveTextContent(
    /RON.*40.00/,
  );
  vi.mocked(api.saveMarketComparison).mockImplementation(async () => {
    report.summaries = [
      {
        currency: "EUR",
        count: 1,
        minMinor: 1000,
        maxMinor: 1000,
        medianMinor: 1000,
        medianRounded: false,
      },
    ];
  });
  fireEvent.change(screen.getByLabelText("OLX listing URL"), {
    target: { value: "https://www.olx.ro/d/oferta/qa-IDtest.html" },
  });
  fireEvent.change(screen.getByLabelText("Advertised price"), {
    target: { value: "10" },
  });
  fireEvent.click(
    screen.getByLabelText("I checked the edition, language and condition."),
  );
  fireEvent.click(screen.getByRole("button", { name: "Save comparison" }));
  await screen.findByText(/EUR · Included listings/);
  expect(screen.getByText(/Estimated net proceeds/)).toHaveTextContent(
    /RON.*40.00/,
  );
  expect(screen.getByRole("button", { name: "Use median" })).toBeDisabled();
});

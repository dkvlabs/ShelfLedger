// Typed wrappers around the backend commands. The webview never sends SQL —
// only validated DTOs to named commands (AGENTS.md). One contract, two
// transports: Tauri IPC inside the desktop app, loopback HTTP
// (`POST /api/<command>`) in the web version — the argument object and the
// `{kind, message}` error shape are identical.

import { invoke } from "@tauri-apps/api/core";
import type {
  AddBookInput,
  AddBookResult,
  AddProgressInput,
  AddValuationInput,
  BackupResult,
  ContactDto,
  CopyDto,
  CreateContactInput,
  CreateCopyInput,
  CreateEditionInput,
  CreateListingInput,
  CreateLocationInput,
  CreateReadingSessionInput,
  CreateWorkInput,
  DashboardDto,
  DisposalDto,
  DuplicateQuery,
  EditionDto,
  ExportResult,
  FinanceReport,
  ImportCommitInput,
  ImportPreview,
  ImportResult,
  LibraryStats,
  LoanDto,
  LendCopyInput,
  LocationDto,
  LookupIsbnResult,
  ReadingSessionDto,
  RecordDisposalInput,
  RestoreResult,
  ReturnCopyInput,
  SaleListingDto,
  SellQueueItem,
  UpdateContactInput,
  UpdateCopyInput,
  UpdateReadingSessionInput,
  UpdateWorkInput,
  ValuationDto,
  WorkDetail,
  WorkSummary,
} from "@/types/domain";

export class CommandError extends Error {
  readonly kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.name = "CommandError";
    this.kind = kind;
  }
}

/** True inside the Tauri webview; false in a plain browser (web version). */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Base for the web transport; empty means same-origin (the web server). */
const API_BASE = ((import.meta as { env?: Record<string, string> }).env?.VITE_API_BASE ?? "");

async function webCall<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}/api/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(args ?? {}),
    });
  } catch (err) {
    throw new CommandError(
      "network",
      `could not reach the local ShelfLedger server: ${(err as Error).message}`,
    );
  }
  if (res.status === 204) return null as T;
  const text = await res.text();
  const body: unknown = text ? JSON.parse(text) : null;
  if (!res.ok) {
    if (body && typeof body === "object" && "kind" in body && "message" in body) {
      const e = body as { kind: string; message: string };
      throw new CommandError(String(e.kind), String(e.message));
    }
    throw new CommandError("network", `HTTP ${res.status}`);
  }
  return body as T;
}

/** One command call over whichever transport this build runs on. */
export async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  try {
    if (isDesktop()) return (await invoke(cmd, args)) as T;
    return await webCall<T>(cmd, args);
  } catch (err) {
    if (err && typeof err === "object" && "kind" in err && "message" in err) {
      const e = err as { kind: string; message: string };
      throw new CommandError(String(e.kind), String(e.message));
    }
    throw err;
  }
}

export const api = {
  // catalog
  searchWorks: (query?: string | null): Promise<WorkSummary[]> =>
    call("search_works", { query: query ?? null }),
  getWork: (id: string): Promise<WorkDetail> => call("get_work", { id }),
  addBook: (input: AddBookInput): Promise<AddBookResult> => call("add_book", { input }),
  createWork: (input: CreateWorkInput): Promise<WorkDetail> => call("create_work", { input }),
  updateWork: (id: string, input: UpdateWorkInput): Promise<WorkDetail> =>
    call("update_work", { id, input }),
  createEdition: (input: CreateEditionInput): Promise<EditionDto> =>
    call("create_edition", { input }),
  createCopy: (input: CreateCopyInput): Promise<CopyDto> => call("create_copy", { input }),
  updateCopy: (id: string, input: UpdateCopyInput): Promise<CopyDto> =>
    call("update_copy", { id, input }),
  archiveCopy: (id: string): Promise<void> => call("archive_copy", { id }),
  findDuplicates: (query: DuplicateQuery): Promise<WorkSummary[]> =>
    call("find_duplicates", { query }),
  getLibraryStats: (): Promise<LibraryStats> => call("get_library_stats"),

  // locations
  listLocations: (): Promise<LocationDto[]> => call("list_locations"),
  createLocation: (input: CreateLocationInput): Promise<LocationDto> =>
    call("create_location", { input }),
  deleteLocation: (id: string): Promise<boolean> => call("delete_location", { id }),

  // settings
  getSettings: (): Promise<Record<string, unknown>> => call("get_settings"),
  setSetting: (key: string, value: unknown): Promise<void> => call("set_setting", { key, value }),

  // metadata
  lookupIsbn: (isbn: string): Promise<LookupIsbnResult | null> =>
    call("lookup_isbn", { isbn }),

  // reading (Phase 2)
  listReadingSessions: (status?: string | null): Promise<ReadingSessionDto[]> =>
    call("list_reading_sessions", { status: status ?? null }),
  listReadingSessionsForWork: (workId: string): Promise<ReadingSessionDto[]> =>
    call("list_reading_sessions_for_work", { workId }),
  createReadingSession: (input: CreateReadingSessionInput): Promise<ReadingSessionDto> =>
    call("create_reading_session", { input }),
  updateReadingSession: (
    id: string,
    input: UpdateReadingSessionInput,
  ): Promise<ReadingSessionDto> => call("update_reading_session", { id, input }),
  addReadingProgress: (sessionId: string, input: AddProgressInput): Promise<ReadingSessionDto> =>
    call("add_reading_progress", { sessionId, input }),

  // loans (Phase 2)
  listContacts: (): Promise<ContactDto[]> => call("list_contacts"),
  createContact: (input: CreateContactInput): Promise<ContactDto> =>
    call("create_contact", { input }),
  updateContact: (id: string, input: UpdateContactInput): Promise<ContactDto> =>
    call("update_contact", { id, input }),
  listLoans: (localToday: string): Promise<LoanDto[]> => call("list_loans", { localToday }),
  listActiveLoans: (localToday: string): Promise<LoanDto[]> =>
    call("list_active_loans", { localToday }),
  listOverdueLoans: (localToday: string): Promise<LoanDto[]> =>
    call("list_overdue_loans", { localToday }),
  lendCopy: (input: LendCopyInput): Promise<LoanDto> => call("lend_copy", { input }),
  returnLoan: (loanId: string, input: ReturnCopyInput): Promise<LoanDto> =>
    call("return_loan", { loanId, input }),

  // finance (Phase 3)
  addValuation: (input: AddValuationInput): Promise<ValuationDto> =>
    call("add_valuation", { input }),
  listValuations: (copyId: string): Promise<ValuationDto[]> =>
    call("list_valuations", { copyId }),
  createListing: (input: CreateListingInput): Promise<SaleListingDto> =>
    call("create_listing", { input }),
  withdrawListing: (listingId: string): Promise<void> =>
    call("withdraw_listing", { listingId }),
  listListings: (copyId: string): Promise<SaleListingDto[]> => call("list_listings", { copyId }),
  recordDisposal: (input: RecordDisposalInput): Promise<DisposalDto> =>
    call("record_disposal", { input }),
  listDisposals: (copyId: string): Promise<DisposalDto[]> => call("list_disposals", { copyId }),
  getSellQueue: (): Promise<SellQueueItem[]> => call("get_sell_queue"),
  setSellOverride: (copyId: string, overrideValue: string | null): Promise<void> =>
    call("set_sell_override", { copyId, overrideValue }),
  getFinanceReport: (): Promise<FinanceReport> => call("get_finance_report"),
  getDashboard: (localToday: string): Promise<DashboardDto> => call("get_dashboard", { localToday }),

  // data portability (Phase 4)
  exportLibraryCsv: (): Promise<ExportResult> => call("export_library_csv"),
  exportLibraryJson: (): Promise<ExportResult> => call("export_library_json"),
  importLibraryJson: (path: string): Promise<ImportResult> =>
    call("import_library_json", { path }),
  previewGoodreadsImport: (path: string): Promise<ImportPreview> =>
    call("preview_goodreads_import", { path }),
  commitGoodreadsImport: (path: string, input: ImportCommitInput): Promise<ImportResult> =>
    call("commit_goodreads_import", { path, input }),
  createBackup: (outputPath?: string | null): Promise<BackupResult> =>
    call("create_backup", { outputPath: outputPath ?? null }),
  restoreBackup: (archivePath: string, mode: string): Promise<RestoreResult> =>
    call("restore_backup", { archivePath, mode }),
};

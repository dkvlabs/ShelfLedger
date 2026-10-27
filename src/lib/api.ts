// Typed wrappers around Tauri IPC commands. The webview never sends SQL —
// only validated DTOs to named commands (AGENTS.md).

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

function unwrap<T>(promise: Promise<T>): Promise<T> {
  return promise.catch((err) => {
    if (err && typeof err === "object" && "kind" in err && "message" in err) {
      throw new CommandError(String(err.kind), String(err.message));
    }
    throw err;
  });
}

export const api = {
  // catalog
  searchWorks: (query?: string | null): Promise<WorkSummary[]> =>
    unwrap(invoke("search_works", { query: query ?? null })),
  getWork: (id: string): Promise<WorkDetail> => unwrap(invoke("get_work", { id })),
  addBook: (input: AddBookInput): Promise<AddBookResult> =>
    unwrap(invoke("add_book", { input })),
  createWork: (input: CreateWorkInput): Promise<WorkDetail> =>
    unwrap(invoke("create_work", { input })),
  updateWork: (id: string, input: UpdateWorkInput): Promise<WorkDetail> =>
    unwrap(invoke("update_work", { id, input })),
  createEdition: (input: CreateEditionInput): Promise<EditionDto> =>
    unwrap(invoke("create_edition", { input })),
  createCopy: (input: CreateCopyInput): Promise<CopyDto> =>
    unwrap(invoke("create_copy", { input })),
  updateCopy: (id: string, input: UpdateCopyInput): Promise<CopyDto> =>
    unwrap(invoke("update_copy", { id, input })),
  archiveCopy: (id: string): Promise<void> => unwrap(invoke("archive_copy", { id })),
  findDuplicates: (query: DuplicateQuery): Promise<WorkSummary[]> =>
    unwrap(invoke("find_duplicates", { query })),
  getLibraryStats: (): Promise<LibraryStats> => unwrap(invoke("get_library_stats")),

  // locations
  listLocations: (): Promise<LocationDto[]> => unwrap(invoke("list_locations")),
  createLocation: (input: CreateLocationInput): Promise<LocationDto> =>
    unwrap(invoke("create_location", { input })),
  deleteLocation: (id: string): Promise<boolean> =>
    unwrap(invoke("delete_location", { id })),

  // settings
  getSettings: (): Promise<Record<string, unknown>> => unwrap(invoke("get_settings")),
  setSetting: (key: string, value: unknown): Promise<void> =>
    unwrap(invoke("set_setting", { key, value })),

  // metadata
  lookupIsbn: (isbn: string): Promise<LookupIsbnResult | null> =>
    unwrap(invoke("lookup_isbn", { isbn })),

  // reading (Phase 2)
  listReadingSessions: (status?: string | null): Promise<ReadingSessionDto[]> =>
    unwrap(invoke("list_reading_sessions", { status: status ?? null })),
  listReadingSessionsForWork: (workId: string): Promise<ReadingSessionDto[]> =>
    unwrap(invoke("list_reading_sessions_for_work", { workId })),
  createReadingSession: (input: CreateReadingSessionInput): Promise<ReadingSessionDto> =>
    unwrap(invoke("create_reading_session", { input })),
  updateReadingSession: (
    id: string,
    input: UpdateReadingSessionInput,
  ): Promise<ReadingSessionDto> =>
    unwrap(invoke("update_reading_session", { id, input })),
  addReadingProgress: (sessionId: string, input: AddProgressInput): Promise<ReadingSessionDto> =>
    unwrap(invoke("add_reading_progress", { sessionId, input })),

  // loans (Phase 2)
  listContacts: (): Promise<ContactDto[]> => unwrap(invoke("list_contacts")),
  createContact: (input: CreateContactInput): Promise<ContactDto> =>
    unwrap(invoke("create_contact", { input })),
  updateContact: (id: string, input: UpdateContactInput): Promise<ContactDto> =>
    unwrap(invoke("update_contact", { id, input })),
  listLoans: (localToday: string): Promise<LoanDto[]> =>
    unwrap(invoke("list_loans", { localToday })),
  listActiveLoans: (localToday: string): Promise<LoanDto[]> =>
    unwrap(invoke("list_active_loans", { localToday })),
  listOverdueLoans: (localToday: string): Promise<LoanDto[]> =>
    unwrap(invoke("list_overdue_loans", { localToday })),
  lendCopy: (input: LendCopyInput): Promise<LoanDto> =>
    unwrap(invoke("lend_copy", { input })),
  returnLoan: (loanId: string, input: ReturnCopyInput): Promise<LoanDto> =>
    unwrap(invoke("return_loan", { loanId, input })),

  // finance (Phase 3)
  addValuation: (input: AddValuationInput): Promise<ValuationDto> =>
    unwrap(invoke("add_valuation", { input })),
  listValuations: (copyId: string): Promise<ValuationDto[]> =>
    unwrap(invoke("list_valuations", { copyId })),
  createListing: (input: CreateListingInput): Promise<SaleListingDto> =>
    unwrap(invoke("create_listing", { input })),
  withdrawListing: (listingId: string): Promise<void> =>
    unwrap(invoke("withdraw_listing", { listingId })),
  listListings: (copyId: string): Promise<SaleListingDto[]> =>
    unwrap(invoke("list_listings", { copyId })),
  recordDisposal: (input: RecordDisposalInput): Promise<DisposalDto> =>
    unwrap(invoke("record_disposal", { input })),
  listDisposals: (copyId: string): Promise<DisposalDto[]> =>
    unwrap(invoke("list_disposals", { copyId })),
  getSellQueue: (): Promise<SellQueueItem[]> => unwrap(invoke("get_sell_queue")),
  setSellOverride: (copyId: string, overrideValue: string | null): Promise<void> =>
    unwrap(invoke("set_sell_override", { copyId, overrideValue })),
  getFinanceReport: (): Promise<FinanceReport> => unwrap(invoke("get_finance_report")),
  getDashboard: (localToday: string): Promise<DashboardDto> =>
    unwrap(invoke("get_dashboard", { localToday })),

  // data portability (Phase 4)
  exportLibraryCsv: (): Promise<ExportResult> => unwrap(invoke("export_library_csv")),
  exportLibraryJson: (): Promise<ExportResult> => unwrap(invoke("export_library_json")),
  importLibraryJson: (path: string): Promise<ImportResult> =>
    unwrap(invoke("import_library_json", { path })),
  previewGoodreadsImport: (path: string): Promise<ImportPreview> =>
    unwrap(invoke("preview_goodreads_import", { path })),
  commitGoodreadsImport: (path: string, input: ImportCommitInput): Promise<ImportResult> =>
    unwrap(invoke("commit_goodreads_import", { path, input })),
  createBackup: (outputPath?: string | null): Promise<BackupResult> =>
    unwrap(invoke("create_backup", { outputPath: outputPath ?? null })),
  restoreBackup: (archivePath: string, mode: string): Promise<RestoreResult> =>
    unwrap(invoke("restore_backup", { archivePath, mode })),
};

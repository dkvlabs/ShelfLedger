// TypeScript counterparts of the Rust DTOs in src-tauri/src/models.rs.
// Keep both sides in sync (AGENTS.md).

export interface WorkDto {
  id: string;
  title: string;
  subtitle: string | null;
  originalLanguage: string | null;
  description: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface AuthorDto {
  id: string;
  name: string;
  sortName: string | null;
}

export interface WorkSummary {
  id: string;
  title: string;
  subtitle: string | null;
  authorNames: string[];
  coverPath: string | null;
  editionCount: number;
  copyCount: number;
}

export interface AcquisitionDto {
  id: string;
  copyId: string;
  acquiredAt: string | null;
  priceMinor: number | null;
  currency: string | null;
  shippingMinor: number | null;
  taxMinor: number | null;
  store: string | null;
  acquisitionType: string | null;
  receiptPath: string | null;
}

export interface CopyDto {
  id: string;
  editionId: string;
  inventoryCode: string | null;
  locationId: string | null;
  locationName: string | null;
  conditionGrade: string;
  ownershipStatus: string;
  acquisitionType: string | null;
  acquiredAt: string | null;
  note: string | null;
  sentimentalScore: number | null;
  archivedAt: string | null;
  createdAt: string;
  updatedAt: string;
  acquisition: AcquisitionDto | null;
}

export interface EditionDto {
  id: string;
  workId: string;
  isbn10: string | null;
  isbn13: string | null;
  publisher: string | null;
  publishedAt: string | null;
  language: string | null;
  pageCount: number | null;
  binding: string | null;
  coverPath: string | null;
  providerIdsJson: string | null;
  createdAt: string;
  copies: CopyDto[];
}

export interface WorkDetail {
  work: WorkDto;
  authors: AuthorDto[];
  editions: EditionDto[];
}

export interface LocationDto {
  id: string;
  parentId: string | null;
  name: string;
  type: "room" | "bookcase" | "shelf" | "box";
  sortOrder: number;
}

export interface LibraryStats {
  workCount: number;
  copyCount: number;
}

export interface LookupIsbnResult {
  source: string;
  isbn13: string | null;
  isbn10: string | null;
  title: string | null;
  subtitle: string | null;
  publisher: string | null;
  publishedAt: string | null;
  language: string | null;
  pageCount: number | null;
  authorNames: string[];
  cached: boolean;
}

// ---- Inputs ----

export interface CreateWorkInput {
  title: string;
  subtitle?: string | null;
  originalLanguage?: string | null;
  description?: string | null;
  authorNames?: string[];
}

export type UpdateWorkInput = CreateWorkInput;

export interface CreateEditionInput {
  workId: string;
  isbn10?: string | null;
  isbn13?: string | null;
  publisher?: string | null;
  publishedAt?: string | null;
  language?: string | null;
  pageCount?: number | null;
  binding?: string | null;
}

export interface CreateAcquisitionInput {
  acquiredAt?: string | null;
  priceMinor?: number | null;
  currency?: string | null;
  shippingMinor?: number | null;
  taxMinor?: number | null;
  store?: string | null;
  acquisitionType?: string | null;
  receiptPath?: string | null;
}

export interface CreateCopyInput {
  editionId: string;
  inventoryCode?: string | null;
  locationId?: string | null;
  conditionGrade: string;
  acquisitionType?: string | null;
  acquiredAt?: string | null;
  note?: string | null;
  sentimentalScore?: number | null;
  acquisition?: CreateAcquisitionInput | null;
}

export interface UpdateCopyInput {
  locationId?: string | null;
  conditionGrade?: string | null;
  note?: string | null;
  sentimentalScore?: number | null;
}

export interface DuplicateQuery {
  isbn?: string | null;
  title?: string | null;
  authorName?: string | null;
}

export interface CreateLocationInput {
  name: string;
  type: LocationDto["type"];
  parentId?: string | null;
  sortOrder?: number | null;
}

// ---- Enumerations (mirror models.rs) ----

export const CONDITION_GRADES = [
  "new",
  "like_new",
  "very_good",
  "good",
  "acceptable",
  "poor",
] as const;

export const OWNERSHIP_STATUSES = [
  "owned",
  "lent",
  "listed",
  "sold",
  "donated",
  "traded",
  "lost",
] as const;

export const ACQUISITION_TYPES = ["bought", "gift", "trade", "inherited"] as const;

export const LOCATION_TYPES = ["room", "bookcase", "shelf", "box"] as const;

export const CONDITION_LABELS: Record<string, string> = {
  new: "New",
  like_new: "Like new",
  very_good: "Very good",
  good: "Good",
  acceptable: "Acceptable",
  poor: "Poor",
};

export const OWNERSHIP_LABELS: Record<string, string> = {
  owned: "Owned",
  lent: "Lent",
  listed: "Listed",
  sold: "Sold",
  donated: "Donated",
  traded: "Traded",
  lost: "Lost",
};

export const ACQUISITION_LABELS: Record<string, string> = {
  bought: "Bought",
  gift: "Gift",
  trade: "Trade",
  inherited: "Inherited",
};

export const LOCATION_LABELS: Record<string, string> = {
  room: "Room",
  bookcase: "Bookcase",
  shelf: "Shelf",
  box: "Box",
};

// ---------------------------------------------------------------------------
// Phase 2 — reading
// ---------------------------------------------------------------------------

export const READING_STATUSES = [
  "want_to_read",
  "reading",
  "finished",
  "paused",
  "dnf",
] as const;

export const READING_STATUS_LABELS: Record<string, string> = {
  want_to_read: "Want to read",
  reading: "Reading",
  finished: "Finished",
  paused: "Paused",
  dnf: "Did not finish",
};

export interface ReadingProgressDto {
  id: string;
  readingSessionId: string;
  occurredAt: string;
  page: number | null;
  percent: number | null;
  note: string | null;
}

export interface ReadingSessionDto {
  id: string;
  workId: string;
  workTitle: string;
  editionId: string | null;
  status: string;
  startedAt: string | null;
  finishedAt: string | null;
  rating: number | null;
  review: string | null;
  createdAt: string;
  updatedAt: string;
  progressEvents: ReadingProgressDto[];
}

export interface CreateReadingSessionInput {
  workId: string;
  editionId?: string | null;
  status: string;
  startedAt?: string | null;
}

export interface UpdateReadingSessionInput {
  status?: string | null;
  startedAt?: string | null;
  finishedAt?: string | null;
  rating?: number | null;
  review?: string | null;
}

export interface AddProgressInput {
  page?: number | null;
  percent?: number | null;
  occurredAt?: string | null;
  note?: string | null;
}

// ---------------------------------------------------------------------------
// Phase 2 — contacts & loans
// ---------------------------------------------------------------------------

export interface ContactDto {
  id: string;
  displayName: string;
  contactInfo: string | null;
  notes: string | null;
  createdAt: string;
}

export interface CreateContactInput {
  displayName: string;
  contactInfo?: string | null;
  notes?: string | null;
}

export interface UpdateContactInput {
  displayName?: string | null;
  contactInfo?: string | null;
  notes?: string | null;
}

export interface LoanDto {
  id: string;
  copyId: string;
  contactId: string;
  contactName: string;
  workTitle: string;
  editionLabel: string;
  lentAt: string;
  dueAt: string | null;
  returnedAt: string | null;
  notes: string | null;
  overdue: boolean;
}

export interface LendCopyInput {
  copyId: string;
  contactId: string;
  lentAt?: string | null;
  dueAt?: string | null;
  notes?: string | null;
}

export interface ReturnCopyInput {
  returnedAt?: string | null;
  conditionGrade?: string | null;
  note?: string | null;
}

// ---------------------------------------------------------------------------
// Phase 3 — finance
// ---------------------------------------------------------------------------

export interface ValuationDto {
  id: string;
  copyId: string;
  valueMinor: number;
  currency: string;
  sourceType: string;
  sourceNote: string | null;
  observedAt: string;
  confidence: string;
}

export interface AddValuationInput {
  copyId: string;
  valueMinor: number;
  currency: string;
  sourceType: string;
  sourceNote?: string | null;
  observedAt?: string | null;
  confidence?: string;
}

export interface SaleListingDto {
  id: string;
  copyId: string;
  platform: string | null;
  listedAt: string;
  askingPriceMinor: number;
  currency: string;
  status: string;
}

export interface CreateListingInput {
  copyId: string;
  platform?: string | null;
  askingPriceMinor: number;
  currency: string;
}

export interface DisposalDto {
  id: string;
  copyId: string;
  disposition: string;
  date: string;
  salePriceMinor: number | null;
  feeMinor: number | null;
  shippingMinor: number | null;
  currency: string | null;
  buyerAlias: string | null;
  notes: string | null;
}

export interface RecordDisposalInput {
  copyId: string;
  disposition: "sold" | "donated" | "traded" | "lost";
  date?: string | null;
  salePriceMinor?: number | null;
  feeMinor?: number | null;
  shippingMinor?: number | null;
  currency?: string | null;
  buyerAlias?: string | null;
  notes?: string | null;
}

export interface HistoricalCost {
  minor: number;
  currency: string;
}

export interface SellQueueItem {
  copyId: string;
  workTitle: string;
  authorNames: string[];
  conditionGrade: string;
  copyNumber: number;
  totalCopiesOfWork: number;
  historicalCost: HistoricalCost | null;
  latestValuation: ValuationDto | null;
  /** Active sale listing while listed for sale; null otherwise. */
  activeListing: SaleListingDto | null;
  lastFinishedAt: string | null;
  bestRating: number | null;
  sentimentalScore: number | null;
  decision: string;
  reasons: string[];
}

export interface ReportBucket {
  currency: string;
  totalMinor: number;
}

export interface YearReport {
  year: string;
  buckets: ReportBucket[];
}

export interface FinanceReport {
  spending: YearReport[];
  proceeds: YearReport[];
  estimatedValue: ReportBucket[];
}

// ---------------------------------------------------------------------------
// Phase 2 — dashboard
// ---------------------------------------------------------------------------

export interface ReadingNow {
  sessionId: string;
  workId: string;
  workTitle: string;
  startedAt: string | null;
  latestPage: number | null;
}

export interface DashboardDto {
  workCount: number;
  ownedCopyCount: number;
  unreadCount: number;
  currentlyReading: ReadingNow[];
  activeLoans: number;
  overdueLoans: number;
  spendingByCurrency: ReportBucket[];
  estimatedValueByCurrency: ReportBucket[];
  proceedsByCurrency: ReportBucket[];
}

// ---------------------------------------------------------------------------
// Phase 4 — export / backup / import
// ---------------------------------------------------------------------------

export interface ExportResult {
  path: string;
  format: string;
}

export interface BackupResult {
  path: string;
}

export interface RestoreResult {
  message: string;
  requiresRestart: boolean;
}

export interface ImportRowPreview {
  rowNumber: number;
  title: string;
  author: string;
  isbn13: string | null;
  shelf: string | null;
  rating: number | null;
  dateRead: string | null;
  warnings: string[];
  duplicateOf: string | null;
}

export interface ImportPreview {
  source: string;
  rows: ImportRowPreview[];
}

export interface ImportCommitInput {
  rowNumbers: number[];
  duplicatePolicy: "skip" | "add_copy" | "new_work";
}

export interface ImportResult {
  importedWorks: number;
  importedEditions: number;
  importedCopies: number;
  skipped: number;
  errors: string[];
}

// ---------------------------------------------------------------------------
// Transactional add-book (work + edition + copy in one IPC call)
// ---------------------------------------------------------------------------

export interface AddBookCopyInput {
  inventoryCode?: string | null;
  locationId?: string | null;
  conditionGrade: string;
  acquisitionType?: string | null;
  acquiredAt?: string | null;
  note?: string | null;
  sentimentalScore?: number | null;
  acquisition?: CreateAcquisitionInput | null;
}

export interface AddBookInput {
  title: string;
  subtitle?: string | null;
  originalLanguage?: string | null;
  description?: string | null;
  authorNames?: string[];
  isbn10?: string | null;
  isbn13?: string | null;
  publisher?: string | null;
  publishedAt?: string | null;
  language?: string | null;
  pageCount?: number | null;
  binding?: string | null;
  copy?: AddBookCopyInput | null;
  /** "Add anyway": create the work but drop a conflicting ISBN (reported). */
  dropConflictingIsbn?: boolean;
}

export interface AddBookResult {
  workId: string;
  editionId: string;
  copyId: string | null;
  warning: string | null;
}

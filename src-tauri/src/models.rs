//! DTOs crossing the Tauri IPC boundary. The TypeScript counterparts live in
//! `src/types/domain.ts` and must be kept in sync (see AGENTS.md).

use serde::{Deserialize, Serialize};

pub const CONDITION_GRADES: &[&str] =
    &["new", "like_new", "very_good", "good", "acceptable", "poor"];

#[allow(dead_code)] // Phase 2/3 flows (loans, disposals) read this list
pub const OWNERSHIP_STATUSES: &[&str] = &[
    "owned", "lent", "listed", "sold", "donated", "traded", "lost",
];

pub const ACQUISITION_TYPES: &[&str] = &["bought", "gift", "trade", "inherited"];

pub const LOCATION_TYPES: &[&str] = &["room", "bookcase", "shelf", "box"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkDto {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub original_language: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorDto {
    pub id: String,
    pub name: String,
    pub sort_name: Option<String>,
}

/// Compact row for list/search results.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub author_names: Vec<String>,
    pub cover_path: Option<String>,
    pub edition_count: i64,
    pub copy_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionDto {
    pub id: String,
    pub work_id: String,
    pub isbn10: Option<String>,
    pub isbn13: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i64>,
    pub binding: Option<String>,
    pub cover_path: Option<String>,
    pub provider_ids_json: Option<String>,
    pub created_at: String,
    pub copies: Vec<CopyDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionDto {
    pub id: String,
    pub copy_id: String,
    pub acquired_at: Option<String>,
    pub price_minor: Option<i64>,
    pub currency: Option<String>,
    pub shipping_minor: Option<i64>,
    pub tax_minor: Option<i64>,
    pub store: Option<String>,
    pub acquisition_type: Option<String>,
    pub receipt_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyDto {
    pub id: String,
    pub edition_id: String,
    pub inventory_code: Option<String>,
    pub location_id: Option<String>,
    pub location_name: Option<String>,
    pub condition_grade: String,
    pub ownership_status: String,
    pub acquisition_type: Option<String>,
    pub acquired_at: Option<String>,
    pub note: Option<String>,
    pub sentimental_score: Option<i64>,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub acquisition: Option<AcquisitionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkDetail {
    pub work: WorkDto,
    pub authors: Vec<AuthorDto>,
    pub editions: Vec<EditionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationDto {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub sort_order: i64,
}

/// Minimal dashboard counters (expanded in later phases).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub work_count: i64,
    pub copy_count: i64,
}

// ---------------------------------------------------------------------------
// Transactional add-book (workflow A: "Save transactionally")
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBookCopyInput {
    pub inventory_code: Option<String>,
    pub location_id: Option<String>,
    #[serde(default = "default_condition")]
    pub condition_grade: String,
    pub acquisition_type: Option<String>,
    pub acquired_at: Option<String>,
    pub note: Option<String>,
    pub sentimental_score: Option<i64>,
    pub acquisition: Option<CreateAcquisitionInput>,
}

impl AddBookCopyInput {
    pub fn validate(&self) -> Result<(), String> {
        if !CONDITION_GRADES.contains(&self.condition_grade.as_str()) {
            return Err(format!(
                "unknown condition grade {:?}",
                self.condition_grade
            ));
        }
        if let Some(t) = &self.acquisition_type {
            if !ACQUISITION_TYPES.contains(&t.as_str()) {
                return Err(format!("unknown acquisition type {t:?}"));
            }
        }
        if let Some(score) = self.sentimental_score {
            if !(0..=5).contains(&score) {
                return Err("sentimental score must be between 0 and 5".into());
            }
        }
        if let Some(acq) = &self.acquisition {
            acq.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBookInput {
    // work
    pub title: String,
    pub subtitle: Option<String>,
    pub original_language: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub author_names: Vec<String>,
    // edition
    pub isbn10: Option<String>,
    pub isbn13: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i64>,
    pub binding: Option<String>,
    // optional owned copy
    pub copy: Option<AddBookCopyInput>,
    /// When the ISBN already belongs to another edition: create the work and
    /// edition anyway but drop the duplicate ISBN (reported as a warning)
    /// instead of failing or cloning ISBNs.
    #[serde(default)]
    pub drop_conflicting_isbn: bool,
}

impl AddBookInput {
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.title) {
            return Err("title is required".into());
        }
        if let Some(copy) = &self.copy {
            copy.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBookResult {
    pub work_id: String,
    pub edition_id: String,
    pub copy_id: Option<String>,
    pub warning: Option<String>,
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkInput {
    pub title: String,
    pub subtitle: Option<String>,
    pub original_language: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub author_names: Vec<String>,
}

impl CreateWorkInput {
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.title) {
            return Err("title is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkInput {
    pub title: String,
    pub subtitle: Option<String>,
    pub original_language: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub author_names: Vec<String>,
}

impl UpdateWorkInput {
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.title) {
            return Err("title is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEditionInput {
    pub work_id: String,
    pub isbn10: Option<String>,
    pub isbn13: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i64>,
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAcquisitionInput {
    pub acquired_at: Option<String>,
    pub price_minor: Option<i64>,
    pub currency: Option<String>,
    pub shipping_minor: Option<i64>,
    pub tax_minor: Option<i64>,
    pub store: Option<String>,
    pub acquisition_type: Option<String>,
    pub receipt_path: Option<String>,
}

impl CreateAcquisitionInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(price) = self.price_minor {
            if price < 0 {
                return Err("price cannot be negative".into());
            }
            match &self.currency {
                Some(c) => crate::money::validate_currency(c)?,
                None => return Err("currency is required when a purchase price is recorded".into()),
            }
        }
        if self.currency.is_some() && self.price_minor.is_none() {
            return Err("a purchase price is required when a currency is recorded".into());
        }
        if let Some(t) = &self.acquisition_type {
            if !ACQUISITION_TYPES.contains(&t.as_str()) {
                return Err(format!("unknown acquisition type {t:?}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCopyInput {
    pub edition_id: String,
    pub inventory_code: Option<String>,
    pub location_id: Option<String>,
    #[serde(default = "default_condition")]
    pub condition_grade: String,
    pub acquisition_type: Option<String>,
    pub acquired_at: Option<String>,
    pub note: Option<String>,
    pub sentimental_score: Option<i64>,
    pub acquisition: Option<CreateAcquisitionInput>,
}

fn default_condition() -> String {
    "good".to_string()
}

impl CreateCopyInput {
    pub fn validate(&self) -> Result<(), String> {
        if !CONDITION_GRADES.contains(&self.condition_grade.as_str()) {
            return Err(format!(
                "unknown condition grade {:?}",
                self.condition_grade
            ));
        }
        if let Some(t) = &self.acquisition_type {
            if !ACQUISITION_TYPES.contains(&t.as_str()) {
                return Err(format!("unknown acquisition type {t:?}"));
            }
        }
        if let Some(score) = self.sentimental_score {
            if !(0..=5).contains(&score) {
                return Err("sentimental score must be between 0 and 5".into());
            }
        }
        if let Some(acq) = &self.acquisition {
            acq.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCopyInput {
    pub location_id: Option<String>,
    pub condition_grade: Option<String>,
    pub note: Option<String>,
    pub sentimental_score: Option<i64>,
}

impl UpdateCopyInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(g) = &self.condition_grade {
            if !CONDITION_GRADES.contains(&g.as_str()) {
                return Err(format!("unknown condition grade {g:?}"));
            }
        }
        if let Some(score) = self.sentimental_score {
            if !(0..=5).contains(&score) {
                return Err("sentimental score must be between 0 and 5".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocationInput {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub parent_id: Option<String>,
    pub sort_order: Option<i64>,
}

impl CreateLocationInput {
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.name) {
            return Err("location name is required".into());
        }
        if !LOCATION_TYPES.contains(&self.kind.as_str()) {
            return Err(format!("unknown location type {:?}", self.kind));
        }
        Ok(())
    }
}

/// Duplicate-detection query: either an ISBN or title (+optional author).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateQuery {
    pub isbn: Option<String>,
    pub title: Option<String>,
    pub author_name: Option<String>,
}

/// Metadata lookup result from a provider (Open Library) or cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupIsbnResult {
    pub source: String,
    pub isbn13: Option<String>,
    pub isbn10: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i64>,
    pub author_names: Vec<String>,
    pub cached: bool,
}

// ---------------------------------------------------------------------------
// Phase 2 — reading
// ---------------------------------------------------------------------------

pub const READING_STATUSES: &[&str] = &["want_to_read", "reading", "finished", "paused", "dnf"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingSessionDto {
    pub id: String,
    pub work_id: String,
    pub work_title: String,
    pub edition_id: Option<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub rating: Option<i64>,
    pub review: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub progress_events: Vec<ReadingProgressDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgressDto {
    pub id: String,
    pub reading_session_id: String,
    pub occurred_at: String,
    pub page: Option<i64>,
    pub percent: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReadingSessionInput {
    pub work_id: String,
    pub edition_id: Option<String>,
    #[serde(default = "default_reading_status")]
    pub status: String,
    pub started_at: Option<String>,
}

fn default_reading_status() -> String {
    "want_to_read".to_string()
}

impl CreateReadingSessionInput {
    pub fn validate(&self) -> Result<(), String> {
        if !READING_STATUSES.contains(&self.status.as_str()) {
            return Err(format!("unknown reading status {:?}", self.status));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReadingSessionInput {
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub rating: Option<i64>,
    pub review: Option<String>,
}

impl UpdateReadingSessionInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(s) = &self.status {
            if !READING_STATUSES.contains(&s.as_str()) {
                return Err(format!("unknown reading status {s:?}"));
            }
        }
        if let Some(r) = self.rating {
            if !(0..=5).contains(&r) {
                return Err("rating must be between 0 and 5".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProgressInput {
    pub page: Option<i64>,
    pub percent: Option<i64>,
    pub occurred_at: Option<String>,
    pub note: Option<String>,
}

impl AddProgressInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.page.is_none() && self.percent.is_none() {
            return Err("progress needs a page or a percent".into());
        }
        if let Some(p) = self.percent {
            if !(0..=100).contains(&p) {
                return Err("percent must be between 0 and 100".into());
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Phase 2 — contacts & loans
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub id: String,
    pub display_name: String,
    pub contact_info: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContactInput {
    pub display_name: String,
    pub contact_info: Option<String>,
    pub notes: Option<String>,
}

impl CreateContactInput {
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.display_name) {
            return Err("contact name is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContactInput {
    pub display_name: Option<String>,
    pub contact_info: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanDto {
    pub id: String,
    pub copy_id: String,
    pub contact_id: String,
    pub contact_name: String,
    pub work_title: String,
    pub edition_label: String,
    pub lent_at: String,
    pub due_at: Option<String>,
    pub returned_at: Option<String>,
    pub notes: Option<String>,
    /// Overdue = active loan whose due date passed the user's local date.
    pub overdue: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LendCopyInput {
    pub copy_id: String,
    pub contact_id: String,
    pub lent_at: Option<String>,
    pub due_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnCopyInput {
    pub returned_at: Option<String>,
    pub condition_grade: Option<String>,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 3 — valuation, listings, disposals, sell queue, reports
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationDto {
    pub id: String,
    pub copy_id: String,
    pub value_minor: i64,
    pub currency: String,
    pub source_type: String,
    pub source_note: Option<String>,
    pub observed_at: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddValuationInput {
    pub copy_id: String,
    pub value_minor: i64,
    pub currency: String,
    pub source_type: String,
    pub source_note: Option<String>,
    pub observed_at: Option<String>,
    #[serde(default = "default_confidence")]
    pub confidence: String,
}

fn default_confidence() -> String {
    "low".to_string()
}

impl AddValuationInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.value_minor < 0 {
            return Err("value cannot be negative".into());
        }
        crate::money::validate_currency(&self.currency)?;
        if !matches!(self.source_type.as_str(), "manual" | "comparable") {
            return Err(format!("unknown valuation source {:?}", self.source_type));
        }
        if !matches!(self.confidence.as_str(), "low" | "medium" | "high") {
            return Err(format!("unknown confidence level {:?}", self.confidence));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleListingDto {
    pub id: String,
    pub copy_id: String,
    pub platform: Option<String>,
    pub listed_at: String,
    pub asking_price_minor: i64,
    pub currency: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateListingInput {
    pub copy_id: String,
    pub platform: Option<String>,
    pub asking_price_minor: i64,
    pub currency: String,
}

impl CreateListingInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.asking_price_minor < 0 {
            return Err("asking price cannot be negative".into());
        }
        crate::money::validate_currency(&self.currency)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisposalDto {
    pub id: String,
    pub copy_id: String,
    pub disposition: String,
    pub date: String,
    pub sale_price_minor: Option<i64>,
    pub fee_minor: Option<i64>,
    pub shipping_minor: Option<i64>,
    pub currency: Option<String>,
    pub buyer_alias: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDisposalInput {
    pub copy_id: String,
    pub disposition: String,
    pub date: Option<String>,
    pub sale_price_minor: Option<i64>,
    pub fee_minor: Option<i64>,
    pub shipping_minor: Option<i64>,
    pub currency: Option<String>,
    pub buyer_alias: Option<String>,
    pub notes: Option<String>,
}

impl RecordDisposalInput {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.disposition.as_str(),
            "sold" | "donated" | "traded" | "lost"
        ) {
            return Err(format!("unknown disposition {:?}", self.disposition));
        }
        if let Some(p) = self.sale_price_minor {
            if p < 0 {
                return Err("sale price cannot be negative".into());
            }
        }
        if self.sale_price_minor.is_some() {
            match &self.currency {
                Some(c) => crate::money::validate_currency(c)?,
                None => {
                    return Err("currency is required when a sale price is recorded".into());
                }
            }
        }
        Ok(())
    }
}

/// One row of the transparent sell/keep/donate decision assistant.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SellQueueItem {
    pub copy_id: String,
    pub work_title: String,
    pub author_names: Vec<String>,
    pub condition_grade: String,
    pub copy_number: i64,
    pub total_copies_of_work: i64,
    pub historical_cost: Option<HistoricalCost>,
    pub latest_valuation: Option<ValuationDto>,
    /// Active sale listing while the copy is listed; none otherwise.
    pub active_listing: Option<SaleListingDto>,
    pub last_finished_at: Option<String>,
    pub best_rating: Option<i64>,
    pub sentimental_score: Option<i64>,
    pub decision: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalCost {
    pub minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportBucket {
    pub currency: String,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearReport {
    pub year: String,
    pub buckets: Vec<ReportBucket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceReport {
    pub spending: Vec<YearReport>,
    pub proceeds: Vec<YearReport>,
    pub estimated_value: Vec<ReportBucket>,
}

// ---------------------------------------------------------------------------
// Phase 2 — dashboard
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingNow {
    pub session_id: String,
    pub work_id: String,
    pub work_title: String,
    pub started_at: Option<String>,
    pub latest_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDto {
    pub work_count: i64,
    pub owned_copy_count: i64,
    pub unread_count: i64,
    pub currently_reading: Vec<ReadingNow>,
    pub active_loans: i64,
    pub overdue_loans: i64,
    pub spending_by_currency: Vec<ReportBucket>,
    pub estimated_value_by_currency: Vec<ReportBucket>,
    pub proceeds_by_currency: Vec<ReportBucket>,
}

// ---------------------------------------------------------------------------
// Phase 4 — export / backup / import
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub message: String,
    pub requires_restart: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRowPreview {
    pub row_number: usize,
    pub title: String,
    pub author: String,
    pub isbn13: Option<String>,
    pub shelf: Option<String>,
    pub rating: Option<i64>,
    pub date_read: Option<String>,
    pub warnings: Vec<String>,
    /// Set when an exact-ISBN or title+author duplicate already exists.
    pub duplicate_of: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub source: String,
    pub rows: Vec<ImportRowPreview>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitInput {
    /// Which preview rows to import (by row_number), after the user resolved
    /// duplicate decisions in the UI.
    pub row_numbers: Vec<usize>,
    /// "skip" drops duplicate rows; "add_copy" adds a copy to the existing
    /// edition; "new_work" always creates a new work.
    #[serde(default = "default_duplicate_policy")]
    pub duplicate_policy: String,
}

fn default_duplicate_policy() -> String {
    "skip".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported_works: i64,
    pub imported_editions: i64,
    pub imported_copies: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

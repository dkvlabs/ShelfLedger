//! Offline comparisons of user-verified asking prices. No scraping or sale claims.
use crate::{
    error::{AppError, AppResult},
    ids,
    models::ValuationDto,
    repositories::market as repo,
    sqlite_db::SqlitePool,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketComparable {
    pub id: String,
    pub edition_id: String,
    pub listing_url: String,
    pub asking_minor: i64,
    pub currency: String,
    pub condition_note: Option<String>,
    pub observed_at: String,
    pub included: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveMarketComparableInput {
    pub edition_id: String,
    pub listing_url: String,
    pub asking_minor: i64,
    pub currency: String,
    pub condition_note: Option<String>,
    pub observed_at: String,
    pub edition_checked: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveMarketEstimateInput {
    pub edition_id: String,
    pub copy_id: String,
    pub currency: String,
    pub confidence: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSummary {
    pub currency: String,
    pub count: usize,
    pub min_minor: i64,
    pub max_minor: i64,
    pub median_minor: i64,
    pub median_rounded: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketReport {
    pub comparables: Vec<MarketComparable>,
    pub summaries: Vec<MarketSummary>,
}

pub fn olx_url(value: &str, listing_only: bool) -> AppResult<String> {
    let invalid =
        || AppError::Validation("Use a public HTTPS OLX Romania listing or book search URL".into());
    if value.len() > 2048 {
        return Err(invalid());
    }
    let mut url = reqwest::Url::parse(value.trim()).map_err(|_| invalid())?;
    let listing = url.path().starts_with("/d/oferta/")
        && url.path().ends_with(".html")
        && url.path().len() > 20;
    let search = url
        .path()
        .starts_with("/hobby-sport-turism/carti-muzica-filme/carti/");
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("olx.ro" | "www.olx.ro"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !(listing || (!listing_only && search))
    {
        return Err(invalid());
    }
    url.set_host(Some("www.olx.ro")).map_err(|_| invalid())?;
    url.set_query(None);
    url.set_fragment(None);
    let result = url.to_string();
    if result.len() > 2048 {
        return Err(invalid());
    }
    Ok(result)
}

pub fn summaries(rows: &[MarketComparable]) -> Vec<MarketSummary> {
    let mut groups: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.included) {
        groups
            .entry(row.currency.clone())
            .or_default()
            .push(row.asking_minor);
    }
    groups
        .into_iter()
        .map(|(currency, mut values)| {
            values.sort_unstable();
            let count = values.len();
            let low = values[(count - 1) / 2];
            let high = values[count / 2];
            MarketSummary {
                currency,
                count,
                min_minor: values[0],
                max_minor: values[count - 1],
                median_minor: low + (high - low) / 2,
                median_rounded: (high - low) % 2 != 0,
            }
        })
        .collect()
}
pub async fn report(pool: &SqlitePool, edition_id: &str) -> AppResult<MarketReport> {
    if crate::repositories::editions::get_edition(pool, edition_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("edition".into()));
    }
    let comparables = repo::list(pool, edition_id).await?;
    Ok(MarketReport {
        summaries: summaries(&comparables),
        comparables,
    })
}
pub async fn save(pool: &SqlitePool, input: &SaveMarketComparableInput) -> AppResult<()> {
    let url = olx_url(&input.listing_url, true)?;
    crate::money::validate_currency(&input.currency).map_err(AppError::Validation)?;
    if !input.edition_checked
        || !(0..=9_007_199_254_740_991).contains(&input.asking_minor)
        || input
            .condition_note
            .as_ref()
            .is_some_and(|v| v.chars().count() > 2000)
        || input.observed_at.len() != 10
        || chrono::NaiveDate::parse_from_str(&input.observed_at, "%Y-%m-%d").is_err()
    {
        return Err(AppError::Validation(
            "Check the edition, date, notes, and nonnegative price".into(),
        ));
    }
    if crate::repositories::editions::get_edition(pool, &input.edition_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("edition".into()));
    }
    repo::save(pool, input, &url).await
}
pub async fn set_included(pool: &SqlitePool, id: &str, included: bool) -> AppResult<()> {
    repo::set_included(pool, id, included).await
}
pub async fn save_estimate(
    pool: &SqlitePool,
    input: &SaveMarketEstimateInput,
) -> AppResult<ValuationDto> {
    if !matches!(input.confidence.as_str(), "low" | "medium" | "high") {
        return Err(AppError::Validation("invalid confidence".into()));
    }
    crate::money::validate_currency(&input.currency).map_err(AppError::Validation)?;
    let currency = input.currency.to_ascii_uppercase();
    let mut tx = pool.begin().await?;
    repo::check_copy(&mut tx, &input.copy_id, &input.edition_id).await?;
    let rows = repo::list(&mut *tx, &input.edition_id).await?;
    let summary = summaries(&rows)
        .into_iter()
        .find(|s| s.currency == currency)
        .ok_or_else(|| {
            AppError::Validation("include at least one comparison in this currency".into())
        })?;
    let mut note = format!("OLX advertised asking prices, not completed sales. Median of {} checked listings; amounts in {} minor units.{}", summary.count, currency, if summary.median_rounded { " Median rounded down by half a minor unit." } else { "" });
    for row in rows.iter().filter(|r| r.included && r.currency == currency) {
        use std::fmt::Write;
        let _ = write!(
            note,
            "\n{} | {} {} minor units | observed {} | {}",
            row.listing_url,
            row.asking_minor,
            row.currency,
            row.observed_at,
            row.condition_note.as_deref().unwrap_or("")
        );
    }
    let result = ValuationDto {
        id: ids::new_id(),
        copy_id: input.copy_id.clone(),
        value_minor: summary.median_minor,
        currency,
        source_type: "comparable".into(),
        source_note: Some(note),
        observed_at: ids::now_iso(),
        confidence: input.confidence.clone(),
    };
    repo::insert_estimate(&mut tx, &result).await?;
    tx.commit().await?;
    Ok(result)
}

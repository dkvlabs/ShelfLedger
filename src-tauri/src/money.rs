//! Money handling (docs/database.md): integer minor units + ISO 4217 code.
//! Never floats; never sum across currencies. Conversions between a user's
//! decimal input and minor units happen at the edge (UI or import) and are
//! validated here.

/// Supported fixed-scale ISO currencies, plus explicitly retained legacy codes.
/// The frontend and backend consume the same bundled catalog.
pub fn validate_currency(code: &str) -> Result<(), String> {
    static CURRENCIES: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../../src/lib/currencies.json"))
            .expect("bundled ISO currency data")
    });
    if CURRENCIES.get(code.to_ascii_uppercase()).is_some() {
        Ok(())
    } else {
        Err(format!("unsupported ISO 4217 currency: {code}"))
    }
}

/// Version-one JSON snapshots used incorrect zero-decimal scales for these
/// currencies. Match migration 0005 without rounding or risking overflow.
pub fn upgrade_legacy_record(
    table: &str,
    record: &mut std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let columns: &[&str] = match table {
        "acquisitions" => &["price_minor", "shipping_minor", "tax_minor"],
        "valuation_records" => &["value_minor"],
        "sale_listings" => &["asking_price_minor"],
        "disposals" => &["sale_price_minor", "fee_minor", "shipping_minor"],
        "wishlist" => &["maximum_price_minor"],
        _ => return Ok(()),
    };
    let factors: serde_json::Value =
        serde_json::from_str(include_str!("../../src/lib/legacy-currency-factors.json"))
            .expect("bundled legacy scales");
    let factor = record
        .get("currency")
        .and_then(serde_json::Value::as_str)
        .and_then(|c| factors.get(c.to_ascii_uppercase()))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(1);
    for column in columns {
        if let Some(value) = record.get_mut(*column) {
            if value.is_null() {
                continue;
            }
            let upgraded = value
                .as_i64()
                .and_then(|v| v.checked_mul(factor))
                .filter(|v| *v >= 0 && *v <= 9_007_199_254_740_991)
                .ok_or_else(|| "legacy money amount cannot be safely upgraded".to_string())?;
            *value = upgraded.into();
        }
    }
    Ok(())
}

/// Parse a decimal money string ("12.50", "12,50", "1 234,00") into integer
/// minor units for a 2-decimal currency. Returns None when unparseable.
#[allow(dead_code)] // used by import parsing (Phase 4); mirrored in src/lib/format.ts
pub fn parse_amount_minor(input: &str) -> Option<i64> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\'' && *c != '\u{00a0}')
        .collect();
    let normalized = if cleaned.contains(',') && cleaned.contains('.') {
        // Assume the last separator is the decimal point.
        if cleaned.rfind(',') > cleaned.rfind('.') {
            cleaned.replace('.', "").replace(',', ".")
        } else {
            cleaned.replace(',', "")
        }
    } else {
        cleaned.replace(',', ".")
    };
    let negative = normalized.starts_with('-');
    let body = normalized.trim_start_matches(['-', '+']);
    let (whole, frac) = match body.split_once('.') {
        Some((w, f)) => (w, f),
        None => (body, ""),
    };
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || frac.len() > 2 {
        return None;
    }
    let frac_padded = format!("{frac:0<2}");
    if !frac_padded.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole_v: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac_v: i64 = frac_padded.parse().ok()?;
    let minor = whole_v.checked_mul(100)?.checked_add(frac_v)?;
    Some(if negative { -minor } else { minor })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_iso_codes() {
        assert!(validate_currency("RON").is_ok());
        assert!(validate_currency("EUR").is_ok());
        assert!(validate_currency("usd").is_ok()); // callers normalize case
        assert!(validate_currency("EU").is_err());
        assert!(validate_currency("EURO").is_err());
        assert!(validate_currency("E1R").is_err());
        assert!(validate_currency("").is_err());
    }

    #[test]
    fn parses_decimal_to_minor_units() {
        assert_eq!(parse_amount_minor("12.50"), Some(1250));
        assert_eq!(parse_amount_minor("12,50"), Some(1250));
        assert_eq!(parse_amount_minor("0.05"), Some(5));
        assert_eq!(parse_amount_minor("5"), Some(500));
        assert_eq!(parse_amount_minor("1234.00"), Some(123_400));
        assert_eq!(parse_amount_minor("1 234,50"), Some(123_450));
        assert_eq!(parse_amount_minor("1'234.50"), Some(123_450));
        assert_eq!(parse_amount_minor("-9.99"), Some(-999));
    }

    #[test]
    fn rejects_bad_amounts() {
        assert_eq!(parse_amount_minor(""), None);
        assert_eq!(parse_amount_minor("abc"), None);
        assert_eq!(parse_amount_minor("12.345"), None); // >2 decimals
        assert_eq!(parse_amount_minor("1.2.3"), None);
    }
}

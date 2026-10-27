//! Money handling (docs/database.md): integer minor units + ISO 4217 code.
//! Never floats; never sum across currencies. Conversions between a user's
//! decimal input and minor units happen at the edge (UI or import) and are
//! validated here.

/// Validate an ISO 4217 currency code: exactly three ASCII letters.
/// We do not hard-code a currency list so any ISO currency works (RON, EUR,
/// USD, …), matching the product requirement "other ISO currencies supported".
pub fn validate_currency(code: &str) -> Result<(), String> {
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(format!(
            "currency must be a 3-letter ISO 4217 code, got {code:?}"
        ))
    }
}

/// Default currencies surfaced in the UI (§17 of the plan: English/Romanian,
/// RON/EUR with other ISO currencies supported).
#[allow(dead_code)] // surfaced by the settings UI
pub const DEFAULT_CURRENCIES: &[&str] = &["RON", "EUR", "USD", "GBP"];

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
    let minor = whole_v * 100 + frac_v;
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

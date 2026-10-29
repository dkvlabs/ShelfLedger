//! ISBN parsing, checksum validation and ISBN-10/13 conversion.
//!
//! User input is messy: hyphens, spaces, lowercase `x`. `normalize` strips
//! everything except uppercase alphanumerics; `parse` validates checksums and
//! returns both forms when convertible.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedIsbn {
    pub isbn13: Option<String>,
    pub isbn10: Option<String>,
}

/// Keep only `[0-9A-Z]`, uppercasing lowercase input.
pub fn normalize(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn digits_only(s: &str) -> Option<Vec<u8>> {
    s.chars().map(|c| c.to_digit(10).map(|d| d as u8)).collect()
}

pub fn is_valid_isbn13(s: &str) -> bool {
    let Some(digits) = digits_only(s) else {
        return false;
    };
    if digits.len() != 13 || !s.starts_with("978") && !s.starts_with("979") {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| (d as u32) * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    sum.is_multiple_of(10)
}

pub fn is_valid_isbn10(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 10 {
        return false;
    }
    let mut values = Vec::with_capacity(10);
    for (i, c) in chars.iter().enumerate() {
        let v = if i == 9 && (*c == 'X' || *c == 'x') {
            10
        } else {
            match c.to_digit(10) {
                Some(d) => d,
                None => return false,
            }
        };
        values.push(v);
    }
    let sum: u32 = values
        .iter()
        .enumerate()
        .map(|(i, v)| v * (10 - i as u32))
        .sum();
    sum.is_multiple_of(11)
}

/// Convert a *valid* ISBN-10 to its ISBN-13 form (978 prefix).
pub fn isbn10_to_isbn13(isbn10: &str) -> Option<String> {
    if !is_valid_isbn10(isbn10) {
        return None;
    }
    let digits = digits_only(isbn10.get(..9)?)?;
    let mut first12: Vec<u8> = vec![9, 7, 8];
    first12.extend_from_slice(&digits[..9]);
    let sum: u32 = first12
        .iter()
        .enumerate()
        .map(|(i, &d)| (d as u32) * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    let mut out = String::with_capacity(13);
    for d in first12 {
        out.push((b'0' + d) as char);
    }
    out.push((b'0' + check as u8) as char);
    Some(out)
}

/// Parse raw user/provider input into validated ISBN forms.
/// Returns `None` when the input is neither a valid ISBN-10 nor ISBN-13.
pub fn parse(raw: &str) -> Option<NormalizedIsbn> {
    let s = normalize(raw);
    if s.is_empty() {
        return None;
    }
    if s.len() == 13 && s.chars().all(|c| c.is_ascii_digit()) && is_valid_isbn13(&s) {
        return Some(NormalizedIsbn {
            isbn13: Some(s.clone()),
            isbn10: isbn13_to_isbn10(&s),
        });
    }
    if s.len() == 10 && is_valid_isbn10(&s) {
        return Some(NormalizedIsbn {
            isbn13: isbn10_to_isbn13(&s),
            isbn10: Some(s),
        });
    }
    None
}

/// Best-effort ISBN-13 -> ISBN-10 (only 978/979 prefix range maps cleanly).
pub fn isbn13_to_isbn10(isbn13: &str) -> Option<String> {
    if !isbn13.starts_with("978") {
        return None;
    }
    let digits = digits_only(isbn13)?;
    if digits.len() != 13 {
        return None;
    }
    let mut values: Vec<u32> = digits[3..12].iter().map(|&d| d as u32).collect();
    let sum: u32 = values
        .iter()
        .enumerate()
        .map(|(i, v)| v * (10 - i as u32))
        .sum();
    let check = (11 - (sum % 11)) % 11;
    values.push(check);
    let out: String = values
        .iter()
        .map(|&v| {
            if v == 10 {
                'X'
            } else {
                (b'0' + v as u8) as char
            }
        })
        .collect();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_separators_and_uppercases() {
        assert_eq!(normalize(" 978-0-306-40615-7 "), "9780306406157");
        assert_eq!(normalize("0-19-852663-6"), "0198526636");
        assert_eq!(normalize("978x0"), "978X0");
    }

    #[test]
    fn valid_isbn13_accepted() {
        assert!(parse("978-0-306-40615-7").is_some());
        assert!(parse("9780306406157").is_some());
        assert_eq!(
            parse("978-606-788-262-9").unwrap().isbn13.as_deref(),
            Some("9786067882629")
        );
    }

    #[test]
    fn invalid_isbn13_checksum_rejected() {
        assert!(parse("978-0-306-40615-8").is_none());
    }

    #[test]
    fn valid_isbn10_with_x_check_accepted_and_converted() {
        let n = parse("0-19-852663-6").expect("valid isbn10");
        assert_eq!(n.isbn10.as_deref(), Some("0198526636"));
        assert_eq!(n.isbn13.as_deref(), Some("9780198526636"));
    }

    #[test]
    fn valid_isbn10_with_x_final_digit() {
        // 080442957X is a classic valid ISBN-10 ending in X.
        assert!(parse("0-8044-2957-X").is_some());
    }

    #[test]
    fn invalid_isbn10_rejected() {
        assert!(parse("0-19-852663-7").is_none());
    }

    #[test]
    fn garbage_rejected() {
        assert!(parse("").is_none());
        assert!(parse("hello").is_none());
        assert!(parse("12345").is_none());
        assert!(parse("978030640615").is_none());
        assert!(parse("97803064061570").is_none());
    }

    #[test]
    fn isbn13_conversion_roundtrip() {
        let n = parse("978-0-306-40615-7").unwrap();
        assert_eq!(n.isbn10, Some("0306406152".to_string()));
        assert_eq!(
            isbn10_to_isbn13("0306406152").as_deref(),
            Some("9780306406157")
        );
    }
}

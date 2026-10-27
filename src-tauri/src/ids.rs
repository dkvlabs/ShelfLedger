/// Stable, portable identifiers (docs/database.md): UUID v4.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_a_uuid() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert!(uuid::Uuid::parse_str(&id).is_ok());
        assert_ne!(new_id(), id);
    }

    #[test]
    fn now_iso_is_rfc3339_utc() {
        let ts = now_iso();
        assert!(ts.ends_with('Z'));
        assert!(chrono::DateTime::parse_from_rfc3339(&ts).is_ok());
    }
}

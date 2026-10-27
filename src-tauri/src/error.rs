use serde::Serialize;

/// Application error surfaced to the frontend as `{ kind, message }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("data error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database",
            AppError::Migration(_) => "database",
            AppError::Json(_) => "validation",
            AppError::Validation(_) => "validation",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::Network(_) => "network",
            AppError::Io(_) => "io",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

fn sqlite_code(err: &sqlx::Error) -> Option<String> {
    match err {
        sqlx::Error::Database(db) => db.code().map(|c| c.to_string()),
        _ => None,
    }
}

/// Map SQLite unique-constraint violations to a user-facing conflict.
pub fn is_unique_violation(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Database(inner) if sqlite_code(inner).as_deref() == Some("2067")
    )
}

/// True when the error is a SQLite foreign-key violation. RESTRICT is
/// enforced through triggers, so SQLite reports code 1811
/// (SQLITE_CONSTRAINT_TRIGGER) rather than 787 for those cases.
pub fn is_foreign_key_violation(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Database(inner)
            if matches!(sqlite_code(inner).as_deref(), Some("787") | Some("1811"))
    )
}

pub fn is_unique_violation_sqlx(err: &sqlx::Error) -> bool {
    sqlite_code(err).as_deref() == Some("2067")
}

pub fn is_foreign_key_violation_sqlx(err: &sqlx::Error) -> bool {
    matches!(sqlite_code(err).as_deref(), Some("787") | Some("1811"))
}

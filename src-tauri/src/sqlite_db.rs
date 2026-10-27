//! SQLite-only SQLx facade. Depending on the driver directly keeps unused
//! MySQL/Postgres drivers (and their crypto dependencies) out of Cargo.lock.
pub use sqlx_core::column::Column;
pub use sqlx_core::error::Error;
pub use sqlx_core::executor::Executor;
pub use sqlx_core::migrate;
pub use sqlx_core::query::query;
pub use sqlx_core::query_as::query_as;
pub use sqlx_core::query_builder::QueryBuilder;
pub use sqlx_core::query_scalar::query_scalar;
pub use sqlx_core::row::Row;
pub use sqlx_core::transaction::Transaction;
pub use sqlx_core::type_info::TypeInfo;
pub use sqlx_core::value::ValueRef;
pub use sqlx_sqlite::{self as sqlite, Sqlite, SqliteConnection, SqlitePool};

pub type DbType = sqlx::Postgres;
pub type ConnectionType = sqlx::postgres::PgConnection;
pub type PoolOptions = sqlx::postgres::PgPoolOptions;
pub type PoolType = sqlx::Pool<DbType>;
pub type DbErrType = sqlx::Error;

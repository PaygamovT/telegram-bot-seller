use crate::shared::error::{AppError, AppResult};
use deadpool_sqlite::{Config, Runtime};
use tracing::{debug, error, info};

pub type DbPool = deadpool_sqlite::Pool;

pub async fn init(db_path: &str) -> AppResult<DbPool> {
    info!("[DB.init] Initializing SQLite at {db_path}");

    // Create parent directories if they don't exist
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if !parent.exists() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                error!("[DB.init] Failed to create database directory: {err}");
                return Err(AppError::Io(err));
            }
        }
    }

    let cfg = Config::new(db_path);
    let pool = cfg.create_pool(Runtime::Tokio1)
        .map_err(|e| {
            let err_msg = format!("Failed to create DB pool: {e}");
            error!("[DB.init] {err_msg}");
            AppError::Config(err_msg)
        })?;

    debug!("[DB.init] Pool created, verifying connectivity...");

    // Verify connectivity with a test query and configure pragmas
    let conn = pool.get().await.map_err(|err| {
        error!("[DB.init] Failed to get connection from pool: {err}");
        AppError::Pool(err)
    })?;

    let test_res = conn.interact(|conn| {
        conn.execute("PRAGMA journal_mode=WAL;", [])?;
        conn.execute("PRAGMA foreign_keys=ON;", [])?;
        let mut stmt = conn.prepare("SELECT 1")?;
        let mut rows = stmt.query([])?;
        if rows.next()?.is_some() {
            Ok(())
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    })
    .await
    .map_err(|e| {
        error!("[DB.init] Failed to execute test query: {e}");
        AppError::Database(rusqlite::Error::SystemError(-1, e.to_string()))
    });

    match test_res {
        Ok(inner_res) => {
            inner_res.map_err(|err| {
                error!("[DB.init] Failed to verify connectivity: {err}");
                AppError::Database(err)
            })?;
        }
        Err(err) => return Err(err),
    }

    info!("[DB.init] Database connection pool ready");
    Ok(pool)
}

pub async fn run_migrations(pool: &DbPool) -> AppResult<()> {
    info!("[DB.run_migrations] Running database migrations...");
    debug!("[DB.run_migrations] Executing migration: 001_init.sql");

    let conn = pool.get().await.map_err(|err| {
        error!("[DB.run_migrations] Failed to get connection from pool: {err}");
        AppError::Pool(err)
    })?;

    let migration_sql = include_str!("../migrations/001_init.sql");

    let migration_res = conn.interact(move |conn| {
        // Execute batch migrations
        conn.execute_batch(migration_sql)
    })
    .await
    .map_err(|e| {
        error!("[DB.run_migrations] Migration interact error: {e}");
        AppError::Database(rusqlite::Error::SystemError(-1, e.to_string()))
    });

    match migration_res {
        Ok(inner_res) => {
            inner_res.map_err(|err| {
                error!("[DB.run_migrations] Migration SQL error: {err}");
                AppError::Database(err)
            })?;
        }
        Err(err) => return Err(err),
    }

    info!("[DB.run_migrations] Migrations completed successfully");
    Ok(())
}

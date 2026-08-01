//! データベースインフラマネージャ。
//! 接続プール、コネクションカスタマイズ（WAL/タイムアウト/外部キー）を提供する。

use crate::errors::{AppError, Result};
use r2d2::{CustomizeConnection, Pool};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

/// 接続取得時に自動実行するPRAGMAカスタマイザー
#[derive(Debug)]
pub struct SqliteConnectionCustomizer;

impl CustomizeConnection<Connection, rusqlite::Error> for SqliteConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> std::result::Result<(), rusqlite::Error> {
        // WALモード（並行読書）、ビジータイムアウト（書き込み衝突防止）、外部キー制約、同期設定を適用
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )?;
        Ok(())
    }
}

pub struct DbManager {
    pool: DbPool,
}

impl DbManager {
    /// 新規データベースマネージャをプールと共に初期化する。
    /// DBの親フォルダが存在しない場合は自動生成する。
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path = db_path.as_ref();

        // 親ディレクトリの自動作成（OSごとのディレクトリ存在エラーを防止）
        if let Some(parent) = path
            .parent()
            .filter(|p| !p.exists() && !p.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::PathError(format!("Failed to create DB directory: {}", e))
            })?;
        }

        let manager = SqliteConnectionManager::file(path);

        let pool = Pool::builder()
            .connection_customizer(Box::new(SqliteConnectionCustomizer))
            .build(manager)?;

        {
            let conn = pool.get().map_err(AppError::from)?;
            init_db(&conn).map_err(|e| AppError::InternalError(e.to_string()))?;
        }

        Ok(Self { pool })
    }

    /// プールからスレッドセーフなコネクションを取得
    pub fn get_connection(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(AppError::from)
    }

    /// デッドロックを防ぐため、IMMEDIATEモードでトランザクションを実行する（書き込み用）
    pub fn execute_transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R>,
    {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// 大量インポート（リビルド）用の外部キー制約一時無効化トランザクション。
    /// バッチ終了後に整合性（PRAGMA foreign_key_check）をチェックし、問題があればエラーを返す。
    pub fn execute_batch_rebuild<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R>,
    {
        let mut conn = self.get_connection()?;
        // このコネクションのみ外部キーを一時オフにする
        conn.execute("PRAGMA foreign_keys = OFF;", [])?;

        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = f(&tx)?;
        tx.commit()?;

        // 整合性の再チェックと外部キー制約の復旧
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // PRAGMA foreign_key_check は行がある場合結果が返る。
        // rusqliteで結果が存在するかを確認する。
        let mut stmt = conn.prepare("PRAGMA foreign_key_check;")?;
        let has_errors = stmt.exists([])?;
        if has_errors {
            return Err(AppError::InternalError(
                "Foreign key violation detected after rebuild batch".into(),
            ));
        }

        Ok(result)
    }
}

fn init_db(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            due_date TEXT,
            due_reminder BOOLEAN NOT NULL,
            parent_id TEXT,
            chain_id TEXT,
            chain_order INTEGER,
            recurrence_rule TEXT,
            recurrence_interval INTEGER,
            recurrence_days TEXT,
            recurrence_dom INTEGER,
            recurrence_limit_type TEXT,
            recurrence_limit_count INTEGER,
            recurrence_limit_date TEXT,
            exclude_dates TEXT NOT NULL,
            markdown_path TEXT NOT NULL,
            last_device_id TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tasks_due_date ON tasks(due_date);
        CREATE INDEX IF NOT EXISTS idx_tasks_parent_id ON tasks(parent_id);
        CREATE INDEX IF NOT EXISTS idx_tasks_chain_id_order ON tasks(chain_id, chain_order);

        CREATE TABLE IF NOT EXISTS triggers (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            trigger_type TEXT NOT NULL,
            trigger_time TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_triggers_task_id ON triggers(task_id);

        CREATE TABLE IF NOT EXISTS holidays (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_holidays_date ON holidays(date);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS paired_devices (
            device_id TEXT PRIMARY KEY,
            device_name TEXT NOT NULL,
            last_sync_at TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

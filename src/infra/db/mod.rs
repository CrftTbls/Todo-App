//! データベースインフラマネージャ。
//! 接続プール、コネクションカスタマイズ（WAL/タイムアウト/外部キー）を提供する。

use std::path::Path;
use r2d2::{Pool, CustomizeConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use crate::errors::{AppError, Result};

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
             PRAGMA synchronous = NORMAL;"
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
        if let Some(parent) = path.parent() {
            if !parent.exists() && parent.as_os_str() != "" {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::PathError(format!("Failed to create DB directory: {}", e)))?;
            }
        }

        let manager = SqliteConnectionManager::file(path);
        
        let pool = Pool::builder()
            .connection_customizer(Box::new(SqliteConnectionCustomizer))
            .build(manager)?;
            
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

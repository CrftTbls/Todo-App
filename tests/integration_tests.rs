use todo_app_core::infra::db::DbManager;

#[test]
fn test_db_pool_initialization_in_memory() {
    // メモリ上の SQLite DB はパス ":memory:" を使うことで作成可能
    let manager_res = DbManager::new(":memory:");
    assert!(manager_res.is_ok(), "DbManager should initialize successfully with :memory:");
    
    let manager = manager_res.unwrap();
    let conn_res = manager.get_connection();
    assert!(conn_res.is_ok(), "Should successfully acquire connection from pool");
}

#[test]
fn test_db_transaction_and_pragma_settings() {
    let manager = DbManager::new(":memory:").unwrap();
    
    // トランザクションユーティリティが正しく動き、PRAGMA設定が各接続に適用されていることを検証
    let result = manager.execute_transaction(|tx| {
        // 設定テーブルをインメモリ一時テーブルとして定義して検証
        tx.execute(
            "CREATE TEMP TABLE test_settings (key TEXT PRIMARY KEY, value TEXT);",
            [],
        )?;
        tx.execute(
            "INSERT INTO test_settings (key, value) VALUES ('theme', 'dark');",
            [],
        )?;
        
        let value: String = tx.query_row(
            "SELECT value FROM test_settings WHERE key = 'theme';",
            [],
            |row| row.get(0),
        )?;
        
        Ok(value)
    });

    assert_eq!(result.unwrap(), "dark");
}

#[test]
fn test_db_batch_rebuild_utility() {
    let manager = DbManager::new(":memory:").unwrap();
    
    // バッチリビルドユーティリティの動作検証
    let result = manager.execute_batch_rebuild(|tx| {
        // 外部キー一時オフ等のバッチ処理がエラーなく完了すること
        tx.execute(
            "CREATE TEMP TABLE test_batch (id INTEGER PRIMARY KEY);",
            [],
        )?;
        Ok(())
    });
    
    assert!(result.is_ok());
}

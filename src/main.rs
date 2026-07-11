//! デスクトップ用実行ファイルエントリーポイント。
//! sysdirsを用いてプラットフォームに適したパスを解決し、UIとバックエンドを結合する。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::path::PathBuf;
use crossbeam_channel::{unbounded, Receiver, Sender};
use slint::ComponentHandle;

mod errors;
mod infra;
mod message;

use errors::Result;
use infra::db::DbManager;
use message::{BackendMessage, BackgroundEvent, LogicEvent, UiCommand};

slint::include_modules!();

/// バックグラウンドから送られてきたUI更新イベントを適用する
fn handle_logic_event(ui: &AppWindow, event: LogicEvent) {
    match event {
        LogicEvent::Pong => {
            println!("UI received Pong");
            ui.set_status_text("Received Pong from Backend".into());
            ui.set_counter(ui.get_counter() + 1);
        }
        LogicEvent::SettingsLoaded(settings) => {
            println!("UI received settings: {:?}", settings);
            ui.set_status_text(format!("Settings loaded count: {}", settings.len()).into());
        }
        LogicEvent::DatabaseRebuilt => {
            println!("UI received DatabaseRebuilt signal");
            ui.set_status_text("Database rebuilt successfully".into());
        }
        LogicEvent::ErrorOccurred(err) => {
            eprintln!("Error in backend: {}", err);
            ui.set_status_text(format!("Error: {}", err).into());
        }
    }
}

/// UIのイベントループに対し、安全に更新クロージャを投射する（パニック耐性・スレッド安全）
fn send_to_ui(ui_weak: &slint::Weak<AppWindow>, event: LogicEvent) -> crate::errors::Result<()> {
    ui_weak
        .upgrade_in_event_loop(move |ui| {
            handle_logic_event(&ui, event);
        })
        .map_err(|e| crate::errors::AppError::UiCommunicationError(e.to_string()))
}

/// メインロジックスレッド。すべてのメッセージをシリアル化して処理する。
fn start_logic_thread(
    rx: Receiver<BackendMessage>,
    db: Arc<DbManager>,
    ui_weak: slint::Weak<AppWindow>,
) {
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let ui_weak_clone = ui_weak.clone();
            let db_clone = db.clone();

            match msg {
                BackendMessage::Ui(cmd) => {
                    let result = match cmd {
                        UiCommand::Ping => {
                            send_to_ui(&ui_weak_clone, LogicEvent::Pong)
                        }
                        UiCommand::GetSettings => {
                            // 設定テーブル未存在時はDBエラーとなるが、パニックを起こさず安全にハンドルしてUIに返す
                            let db_res = db_clone.get_connection().and_then(|conn| {
                                let mut stmt = conn.prepare("SELECT key, value FROM settings;")?;
                                let rows = stmt.query_map([], |row| {
                                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                                })?;
                                let mut settings = std::collections::HashMap::new();
                                for item in rows {
                                    if let Ok((k, v)) = item {
                                        settings.insert(k, v);
                                    }
                                }
                                Ok(settings)
                            });

                            match db_res {
                                Ok(settings) => send_to_ui(&ui_weak_clone, LogicEvent::SettingsLoaded(settings)),
                                Err(e) => send_to_ui(&ui_weak_clone, LogicEvent::ErrorOccurred(format!("DB Query failed (Normal if table not created yet): {:?}", e))),
                            }
                        }
                        UiCommand::UpdateSetting { key, value } => {
                            // データの競合・ON DELETE CASCADEでの子レコード誤消去を防止するため、UPSERT(ON CONFLICT DO UPDATE)を使用
                            let db_res = db_clone.execute_transaction(move |tx| {
                                tx.execute(
                                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                                     ON CONFLICT(key) DO UPDATE SET value=excluded.value;",
                                    (key, value),
                                )?;
                                Ok(())
                            });
                            if let Err(e) = db_res {
                                let _ = send_to_ui(&ui_weak_clone, LogicEvent::ErrorOccurred(format!("Setting Update Failed: {:?}", e)));
                            }
                            Ok(())
                        }
                        UiCommand::RebuildDatabase => {
                            // 将来的にローカルのMarkdown/JSONを再スキャンしてキャッシュDBを完全再構築する処理を実行
                            // execute_batch_rebuild を使用して、外部キー制約を一時オフにしてインポートのデッドロック・順序制約を回避する
                            let db_res = db_clone.execute_batch_rebuild(|_tx| {
                                // ここにMarkdownファイル再スキャン・パース・DB上書きUPSERTのループを記述する
                                Ok(())
                            });
                            match db_res {
                                Ok(_) => send_to_ui(&ui_weak_clone, LogicEvent::DatabaseRebuilt),
                                Err(e) => send_to_ui(&ui_weak_clone, LogicEvent::ErrorOccurred(format!("Database Rebuild Failed: {:?}", e))),
                            }
                        }
                    };

                    if let Err(e) = result {
                        eprintln!("Failed to send event to UI event loop: {:?}", e);
                    }
                }
                BackendMessage::Background(event) => {
                    match event {
                        BackgroundEvent::FileChanged { path } => {
                            println!("Backend: File change detected at {}", path);
                        }
                        BackgroundEvent::SchedulerTick => {
                            println!("Backend: Scheduler tick execution");
                        }
                        BackgroundEvent::SyncCompleted { device_id } => {
                            println!("Backend: Sync completed: {:?}", device_id);
                        }
                    }
                }
            }
        }
    });
}

/// バックグラウンドスケジューラスレッド（1分周期でメインロジックへTickを伝送）
fn start_scheduler_thread(tx: Sender<BackendMessage>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if tx.send(BackendMessage::Background(BackgroundEvent::SchedulerTick)).is_err() {
                break; // 送信エラー時はアプリケーション終了とみなしスレッドを破棄
            }
        }
    });
}

/// デスクトップ向けの安全なパス解決（ユーザーローカルの AppData 領域）
fn resolve_desktop_db_path() -> PathBuf {
    if let Some(mut path) = sysdirs::data_local_dir() {
        path.push("TodoApp");
        path.push("todo.db");
        path
    } else {
        // フォールバック（通常は発生しないが、万が一に備え安全なカレント相対パス）
        PathBuf::from("todo.db")
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // 1. データベースマネージャ初期化 (SQLiteプール起動)
    let db_path = resolve_desktop_db_path();
    let db = Arc::new(DbManager::new(db_path)?);

    // 2. メッセージキューの初期化
    let (tx, rx) = unbounded::<BackendMessage>();

    // 3. UIのインスタンス化
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    // 4. メインロジックおよびスケジューラの起動
    start_logic_thread(rx, db, ui_weak);
    start_scheduler_thread(tx.clone());

    // 5. UIイベントコールバックの登録 (チャネルを介した非ブロッキング送信)
    let tx_clone = tx.clone();
    ui.on_request_increase_value(move || {
        let _ = tx_clone.send(BackendMessage::Ui(UiCommand::Ping));
    });

    // 6. UIイベントループの実行
    ui.run()?;

    Ok(())
}

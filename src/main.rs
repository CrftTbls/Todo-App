//! デスクトップ用実行ファイルエントリーポイント。
//! sysdirsを用いてプラットフォームに適したパスを解決し、UIとバックエンドを結合する。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use crossbeam_channel::{Receiver, Sender, unbounded};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::Arc;

use todo_app_core::infra::db::DbManager;
use todo_app_core::message::{BackendMessage, BackgroundEvent, LogicEvent, UiCommand};

mod ui {
    #![allow(clippy::all)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::arithmetic_side_effects)]
    #![allow(clippy::let_underscore_must_use)]
    slint::include_modules!();
}
use ui::*;

fn to_slint_task(task: &todo_app_core::features::task::models::Task) -> SlintTask {
    SlintTask {
        id: task.id.clone().into(),
        title: task.title.clone().into(),
        status: task.status.as_str().into(),
        priority: task.priority.as_str().into(),
        due_date: task.due_date.clone().unwrap_or_default().into(),
        parent_id: task.parent_id.clone().unwrap_or_default().into(),
        chain_id: task.chain_id.clone().unwrap_or_default().into(),
        chain_order: task.chain_order.unwrap_or(0) as i32,
    }
}

/// バックグラウンドから送られてきたUI更新イベントを適用する
#[allow(clippy::print_stdout)]
fn handle_logic_event(ui: &AppWindow, event: LogicEvent) {
    match event {
        LogicEvent::Pong => {
            println!("UI received Pong");
        }
        LogicEvent::SettingsLoaded(settings) => {
            println!("UI received settings: {:?}", settings);
        }
        LogicEvent::DatabaseRebuilt => {
            println!("UI received DatabaseRebuilt signal");
        }
        LogicEvent::ErrorOccurred(err) => {
            eprintln!("Error in backend: {}", err);
        }
        LogicEvent::TasksLoaded(tasks) => {
            let today_slint_tasks: Vec<SlintTask> = tasks
                .iter()
                .filter(|t| t.status == todo_app_core::features::task::models::TaskStatus::Todo)
                .map(to_slint_task)
                .collect();
            let done_slint_tasks: Vec<SlintTask> = tasks
                .iter()
                .filter(|t| t.status == todo_app_core::features::task::models::TaskStatus::Done)
                .map(to_slint_task)
                .collect();

            ui.set_today_tasks(slint::ModelRc::new(slint::VecModel::from(
                today_slint_tasks,
            )));
            ui.set_done_tasks(slint::ModelRc::new(slint::VecModel::from(done_slint_tasks)));
        }
        LogicEvent::TaskCreated(_task) => {}
        LogicEvent::TaskUpdated(_task) => {}
        LogicEvent::TaskDeleted(_id) => {}
    }
}

/// UIのイベントループに対し、安全に更新クロージャを投射する（パニック耐性・スレッド安全）
fn send_to_ui(
    ui_weak: &slint::Weak<AppWindow>,
    event: LogicEvent,
) -> todo_app_core::errors::Result<()> {
    ui_weak
        .upgrade_in_event_loop(move |ui| {
            handle_logic_event(&ui, event);
        })
        .map_err(|e| todo_app_core::errors::AppError::UiCommunicationError(e.to_string()))
}

/// メインロジックスレッド。すべてのメッセージをシリアル化して処理する。
#[allow(clippy::print_stdout)]
fn start_logic_thread(
    rx: Receiver<BackendMessage>,
    db: Arc<DbManager>,
    ui_weak: slint::Weak<AppWindow>,
    tx: Sender<BackendMessage>,
) {
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let ui_weak_clone = ui_weak.clone();
            let db_clone = db.clone();
            let tx_clone = tx.clone();

            match msg {
                BackendMessage::Ui(cmd) => {
                    let result = match *cmd {
                        UiCommand::Ping => send_to_ui(&ui_weak_clone, LogicEvent::Pong),
                        UiCommand::GetSettings => {
                            let db_res = db_clone.get_connection().and_then(|conn| {
                                let mut stmt = conn.prepare("SELECT key, value FROM settings;")?;
                                let rows = stmt.query_map([], |row| {
                                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                                })?;
                                let mut settings = std::collections::HashMap::new();
                                for (k, v) in rows.flatten() {
                                    settings.insert(k, v);
                                }
                                Ok(settings)
                            });

                            match db_res {
                                Ok(settings) => {
                                    send_to_ui(&ui_weak_clone, LogicEvent::SettingsLoaded(settings))
                                }
                                Err(e) => send_to_ui(
                                    &ui_weak_clone,
                                    LogicEvent::ErrorOccurred(format!("DB Query failed: {:?}", e)),
                                ),
                            }
                        }
                        UiCommand::UpdateSetting { key, value } => {
                            let db_res = db_clone.execute_transaction(move |tx| {
                                tx.execute(
                                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                                     ON CONFLICT(key) DO UPDATE SET value=excluded.value;",
                                    (key, value),
                                )?;
                                Ok(())
                            });
                            if let Err(e) = db_res {
                                drop(send_to_ui(
                                    &ui_weak_clone,
                                    LogicEvent::ErrorOccurred(format!(
                                        "Setting Update Failed: {:?}",
                                        e
                                    )),
                                ));
                            }
                            Ok(())
                        }
                        UiCommand::RebuildDatabase => {
                            let db_res = db_clone.execute_batch_rebuild(|_tx| Ok(()));
                            match db_res {
                                Ok(_) => send_to_ui(&ui_weak_clone, LogicEvent::DatabaseRebuilt),
                                Err(e) => send_to_ui(
                                    &ui_weak_clone,
                                    LogicEvent::ErrorOccurred(format!(
                                        "Database Rebuild Failed: {:?}",
                                        e
                                    )),
                                ),
                            }
                        }
                        UiCommand::GetTasks => {
                            let db_res = db_clone.get_connection().and_then(|conn| {
                                let mut stmt = conn.prepare("SELECT * FROM tasks;")?;
                                let rows = stmt.query_map(
                                    [],
                                    todo_app_core::features::task::db::row_to_task,
                                )?;
                                let mut tasks = Vec::new();
                                for r in rows {
                                    tasks.push(r?);
                                }
                                Ok(tasks)
                            });
                            match db_res {
                                Ok(tasks) => {
                                    send_to_ui(&ui_weak_clone, LogicEvent::TasksLoaded(tasks))
                                }
                                Err(e) => send_to_ui(
                                    &ui_weak_clone,
                                    LogicEvent::ErrorOccurred(format!("GetTasks failed: {:?}", e)),
                                ),
                            }
                        }
                        UiCommand::CreateTask {
                            title,
                            parent_id,
                            chain_id,
                            chain_order,
                        } => {
                            let db_res = db_clone.execute_transaction(move |tx| {
                                let id = uuid::Uuid::new_v4().to_string();
                                let now = chrono::Local::now().to_rfc3339();
                                let task = todo_app_core::features::task::models::Task {
                                    id: id.clone(),
                                    title,
                                    status: todo_app_core::features::task::models::TaskStatus::Todo,
                                    priority: todo_app_core::features::task::models::TaskPriority::None,
                                    created_at: now.clone(),
                                    updated_at: now.clone(),
                                    completed_at: None,
                                    due_date: None,
                                    due_reminder: false,
                                    parent_id: parent_id.clone(),
                                    chain_id,
                                    chain_order,
                                    recurrence_rule: None,
                                    recurrence_interval: None,
                                    recurrence_days: None,
                                    recurrence_dom: None,
                                    recurrence_limit_type: None,
                                    recurrence_limit_count: None,
                                    recurrence_limit_date: None,
                                    exclude_dates: "[]".to_string(),
                                    markdown_path: "".to_string(),
                                    last_device_id: "".to_string(),
                                };
                                todo_app_core::features::task::db::insert_task(tx, &task)?;

                                if let Some(ref p_id) = parent_id {
                                    todo_app_core::features::task::rules::check_and_update_parent_status(tx, p_id)?;
                                }

                                Ok(task)
                            });
                            match db_res {
                                Ok(task) => {
                                    drop(send_to_ui(&ui_weak_clone, LogicEvent::TaskCreated(task)));
                                    drop(
                                        tx_clone.send(BackendMessage::Ui(Box::new(
                                            UiCommand::GetTasks,
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    drop(send_to_ui(
                                        &ui_weak_clone,
                                        LogicEvent::ErrorOccurred(format!(
                                            "CreateTask failed: {:?}",
                                            e
                                        )),
                                    ));
                                }
                            }
                            Ok(())
                        }
                        UiCommand::UpdateTask(task) => {
                            let db_res = db_clone.execute_transaction(move |tx| {
                                todo_app_core::features::task::db::update_task(tx, &task)?;
                                Ok(task)
                            });
                            match db_res {
                                Ok(task) => {
                                    drop(send_to_ui(
                                        &ui_weak_clone,
                                        LogicEvent::TaskUpdated(*task),
                                    ));
                                    drop(
                                        tx_clone.send(BackendMessage::Ui(Box::new(
                                            UiCommand::GetTasks,
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    drop(send_to_ui(
                                        &ui_weak_clone,
                                        LogicEvent::ErrorOccurred(format!(
                                            "UpdateTask failed: {:?}",
                                            e
                                        )),
                                    ));
                                }
                            }
                            Ok(())
                        }
                        UiCommand::UpdateTaskStatus { id, status } => {
                            let db_res = db_clone.execute_transaction(move |tx| {
                                let mut task = match todo_app_core::features::task::db::get_task(tx, &id)? {
                                    Some(t) => t,
                                    None => return Err(todo_app_core::errors::AppError::InternalError("Task not found".into())),
                                };
                                let new_status = match status.as_str() {
                                    "todo" => todo_app_core::features::task::models::TaskStatus::Todo,
                                    "done" => todo_app_core::features::task::models::TaskStatus::Done,
                                    "canceled" => todo_app_core::features::task::models::TaskStatus::Canceled,
                                    _ => task.status.clone(),
                                };
                                task.status = new_status;
                                task.updated_at = chrono::Local::now().to_rfc3339();
                                if task.status == todo_app_core::features::task::models::TaskStatus::Done {
                                    task.completed_at = Some(chrono::Local::now().to_rfc3339());
                                } else {
                                    task.completed_at = None;
                                }

                                todo_app_core::features::task::db::update_task(tx, &task)?;

                                if task.status == todo_app_core::features::task::models::TaskStatus::Done {
                                    todo_app_core::features::task::rules::update_children_status_on_parent_done(tx, &task.id)?;
                                }

                                if let Some(ref p_id) = task.parent_id {
                                    todo_app_core::features::task::rules::check_and_update_parent_status(tx, p_id)?;
                                }

                                Ok(task)
                            });
                            match db_res {
                                Ok(task) => {
                                    drop(send_to_ui(&ui_weak_clone, LogicEvent::TaskUpdated(task)));
                                    drop(
                                        tx_clone.send(BackendMessage::Ui(Box::new(
                                            UiCommand::GetTasks,
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    drop(send_to_ui(
                                        &ui_weak_clone,
                                        LogicEvent::ErrorOccurred(format!(
                                            "UpdateTaskStatus failed: {:?}",
                                            e
                                        )),
                                    ));
                                }
                            }
                            Ok(())
                        }
                        UiCommand::DeleteTask { id } => {
                            let db_res = db_clone.execute_transaction(move |tx| {
                                todo_app_core::features::task::db::delete_task(tx, &id)?;
                                Ok(id)
                            });
                            match db_res {
                                Ok(id) => {
                                    drop(send_to_ui(&ui_weak_clone, LogicEvent::TaskDeleted(id)));
                                    drop(
                                        tx_clone.send(BackendMessage::Ui(Box::new(
                                            UiCommand::GetTasks,
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    drop(send_to_ui(
                                        &ui_weak_clone,
                                        LogicEvent::ErrorOccurred(format!(
                                            "DeleteTask failed: {:?}",
                                            e
                                        )),
                                    ));
                                }
                            }
                            Ok(())
                        }
                    };

                    if let Err(e) = result {
                        eprintln!("Failed to send event to UI event loop: {:?}", e);
                    }
                }
                BackendMessage::Background(event) => match event {
                    BackgroundEvent::FileChanged { path } => {
                        println!("Backend: File change detected at {}", path);
                    }
                    BackgroundEvent::SchedulerTick => {
                        println!("Backend: Scheduler tick execution");
                    }
                    BackgroundEvent::SyncCompleted { device_id } => {
                        println!("Backend: Sync completed: {:?}", device_id);
                    }
                },
            }
        }
    });
}

/// バックグラウンドスケジューラスレッド（1分周期でメインロジックへTickを伝送）
fn start_scheduler_thread(tx: Sender<BackendMessage>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if tx
                .send(BackendMessage::Background(BackgroundEvent::SchedulerTick))
                .is_err()
            {
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
    start_logic_thread(rx, db, ui_weak, tx.clone());
    start_scheduler_thread(tx.clone());

    // 5. UIイベントコールバックの登録
    let tx_clone = tx.clone();
    ui.on_load_tasks(move || {
        drop(tx_clone.send(BackendMessage::Ui(Box::new(UiCommand::GetTasks))));
    });

    let tx_clone = tx.clone();
    ui.on_create_task(move |text| {
        let title = text.to_string();
        if !title.trim().is_empty() {
            drop(
                tx_clone.send(BackendMessage::Ui(Box::new(UiCommand::CreateTask {
                    title,
                    parent_id: None,
                    chain_id: None,
                    chain_order: None,
                }))),
            );
        }
    });

    let tx_clone = tx.clone();
    ui.on_update_task_status(move |id, status| {
        drop(
            tx_clone.send(BackendMessage::Ui(Box::new(UiCommand::UpdateTaskStatus {
                id: id.into(),
                status: status.into(),
            }))),
        );
    });

    let tx_clone = tx.clone();
    ui.on_delete_task(move |id| {
        drop(
            tx_clone.send(BackendMessage::Ui(Box::new(UiCommand::DeleteTask {
                id: id.into(),
            }))),
        );
    });

    // 起動時に全タスク取得
    drop(tx.send(BackendMessage::Ui(Box::new(UiCommand::GetTasks))));

    // 6. UIイベントループの実行
    ui.run()?;

    Ok(())
}

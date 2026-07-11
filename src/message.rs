//! スレッド間通信用メッセージの定義。
//! UIコマンドとバックグラウンドイベントを明確に分類・統合する。

use std::collections::HashMap;

/// UIスレッドからロジックへ対する操作要求
#[derive(Debug)]
pub enum UiCommand {
    Ping,
    GetSettings,
    UpdateSetting { key: String, value: String },
    /// MarkdownディレクトリをフルスキャンしてキャッシュDBを完全に再構成（リビルド）する要求
    RebuildDatabase,
}

/// バックグラウンド監視プロセスからメインロジックへの非同期イベント通知
#[derive(Debug)]
pub enum BackgroundEvent {
    FileChanged { path: String },
    SchedulerTick,
    SyncCompleted { device_id: Option<String> },
}

/// メインロジックが処理する共通メッセージ表現
#[derive(Debug)]
pub enum BackendMessage {
    Ui(UiCommand),
    Background(BackgroundEvent),
}

/// メインロジックからUIスレッド（イベントループ）へのデータフィードバック
#[derive(Debug, Clone)]
pub enum LogicEvent {
    Pong,
    SettingsLoaded(HashMap<String, String>),
    DatabaseRebuilt,
    ErrorOccurred(String),
}

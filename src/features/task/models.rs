use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Todo,
    Done,
    Canceled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::Done => "done",
            TaskStatus::Canceled => "canceled",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "todo" => Ok(TaskStatus::Todo),
            "done" => Ok(TaskStatus::Done),
            "canceled" | "cancelled" => Ok(TaskStatus::Canceled),
            _ => Err(format!("invalid task status: {}", s)),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        TaskStatus::from_str(s)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
    None,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskPriority::High => "high",
            TaskPriority::Medium => "medium",
            TaskPriority::Low => "low",
            TaskPriority::None => "none",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "high" => Ok(TaskPriority::High),
            "medium" => Ok(TaskPriority::Medium),
            "low" => Ok(TaskPriority::Low),
            "none" => Ok(TaskPriority::None),
            _ => Err(format!("invalid task priority: {}", s)),
        }
    }
}

impl std::str::FromStr for TaskPriority {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        TaskPriority::from_str(s)
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub due_date: Option<String>,
    pub due_reminder: bool,
    pub parent_id: Option<String>,
    pub chain_id: Option<String>,
    pub chain_order: Option<i64>,
    pub recurrence_rule: Option<String>,
    pub recurrence_interval: Option<i64>,
    pub recurrence_days: Option<String>,
    pub recurrence_dom: Option<i64>,
    pub recurrence_limit_type: Option<String>,
    pub recurrence_limit_count: Option<i64>,
    pub recurrence_limit_date: Option<String>,
    pub exclude_dates: String,
    pub markdown_path: String,
    pub last_device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: String,
    pub task_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub time_trigger: Option<String>,
    pub loc_name: Option<String>,
    pub loc_lat: Option<f64>,
    pub loc_lng: Option<f64>,
    pub loc_radius: Option<f64>,
    pub and_condition: bool,
}

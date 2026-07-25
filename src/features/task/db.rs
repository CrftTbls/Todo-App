use crate::errors::AppError;
use super::models::{Task, TaskStatus, TaskPriority};
use rusqlite::{params, Row, OptionalExtension, Transaction};

pub fn row_to_task(row: &Row) -> std::result::Result<Task, rusqlite::Error> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        status: {
            let s: String = row.get(2)?;
            match s.as_str() {
                "todo" => TaskStatus::Todo,
                "done" => TaskStatus::Done,
                "canceled" => TaskStatus::Canceled,
                _ => TaskStatus::Todo,
            }
        },
        priority: {
            let s: String = row.get(3)?;
            match s.as_str() {
                "high" => TaskPriority::High,
                "medium" => TaskPriority::Medium,
                "low" => TaskPriority::Low,
                "none" => TaskPriority::None,
                _ => TaskPriority::None,
            }
        },
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        completed_at: row.get(6)?,
        due_date: row.get(7)?,
        due_reminder: row.get(8)?,
        parent_id: row.get(9)?,
        chain_id: row.get(10)?,
        chain_order: row.get(11)?,
        recurrence_rule: row.get(12)?,
        recurrence_interval: row.get(13)?,
        recurrence_days: row.get(14)?,
        recurrence_dom: row.get(15)?,
        recurrence_limit_type: row.get(16)?,
        recurrence_limit_count: row.get(17)?,
        recurrence_limit_date: row.get(18)?,
        exclude_dates: row.get(19)?,
        markdown_path: row.get(20)?,
        last_device_id: row.get(21)?,
    })
}

pub fn insert_task(tx: &Transaction, task: &Task) -> Result<(), AppError> {
    tx.execute(
        "INSERT INTO tasks (
            id, title, status, priority, created_at, updated_at, completed_at, 
            due_date, due_reminder, parent_id, chain_id, chain_order, 
            recurrence_rule, recurrence_interval, recurrence_days, recurrence_dom, 
            recurrence_limit_type, recurrence_limit_count, recurrence_limit_date, 
            exclude_dates, markdown_path, last_device_id
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 
            ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
        )",
        params![
            task.id, task.title, task.status.as_str(), task.priority.as_str(),
            task.created_at, task.updated_at, task.completed_at, task.due_date,
            task.due_reminder, task.parent_id, task.chain_id, task.chain_order,
            task.recurrence_rule, task.recurrence_interval, task.recurrence_days,
            task.recurrence_dom, task.recurrence_limit_type, task.recurrence_limit_count,
            task.recurrence_limit_date, task.exclude_dates, task.markdown_path,
            task.last_device_id
        ]
    )?;
    Ok(())
}

pub fn update_task(tx: &Transaction, task: &Task) -> Result<(), AppError> {
    tx.execute(
        "UPDATE tasks SET 
            title = ?2, status = ?3, priority = ?4, updated_at = ?5, 
            completed_at = ?6, due_date = ?7, due_reminder = ?8, parent_id = ?9, 
            chain_id = ?10, chain_order = ?11, recurrence_rule = ?12, 
            recurrence_interval = ?13, recurrence_days = ?14, recurrence_dom = ?15, 
            recurrence_limit_type = ?16, recurrence_limit_count = ?17, 
            recurrence_limit_date = ?18, exclude_dates = ?19, markdown_path = ?20, 
            last_device_id = ?21
        WHERE id = ?1",
        params![
            task.id, task.title, task.status.as_str(), task.priority.as_str(),
            task.updated_at, task.completed_at, task.due_date, task.due_reminder, 
            task.parent_id, task.chain_id, task.chain_order, task.recurrence_rule, 
            task.recurrence_interval, task.recurrence_days, task.recurrence_dom, 
            task.recurrence_limit_type, task.recurrence_limit_count, 
            task.recurrence_limit_date, task.exclude_dates, task.markdown_path, 
            task.last_device_id
        ]
    )?;
    Ok(())
}

pub fn get_task(tx: &Transaction, id: &str) -> Result<Option<Task>, AppError> {
    let mut stmt = tx.prepare("SELECT * FROM tasks WHERE id = ?1")?;
    let task = stmt.query_row(params![id], row_to_task).optional()?;
    Ok(task)
}

pub fn delete_task(tx: &Transaction, id: &str) -> Result<(), AppError> {
    tx.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_children(tx: &Transaction, parent_id: &str) -> Result<Vec<Task>, AppError> {
    let mut stmt = tx.prepare("SELECT * FROM tasks WHERE parent_id = ?1")?;
    let rows = stmt.query_map(params![parent_id], row_to_task)?;
    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row?);
    }
    Ok(tasks)
}

pub fn get_chain_tasks(tx: &Transaction, chain_id: &str) -> Result<Vec<Task>, AppError> {
    let mut stmt = tx.prepare("SELECT * FROM tasks WHERE chain_id = ?1 ORDER BY chain_order ASC")?;
    let rows = stmt.query_map(params![chain_id], row_to_task)?;
    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row?);
    }
    Ok(tasks)
}

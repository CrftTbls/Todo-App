use super::db::{get_chain_tasks, get_children, get_task, update_task};
use super::models::{Task, TaskStatus};
use crate::errors::AppError;
use rusqlite::Transaction;

pub fn check_and_update_parent_status(tx: &Transaction, parent_id: &str) -> Result<(), AppError> {
    let mut parent = match get_task(tx, parent_id)? {
        Some(p) => p,
        None => return Ok(()),
    };

    let children = get_children(tx, parent_id)?;
    if children.is_empty() {
        return Ok(());
    }

    let all_done = children.iter().all(|c| c.status == TaskStatus::Done);
    let mut changed = false;

    if all_done {
        if parent.status != TaskStatus::Done {
            parent.status = TaskStatus::Done;
            parent.completed_at = Some(chrono::Local::now().to_rfc3339());
            parent.updated_at = chrono::Local::now().to_rfc3339();
            changed = true;
        }
    } else {
        if parent.status == TaskStatus::Done {
            parent.status = TaskStatus::Todo;
            parent.completed_at = None;
            parent.updated_at = chrono::Local::now().to_rfc3339();
            changed = true;
        }
    }

    if changed {
        update_task(tx, &parent)?;
        if let Some(ref grand_parent_id) = parent.parent_id {
            check_and_update_parent_status(tx, grand_parent_id)?;
        }
    }
    Ok(())
}

pub fn update_children_status_on_parent_done(
    tx: &Transaction,
    parent_id: &str,
) -> Result<(), AppError> {
    let now = chrono::Local::now().to_rfc3339();
    let mut children = get_children(tx, parent_id)?;
    for child in children.iter_mut() {
        if child.status != TaskStatus::Done {
            child.status = TaskStatus::Done;
            child.completed_at = Some(now.clone());
            child.updated_at = now.clone();
            update_task(tx, child)?;
            update_children_status_on_parent_done(tx, &child.id)?;
        }
    }
    Ok(())
}

pub fn get_active_chain_task(tx: &Transaction, chain_id: &str) -> Result<Option<Task>, AppError> {
    let tasks = get_chain_tasks(tx, chain_id)?;
    for task in tasks {
        if task.status == TaskStatus::Todo {
            return Ok(Some(task));
        }
    }
    Ok(None)
}

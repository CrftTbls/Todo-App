use todo_app_core::features::task::db::{get_task, insert_task, update_task};
use todo_app_core::features::task::models::{Task, TaskPriority, TaskStatus};
use todo_app_core::features::task::rules::{
    check_and_update_parent_status, get_active_chain_task, update_children_status_on_parent_done,
};
use todo_app_core::infra::db::DbManager;

fn setup_test_db() -> DbManager {
    DbManager::new(":memory:").unwrap()
}

fn create_dummy_task(
    id: &str,
    parent_id: Option<String>,
    chain_id: Option<String>,
    chain_order: Option<i64>,
) -> Task {
    let now = chrono::Local::now().to_rfc3339();
    Task {
        id: id.to_string(),
        title: format!("Task {}", id),
        status: TaskStatus::Todo,
        priority: TaskPriority::None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        due_date: None,
        due_reminder: false,
        parent_id,
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
    }
}

#[test]
fn test_parent_child_auto_completion() {
    let manager = setup_test_db();

    let res = manager.execute_transaction(|tx| {
        let parent = create_dummy_task("parent", None, None, None);
        let mut child1 = create_dummy_task("child1", Some("parent".to_string()), None, None);
        let mut child2 = create_dummy_task("child2", Some("parent".to_string()), None, None);

        insert_task(tx, &parent)?;
        insert_task(tx, &child1)?;
        insert_task(tx, &child2)?;

        // Complete child1 -> parent shouldn't be completed
        child1.status = TaskStatus::Done;
        update_task(tx, &child1)?;
        check_and_update_parent_status(tx, "parent")?;

        let p = get_task(tx, "parent")?.unwrap();
        assert_eq!(p.status, TaskStatus::Todo);

        // Complete child2 -> parent should be auto completed
        child2.status = TaskStatus::Done;
        update_task(tx, &child2)?;
        check_and_update_parent_status(tx, "parent")?;

        let p = get_task(tx, "parent")?.unwrap();
        assert_eq!(p.status, TaskStatus::Done);

        // Revert child1 to todo -> parent should become todo
        child1.status = TaskStatus::Todo;
        update_task(tx, &child1)?;
        check_and_update_parent_status(tx, "parent")?;

        let p = get_task(tx, "parent")?.unwrap();
        assert_eq!(p.status, TaskStatus::Todo);

        Ok(())
    });
    assert!(res.is_ok());
}

#[test]
fn test_parent_done_auto_completes_children() {
    let manager = setup_test_db();
    let res = manager.execute_transaction(|tx| {
        let parent = create_dummy_task("parent", None, None, None);
        let child1 = create_dummy_task("child1", Some("parent".to_string()), None, None);
        let child2 = create_dummy_task("child2", Some("parent".to_string()), None, None);

        insert_task(tx, &parent)?;
        insert_task(tx, &child1)?;
        insert_task(tx, &child2)?;

        // Complete parent
        let mut p = get_task(tx, "parent")?.unwrap();
        p.status = TaskStatus::Done;
        update_task(tx, &p)?;

        update_children_status_on_parent_done(tx, "parent")?;

        let c1 = get_task(tx, "child1")?.unwrap();
        let c2 = get_task(tx, "child2")?.unwrap();

        assert_eq!(c1.status, TaskStatus::Done);
        assert_eq!(c2.status, TaskStatus::Done);

        Ok(())
    });
    assert!(res.is_ok());
}

#[test]
fn test_active_chain_task() {
    let manager = setup_test_db();
    let res = manager.execute_transaction(|tx| {
        let mut t1 = create_dummy_task("t1", None, Some("chain1".to_string()), Some(1));
        let mut t2 = create_dummy_task("t2", None, Some("chain1".to_string()), Some(2));
        let t3 = create_dummy_task("t3", None, Some("chain1".to_string()), Some(3));

        insert_task(tx, &t1)?;
        insert_task(tx, &t2)?;
        insert_task(tx, &t3)?;

        // Active should be t1 (order 1)
        let active = get_active_chain_task(tx, "chain1")?.unwrap();
        assert_eq!(active.id, "t1");

        // Complete t1 -> active should be t2 (order 2)
        t1.status = TaskStatus::Done;
        update_task(tx, &t1)?;

        let active = get_active_chain_task(tx, "chain1")?.unwrap();
        assert_eq!(active.id, "t2");

        // Complete t2 -> active should be t3 (order 3)
        t2.status = TaskStatus::Done;
        update_task(tx, &t2)?;

        let active = get_active_chain_task(tx, "chain1")?.unwrap();
        assert_eq!(active.id, "t3");

        Ok(())
    });
    assert!(res.is_ok());
}

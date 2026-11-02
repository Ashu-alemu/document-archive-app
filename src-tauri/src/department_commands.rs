use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::security::{actor, audit, database_connection, notify_best_effort, require, timestamp, Department, SessionState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepartmentMember {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub active: i64,
    pub role_id: String,
    pub role_name: String,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 80 && value.chars().all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn required_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 { return Err("Department names must contain 1 to 80 characters.".into()); }
    Ok(value.to_string())
}

fn description(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.chars().count() > 240 { return Err("Department descriptions must be 240 characters or fewer.".into()); }
    Ok(value.to_string())
}

fn department(connection: &Connection, department_id: &str) -> Result<Department, String> {
    if !valid_id(department_id) { return Err("Invalid department ID.".into()); }
    connection.query_row("SELECT id, name, description, icon, active, sort_order FROM departments WHERE id = ?", params![department_id], |row| Ok(Department { id: row.get(0)?, name: row.get(1)?, description: row.get(2)?, icon: row.get(3)?, active: row.get(4)?, sort_order: row.get(5)? })).map_err(|_| "Department not found.".into())
}

fn slug(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() { result.push(character); } else if !result.ends_with('-') { result.push('-'); }
    }
    result.trim_matches('-').chars().take(70).collect()
}

#[tauri::command]
pub fn list_all_departments(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<Department>, String> {
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let mut statement = connection.prepare("SELECT id, name, description, icon, active, sort_order FROM departments ORDER BY sort_order, name COLLATE NOCASE").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(Department { id: row.get(0)?, name: row.get(1)?, description: row.get(2)?, icon: row.get(3)?, active: row.get(4)?, sort_order: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_department(app: AppHandle, state: State<'_, SessionState>, token: String, name: String, department_description: String, active: bool) -> Result<Department, String> {
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let name = required_name(&name)?;
    let department_description = description(&department_description)?;
    let id = slug(&name);
    if !valid_id(&id) || id.is_empty() { return Err("Department name must contain letters or numbers.".into()); }
    let now = timestamp();
    let sort_order: i64 = connection.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM departments", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    connection.execute("INSERT INTO departments (id, name, description, icon, active, sort_order, created_at, updated_at) VALUES (?, ?, ?, '✦', ?, ?, ?, ?)", params![id, name, department_description, if active { 1 } else { 0 }, sort_order, now, now]).map_err(|error| format!("Unable to create department: {error}"))?;
    audit(&connection, Some(&current.id), "department_created", "department", Some(&id), Some(&name))?;
    notify_best_effort(&connection, Some(&current.id), "department_created", "Department created", "A new church department was created.", Some("department"), Some(&id), "info");
    department(&connection, &id)
}

#[tauri::command]
pub fn update_department(app: AppHandle, state: State<'_, SessionState>, token: String, department_id: String, name: String, department_description: String) -> Result<Department, String> {
    let mut connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let existing = department(&connection, department_id.trim())?;
    let name = required_name(&name)?;
    let department_description = description(&department_description)?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    transaction.execute("UPDATE departments SET name = ?, description = ?, updated_at = ? WHERE id = ?", params![name, department_description, timestamp(), existing.id]).map_err(|error| format!("Unable to update department: {error}"))?;
    audit(&transaction, Some(&current.id), "department_updated", "department", Some(&existing.id), Some(&format!("name={name}")))?;
    transaction.commit().map_err(|error| error.to_string())?;
    notify_best_effort(&connection, Some(&current.id), "department_updated", "Department updated", "A church department was updated.", Some("department"), Some(&existing.id), "info");
    department(&connection, &existing.id)
}

#[tauri::command]
pub fn set_department_active(app: AppHandle, state: State<'_, SessionState>, token: String, department_id: String, active: bool) -> Result<Department, String> {
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let existing = department(&connection, department_id.trim())?;
    if existing.id == "administration" && !active { return Err("The Administration department cannot be deactivated.".into()); }
    connection.execute("UPDATE departments SET active = ?, updated_at = ? WHERE id = ?", params![if active { 1 } else { 0 }, timestamp(), existing.id]).map_err(|error| format!("Unable to update department status: {error}"))?;
    let action = if active { "department_activated" } else { "department_deactivated" };
    audit(&connection, Some(&current.id), action, "department", Some(&existing.id), None)?;
    notify_best_effort(&connection, Some(&current.id), action, if active { "Department activated" } else { "Department deactivated" }, if active { "A church department is available for sign-in again." } else { "A church department was removed from new sign-in selection." }, Some("department"), Some(&existing.id), if active { "info" } else { "warning" });
    department(&connection, &existing.id)
}

#[tauri::command]
pub fn list_department_members(app: AppHandle, state: State<'_, SessionState>, token: String, department_id: String) -> Result<Vec<DepartmentMember>, String> {
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let _ = department(&connection, department_id.trim())?;
    let mut statement = connection.prepare("SELECT u.id, u.username, u.display_name, u.active, r.id, r.name FROM users u JOIN department_memberships dm ON dm.user_id = u.id JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id WHERE dm.department_id = ? AND dm.active = 1 ORDER BY u.display_name COLLATE NOCASE, u.username").map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![department_id.trim()], |row| Ok(DepartmentMember { id: row.get(0)?, username: row.get(1)?, display_name: row.get(2)?, active: row.get(3)?, role_id: row.get(4)?, role_name: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_department_membership(app: AppHandle, state: State<'_, SessionState>, token: String, department_id: String, user_id: String, active: bool) -> Result<(), String> {
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let department = department(&connection, department_id.trim())?;
    if !valid_id(user_id.trim()) { return Err("Invalid user ID.".into()); }
    let user: Option<(String, String, String, i64)> = connection.query_row("SELECT u.id, u.username, r.id, u.active FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id WHERE u.id = ?", params![user_id.trim()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional().map_err(|error| error.to_string())?;
    let Some((user_id, username, role_id, user_active)) = user else { return Err("User not found.".into()); };
    if !active && department.id == "administration" && role_id == "admin" && user_active == 1 {
        let administrators: i64 = connection.query_row("SELECT COUNT(*) FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN department_memberships dm ON dm.user_id = u.id WHERE ur.role_id = 'admin' AND u.active = 1 AND dm.department_id = 'administration' AND dm.active = 1", [], |row| row.get(0)).map_err(|error| error.to_string())?;
        if administrators <= 1 { return Err("The Administration department must retain one active administrator membership.".into()); }
    }
    let now = timestamp();
    connection.execute("INSERT INTO department_memberships (user_id, department_id, active, created_at, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(user_id, department_id) DO UPDATE SET active = excluded.active, updated_at = excluded.updated_at", params![user_id, department.id, if active { 1 } else { 0 }, now, now]).map_err(|error| format!("Unable to update department membership: {error}"))?;
    let action = if active { "department_membership_added" } else { "department_membership_removed" };
    audit(&connection, Some(&current.id), action, "department_membership", Some(&format!("{}:{}", department.id, user_id)), Some(&format!("user={username}")))?;
    notify_best_effort(&connection, Some(&current.id), action, if active { "Department member added" } else { "Department member removed" }, if active { "A user was assigned to a church department." } else { "A user was removed from a church department." }, Some("department"), Some(&department.id), "info");
    Ok(())
}

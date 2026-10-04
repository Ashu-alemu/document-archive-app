use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, State};
use crate::security::{actor, audit, require, SessionState};

#[derive(Serialize)]
pub struct ManagedValue {
    pub name: String,
}

fn validate_name(name: &str, kind: &str) -> Result<String, String> {
    let normalized = name.trim();
    if normalized.is_empty() || normalized.chars().count() > 80 {
        return Err(format!("{kind} names must contain 1 to 80 characters."));
    }
    if kind == "Tag" && normalized.contains(',') {
        return Err("Tags cannot contain commas because commas separate document tags.".into());
    }
    Ok(normalized.to_string())
}

fn list_values(connection: &Connection, table: &str) -> Result<Vec<ManagedValue>, String> {
    let mut statement = connection.prepare(&format!("SELECT name FROM {table} ORDER BY name COLLATE NOCASE")).map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(ManagedValue { name: row.get(0)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_categories(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<ManagedValue>, String> {
    let connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "view")?;
    list_values(&connection, "categories")
}

#[tauri::command]
pub fn list_tags(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<ManagedValue>, String> {
    let connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "view")?;
    list_values(&connection, "tags")
}

#[tauri::command]
pub fn create_category(app: AppHandle, state: State<'_, SessionState>, token: String, name: String) -> Result<(), String> {
    let name = validate_name(&name, "Category")?;
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO categories (name, created_at) VALUES (?, ?)", params![name, crate::security::timestamp()]).map_err(|error| format!("Unable to create category: {error}"))?;
    audit(&transaction, Some(&current.id), "category_created", "category", Some(&name), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_category(app: AppHandle, state: State<'_, SessionState>, token: String, old_name: String, new_name: String) -> Result<(), String> {
    let new_name = validate_name(&new_name, "Category")?;
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let changed = transaction.execute("UPDATE categories SET name = ? WHERE name = ? COLLATE NOCASE", params![new_name, old_name.trim()]).map_err(|error| format!("Unable to rename category: {error}"))?;
    if changed == 0 { return Err("Category not found.".into()); }
    transaction.execute("UPDATE documents SET document_type = ?, updated_at = ? WHERE document_type = ? COLLATE NOCASE", params![new_name, crate::security::timestamp(), old_name.trim()]).map_err(|error| error.to_string())?;
    audit(&transaction, Some(&current.id), "category_renamed", "category", Some(&new_name), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_category(app: AppHandle, state: State<'_, SessionState>, token: String, name: String) -> Result<(), String> {
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let count: i64 = transaction.query_row("SELECT count(*) FROM categories", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if count <= 1 { return Err("The archive must keep at least one category.".into()); }
    let used: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM documents WHERE document_type = ? COLLATE NOCASE)", params![name.trim()], |row| row.get(0)).map_err(|error| error.to_string())?;
    if used { return Err("Reassign documents from this category before deleting it.".into()); }
    let changed = transaction.execute("DELETE FROM categories WHERE name = ? COLLATE NOCASE", params![name.trim()]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Category not found.".into()); }
    audit(&transaction, Some(&current.id), "category_deleted", "category", Some(name.trim()), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_tag(app: AppHandle, state: State<'_, SessionState>, token: String, name: String) -> Result<(), String> {
    let name = validate_name(&name, "Tag")?;
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO tags (name, created_at) VALUES (?, ?)", params![name, crate::security::timestamp()]).map_err(|error| format!("Unable to create tag: {error}"))?;
    audit(&transaction, Some(&current.id), "tag_created", "tag", Some(&name), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

fn rewrite_document_tags(connection: &Connection, old_name: Option<&str>, new_name: Option<&str>) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT id, tags FROM documents WHERE tags IS NOT NULL AND tags <> ''").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
    let documents = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    drop(statement);
    for (id, tags) in documents {
        let mut updated = Vec::new();
        let mut changed = false;
        for tag in tags.split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
            let value = if old_name.is_some_and(|old| old.eq_ignore_ascii_case(tag)) {
                changed = true;
                new_name
            } else {
                Some(tag)
            };
            if let Some(value) = value {
                if !updated.iter().any(|existing: &String| existing.eq_ignore_ascii_case(value)) {
                    updated.push(value.to_string());
                }
            }
        }
        if changed {
            connection.execute("UPDATE documents SET tags = ?, updated_at = ? WHERE id = ?", params![updated.join(", "), crate::security::timestamp(), id]).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_tag(app: AppHandle, state: State<'_, SessionState>, token: String, old_name: String, new_name: String) -> Result<(), String> {
    let old_name = validate_name(&old_name, "Tag")?;
    let new_name = validate_name(&new_name, "Tag")?;
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let changed = transaction.execute("UPDATE tags SET name = ? WHERE name = ? COLLATE NOCASE", params![new_name, old_name]).map_err(|error| format!("Unable to rename tag: {error}"))?;
    if changed == 0 { return Err("Tag not found.".into()); }
    rewrite_document_tags(&transaction, Some(&old_name), Some(&new_name))?;
    audit(&transaction, Some(&current.id), "tag_renamed", "tag", Some(&new_name), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_tag(app: AppHandle, state: State<'_, SessionState>, token: String, name: String) -> Result<(), String> {
    let name = validate_name(&name, "Tag")?;
    let mut connection = crate::security::database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let changed = transaction.execute("DELETE FROM tags WHERE name = ? COLLATE NOCASE", params![name]).map_err(|error| error.to_string())?;
    if changed == 0 { return Err("Tag not found.".into()); }
    rewrite_document_tags(&transaction, Some(&name), None)?;
    audit(&transaction, Some(&current.id), "tag_deleted", "tag", Some(&name), None)?;
    transaction.commit().map_err(|error| error.to_string())
}

use rusqlite::{params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};
use crate::{managed_file_path, security::{actor, audit, require, SessionState}};

#[cfg(test)]
mod tests {
    use super::{escape_like, normalize_tags};

    #[test]
    fn normalizes_and_deduplicates_tags() {
        assert_eq!(normalize_tags(Some("  Legal, legal, finance ,, ".into())), Some("Legal, finance".into()));
        assert_eq!(normalize_tags(Some(" ,  ".into())), None);
    }

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(escape_like(r"invoice%_2025\final"), r"invoice\%\_2025\\final");
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: String, pub title: String, pub document_type: String, pub document_date: String,
    pub reference_number: Option<String>, pub person_or_organization: Option<String>, pub department: Option<String>,
    pub description: Option<String>, pub tags: Option<String>, pub file_name: String, pub stored_file_name: String,
    pub file_path: String, pub mime_type: Option<String>, pub file_size: Option<i64>, pub file_hash: Option<String>,
    pub ocr_text: Option<String>, pub ocr_status: String, pub created_by: Option<String>, pub created_at: String,
    pub updated_at: String, pub status: String, #[serde(default)] pub is_favorite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentValues { pub title: String, pub document_type: String, pub document_date: String, pub reference_number: Option<String>, pub person_or_organization: Option<String>, pub department: Option<String>, pub description: Option<String>, pub tags: Option<String>, pub status: String }

fn map_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRecord> { Ok(DocumentRecord { id: row.get(0)?, title: row.get(1)?, document_type: row.get(2)?, document_date: row.get(3)?, reference_number: row.get(4)?, person_or_organization: row.get(5)?, department: row.get(6)?, description: row.get(7)?, tags: row.get(8)?, file_name: row.get(9)?, stored_file_name: row.get(10)?, file_path: row.get(11)?, mime_type: row.get(12)?, file_size: row.get(13)?, file_hash: row.get(14)?, ocr_text: row.get(15)?, ocr_status: row.get(16)?, created_by: row.get(17)?, created_at: row.get(18)?, updated_at: row.get(19)?, status: row.get(20)?, is_favorite: row.get(21)? }) }
const DOCUMENT_COLUMNS: &str = "id, title, document_type, document_date, reference_number, person_or_organization, department, description, tags, file_name, stored_file_name, file_path, mime_type, file_size, file_hash, ocr_text, ocr_status, created_by, created_at, updated_at, status";
fn qualified_columns(alias: &str) -> String { DOCUMENT_COLUMNS.split(", ").map(|column| format!("{alias}.{column}")).collect::<Vec<_>>().join(", ") }
fn document_with_favorite(alias: &str) -> String { format!("{}, EXISTS (SELECT 1 FROM document_favorites f WHERE f.document_id = {alias}.id AND f.user_id = ?)", qualified_columns(alias)) }
fn escape_like(value: &str) -> String { value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_") }
fn normalize_tags(tags: Option<String>) -> Option<String> {
    let mut normalized: Vec<String> = Vec::new();
    for tag in tags.unwrap_or_default().split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
        if !normalized.iter().any(|existing| existing.eq_ignore_ascii_case(tag)) {
            normalized.push(tag.to_string());
        }
    }
    (!normalized.is_empty()).then(|| normalized.join(", "))
}
fn sync_tag_catalog(connection: &rusqlite::Connection, tags: Option<&str>) -> Result<(), String> {
    for tag in tags.unwrap_or_default().split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
        connection.execute("INSERT OR IGNORE INTO tags (name, created_at) VALUES (?, ?)", params![tag, crate::security::timestamp()]).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SearchResults { pub documents: Vec<DocumentRecord>, pub total: i64 }

#[tauri::command]
pub fn find_documents(
    app: AppHandle, state: State<'_, SessionState>, token: String,
    search: String, search_mode: String, document_type: String, from: String, to: String,
    organization: String, tag: String, favorites_only: bool, sort_by: String,
    sort_direction: String, status: String, limit: i64, offset: i64,
) -> Result<SearchResults, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    if status == "active" || status == "archived" {
        conditions.push("d.status = ?".to_string());
        values.push(Value::Text(status));
    }
    let columns = ["title", "reference_number", "person_or_organization", "department", "description", "tags", "file_name", "ocr_text"];
    let terms = search.split_whitespace().collect::<Vec<_>>();
    if !terms.is_empty() {
        let joiner = if search_mode == "any" { " OR " } else { " AND " };
        let term_conditions = terms.iter().map(|term| {
            let pattern = format!("%{}%", escape_like(term));
            let column_conditions = columns.iter().map(|column| {
                values.push(Value::Text(pattern.clone()));
                format!("lower(d.{column}) LIKE lower(?) ESCAPE '\\'")
            }).collect::<Vec<_>>();
            format!("({})", column_conditions.join(" OR "))
        }).collect::<Vec<_>>();
        conditions.push(format!("({})", term_conditions.join(joiner)));
    }
    for (column, value, operator) in [
        ("document_type", document_type, "="),
        ("document_date", from, ">="),
        ("document_date", to, "<="),
    ] {
        if !value.trim().is_empty() {
            conditions.push(format!("d.{column} {operator} ?"));
            values.push(Value::Text(value));
        }
    }
    if !organization.trim().is_empty() {
        conditions.push("lower(d.person_or_organization) LIKE lower(?) ESCAPE '\\'".into());
        values.push(Value::Text(format!("%{}%", escape_like(organization.trim()))));
    }
    if !tag.trim().is_empty() {
        conditions.push("(',' || replace(lower(coalesce(d.tags, '')), ', ', ',') || ',') LIKE lower(?) ESCAPE '\\'".into());
        values.push(Value::Text(format!("%,{},%", escape_like(tag.trim()))));
    }
    if favorites_only {
        conditions.push("EXISTS (SELECT 1 FROM document_favorites fav WHERE fav.document_id = d.id AND fav.user_id = ?)".into());
        values.push(Value::Text(current.id.clone()));
    }
    let where_clause = if conditions.is_empty() { "1 = 1".to_string() } else { conditions.join(" AND ") };
    let total: i64 = connection.query_row(&format!("SELECT count(*) FROM documents d WHERE {where_clause}"), params_from_iter(values.iter()), |row| row.get(0)).map_err(|error| error.to_string())?;
    let sort_column = match sort_by.as_str() {
        "title" => "d.title COLLATE NOCASE",
        "category" => "d.document_type COLLATE NOCASE",
        "created" => "d.created_at",
        "updated" => "d.updated_at",
        "size" => "d.file_size",
        _ => "d.document_date",
    };
    let direction = if sort_direction == "asc" { "ASC" } else { "DESC" };
    let safe_limit = limit.clamp(1, 100);
    let safe_offset = offset.clamp(0, 1_000_000);
    let query = format!("SELECT {} FROM documents d WHERE {where_clause} ORDER BY {sort_column} {direction}, d.created_at DESC, d.id ASC LIMIT ? OFFSET ?", document_with_favorite("d"));
    values.insert(0, Value::Text(current.id));
    values.push(Value::Integer(safe_limit));
    values.push(Value::Integer(safe_offset));
    let mut statement = connection.prepare(&query).map_err(|error| error.to_string())?;
    let rows = statement.query_map(params_from_iter(values.iter()), map_document).map_err(|error| error.to_string())?;
    let documents = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    Ok(SearchResults { documents, total })
}

#[tauri::command]
pub fn find_recent_documents(app: AppHandle, state: State<'_, SessionState>, token: String, limit: i64) -> Result<Vec<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    let query = format!("SELECT {} FROM documents d JOIN recent_documents r ON r.document_id = d.id WHERE r.user_id = ? ORDER BY r.last_opened_at DESC LIMIT ?", document_with_favorite("d"));
    let mut statement = connection.prepare(&query).map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![current.id, current.id, limit.clamp(1, 20)], map_document).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn find_document_by_id(app: AppHandle, state: State<'_, SessionState>, token: String, id: String) -> Result<Option<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    connection.query_row(&format!("SELECT {} FROM documents d WHERE d.id = ?", document_with_favorite("d")), params![current.id, id], map_document).optional().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn find_document_by_hash(app: AppHandle, state: State<'_, SessionState>, token: String, file_hash: String) -> Result<Option<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "create")?;
    connection.query_row(&format!("SELECT {} FROM documents d WHERE d.file_hash = ? LIMIT 1", document_with_favorite("d")), params![current.id, file_hash], map_document).optional().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn insert_document(app: AppHandle, state: State<'_, SessionState>, token: String, mut record: DocumentRecord) -> Result<(), String> {
    let mut connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "create")?; record.created_by = Some(current.id.clone());
    record.tags = normalize_tags(record.tags);
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let category_exists: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE name = ? COLLATE NOCASE)", params![record.document_type], |row| row.get(0)).map_err(|error| error.to_string())?;
    if !category_exists { return Err("Select a category from the managed category list.".into()); }
    transaction.execute("INSERT INTO documents (id, title, document_type, document_date, reference_number, person_or_organization, department, description, tags, file_name, stored_file_name, file_path, mime_type, file_size, file_hash, ocr_text, ocr_status, created_by, created_at, updated_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", params![record.id, record.title, record.document_type, record.document_date, record.reference_number, record.person_or_organization, record.department, record.description, record.tags, record.file_name, record.stored_file_name, record.file_path, record.mime_type, record.file_size, record.file_hash, record.ocr_text, record.ocr_status, record.created_by, record.created_at, record.updated_at, record.status]).map_err(|error| error.to_string())?;
    sync_tag_catalog(&transaction, record.tags.as_deref())?;
    let details = serde_json::to_string(&json!({"title": record.title})).map_err(|_| "Unable to record document audit.".to_string())?;
    audit(&transaction, Some(&current.id), "document_created", "document", Some(&record.id), Some(&details))?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_document(app: AppHandle, state: State<'_, SessionState>, token: String, id: String, mut values: DocumentValues) -> Result<(), String> {
    values.tags = normalize_tags(values.tags);
    let mut connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "update")?;
    let transaction = connection.transaction().map_err(|error| error.to_string())?;
    let category_exists: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE name = ? COLLATE NOCASE)", params![values.document_type], |row| row.get(0)).map_err(|error| error.to_string())?;
    if !category_exists { return Err("Select a category from the managed category list.".into()); }
    let previous: (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String) = transaction.query_row("SELECT title, document_type, document_date, reference_number, person_or_organization, department, description, tags, status FROM documents WHERE id = ?", params![id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))).map_err(|_| "Unable to find document.".to_string())?;
    if previous.8 != values.status { if values.status == "archived" { require(&current, "archive")?; } else if values.status == "active" { require(&current, "restore")?; } else { return Err("Invalid document status.".into()); } }
    transaction.execute("UPDATE documents SET title = ?, document_type = ?, document_date = ?, reference_number = ?, person_or_organization = ?, department = ?, description = ?, tags = ?, status = ?, updated_at = ? WHERE id = ?", params![values.title, values.document_type, values.document_date, values.reference_number, values.person_or_organization, values.department, values.description, values.tags, values.status, crate::security::timestamp(), id]).map_err(|error| error.to_string())?;
    sync_tag_catalog(&transaction, values.tags.as_deref())?;
    if previous.8 != values.status {
        audit(&transaction, Some(&current.id), if values.status == "archived" { "document_archived" } else { "document_restored" }, "document", Some(&id), None)?;
        return transaction.commit().map_err(|error| error.to_string());
    }
    let fields = [
        ("title", previous.0 != values.title), ("document_type", previous.1 != values.document_type), ("document_date", previous.2 != values.document_date),
        ("reference_number", previous.3 != values.reference_number), ("person_or_organization", previous.4 != values.person_or_organization),
        ("department", previous.5 != values.department), ("description", previous.6 != values.description), ("tags", previous.7 != values.tags),
    ].into_iter().filter_map(|(field, changed)| changed.then_some(field)).collect::<Vec<_>>();
    let details = serde_json::to_string(&json!({"changed_fields": fields})).map_err(|_| "Unable to record document audit.".to_string())?;
    audit(&transaction, Some(&current.id), "document_updated", "document", Some(&id), Some(&details))?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_document(app: AppHandle, state: State<'_, SessionState>, token: String, id: String) -> Result<(), String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "delete")?;
    let path: String = connection.query_row("SELECT file_path FROM documents WHERE id = ?", params![id], |row| row.get(0)).map_err(|error| format!("Unable to find document: {error}"))?;
    let managed = managed_file_path(&app, &path)?;
    if managed.exists() { std::fs::remove_file(&managed).map_err(|error| format!("Unable to delete archived file: {error}"))?; }
    connection.execute("DELETE FROM documents WHERE id = ?", params![id]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "document_deleted", "document", Some(&id), None)
}

#[tauri::command]
pub fn open_document(app: AppHandle, state: State<'_, SessionState>, token: String, id: String) -> Result<(), String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "open")?;
    let path: String = connection.query_row("SELECT file_path FROM documents WHERE id = ?", params![id], |row| row.get(0)).map_err(|error| format!("Unable to find document: {error}"))?;
    let path = managed_file_path(&app, &path)?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|error| format!("Unable to open archived file: {error}"))?;
    connection.execute("INSERT INTO recent_documents (user_id, document_id, last_opened_at) VALUES (?, ?, ?) ON CONFLICT(user_id, document_id) DO UPDATE SET last_opened_at = excluded.last_opened_at", params![current.id, id, crate::security::timestamp()]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "document_opened", "document", Some(&id), None)
}

#[tauri::command]
pub fn set_document_favorite(app: AppHandle, state: State<'_, SessionState>, token: String, id: String, favorite: bool) -> Result<(), String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    if favorite {
        connection.execute("INSERT OR IGNORE INTO document_favorites (user_id, document_id, created_at) VALUES (?, ?, ?)", params![current.id, id, crate::security::timestamp()])
    } else {
        connection.execute("DELETE FROM document_favorites WHERE user_id = ? AND document_id = ?", params![current.id, id])
    }.map_err(|error| error.to_string())?;
    Ok(())
}

use rusqlite::OptionalExtension;

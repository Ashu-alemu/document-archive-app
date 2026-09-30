use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};
use crate::{managed_file_path, security::{actor, audit, require, SessionState}};

#[derive(Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: String, pub title: String, pub document_type: String, pub document_date: String,
    pub reference_number: Option<String>, pub person_or_organization: Option<String>, pub department: Option<String>,
    pub description: Option<String>, pub tags: Option<String>, pub file_name: String, pub stored_file_name: String,
    pub file_path: String, pub mime_type: Option<String>, pub file_size: Option<i64>, pub file_hash: Option<String>,
    pub ocr_text: Option<String>, pub ocr_status: String, pub created_by: Option<String>, pub created_at: String,
    pub updated_at: String, pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentValues { pub title: String, pub document_type: String, pub document_date: String, pub reference_number: Option<String>, pub person_or_organization: Option<String>, pub department: Option<String>, pub description: Option<String>, pub tags: Option<String>, pub status: String }

fn map_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRecord> { Ok(DocumentRecord { id: row.get(0)?, title: row.get(1)?, document_type: row.get(2)?, document_date: row.get(3)?, reference_number: row.get(4)?, person_or_organization: row.get(5)?, department: row.get(6)?, description: row.get(7)?, tags: row.get(8)?, file_name: row.get(9)?, stored_file_name: row.get(10)?, file_path: row.get(11)?, mime_type: row.get(12)?, file_size: row.get(13)?, file_hash: row.get(14)?, ocr_text: row.get(15)?, ocr_status: row.get(16)?, created_by: row.get(17)?, created_at: row.get(18)?, updated_at: row.get(19)?, status: row.get(20)? }) }
const DOCUMENT_COLUMNS: &str = "id, title, document_type, document_date, reference_number, person_or_organization, department, description, tags, file_name, stored_file_name, file_path, mime_type, file_size, file_hash, ocr_text, ocr_status, created_by, created_at, updated_at, status";

#[tauri::command]
pub fn find_documents(app: AppHandle, state: State<'_, SessionState>, token: String, search: String, document_type: String, from: String, to: String, status: String) -> Result<Vec<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    let term = format!("%{}%", search.trim()); let status = if status == "archived" { "archived" } else { "active" };
    let mut statement = connection.prepare(&format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE status = ? AND (? = '' OR title LIKE ? OR reference_number LIKE ? OR person_or_organization LIKE ? OR department LIKE ? OR description LIKE ? OR tags LIKE ? OR file_name LIKE ? OR ocr_text LIKE ?) AND (? = '' OR document_type = ?) AND (? = '' OR document_date >= ?) AND (? = '' OR document_date <= ?) ORDER BY document_date DESC, created_at DESC")).map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![status, search.trim(), term, term, term, term, term, term, term, term, document_type, document_type, from, from, to, to], map_document).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn find_document_by_id(app: AppHandle, state: State<'_, SessionState>, token: String, id: String) -> Result<Option<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "view")?;
    connection.query_row(&format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id = ?"), params![id], map_document).optional().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn find_document_by_hash(app: AppHandle, state: State<'_, SessionState>, token: String, file_hash: String) -> Result<Option<DocumentRecord>, String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "create")?;
    connection.query_row(&format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE file_hash = ? LIMIT 1"), params![file_hash], map_document).optional().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn insert_document(app: AppHandle, state: State<'_, SessionState>, token: String, mut record: DocumentRecord) -> Result<(), String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "create")?; record.created_by = Some(current.id.clone());
    connection.execute("INSERT INTO documents (id, title, document_type, document_date, reference_number, person_or_organization, department, description, tags, file_name, stored_file_name, file_path, mime_type, file_size, file_hash, ocr_text, ocr_status, created_by, created_at, updated_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", params![record.id, record.title, record.document_type, record.document_date, record.reference_number, record.person_or_organization, record.department, record.description, record.tags, record.file_name, record.stored_file_name, record.file_path, record.mime_type, record.file_size, record.file_hash, record.ocr_text, record.ocr_status, record.created_by, record.created_at, record.updated_at, record.status]).map_err(|error| error.to_string())?;
    let details = serde_json::to_string(&json!({"title": record.title})).map_err(|_| "Unable to record document audit.".to_string())?;
    audit(&connection, Some(&current.id), "document_created", "document", Some(&record.id), Some(&details))
}

#[tauri::command]
pub fn update_document(app: AppHandle, state: State<'_, SessionState>, token: String, id: String, values: DocumentValues) -> Result<(), String> {
    let connection = crate::security::database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "update")?;
    let previous: (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String) = connection.query_row("SELECT title, document_type, document_date, reference_number, person_or_organization, department, description, tags, status FROM documents WHERE id = ?", params![id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))).map_err(|_| "Unable to find document.".to_string())?;
    if previous.8 != values.status { if values.status == "archived" { require(&current, "archive")?; } else if values.status == "active" { require(&current, "restore")?; } else { return Err("Invalid document status.".into()); } }
    connection.execute("UPDATE documents SET title = ?, document_type = ?, document_date = ?, reference_number = ?, person_or_organization = ?, department = ?, description = ?, tags = ?, status = ?, updated_at = ? WHERE id = ?", params![values.title, values.document_type, values.document_date, values.reference_number, values.person_or_organization, values.department, values.description, values.tags, values.status, crate::security::timestamp(), id]).map_err(|error| error.to_string())?;
    if previous.8 != values.status { return audit(&connection, Some(&current.id), if values.status == "archived" { "document_archived" } else { "document_restored" }, "document", Some(&id), None); }
    let fields = [
        ("title", previous.0 != values.title), ("document_type", previous.1 != values.document_type), ("document_date", previous.2 != values.document_date),
        ("reference_number", previous.3 != values.reference_number), ("person_or_organization", previous.4 != values.person_or_organization),
        ("department", previous.5 != values.department), ("description", previous.6 != values.description), ("tags", previous.7 != values.tags),
    ].into_iter().filter_map(|(field, changed)| changed.then_some(field)).collect::<Vec<_>>();
    let details = serde_json::to_string(&json!({"changed_fields": fields})).map_err(|_| "Unable to record document audit.".to_string())?;
    audit(&connection, Some(&current.id), "document_updated", "document", Some(&id), Some(&details))
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
    audit(&connection, Some(&current.id), "document_opened", "document", Some(&id), None)
}

use rusqlite::OptionalExtension;

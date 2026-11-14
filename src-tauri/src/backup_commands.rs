use crate::security::{actor, audit, database_connection, database_path, notify_best_effort, require, timestamp, SessionState};
use crate::storage_root;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::{Read, Write}, path::{Component, Path, PathBuf}};
use tauri::{AppHandle, Manager, State};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const FORMAT_VERSION: u32 = 2;
const LEGACY_FORMAT_VERSION: u32 = 1;
const RESTORE_DISABLED_MESSAGE: &str = "Backup restore remains disabled until crash-resumable restore journaling is complete.";

fn restore_disabled() -> Result<(), String> {
    Err(RESTORE_DISABLED_MESSAGE.into())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format_version: u32,
    #[serde(default)]
    application_version: Option<String>,
    #[serde(default)]
    migration_version: Option<i64>,
    #[serde(default)]
    schema_version: Option<String>,
    created_at: String,
    files: Vec<BackupFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub file_name: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub verified_at: Option<String>,
    pub file_size: i64,
    pub file_hash: Option<String>,
    pub status: String,
    pub format_version: Option<u32>,
    pub application_version: Option<String>,
    pub migration_version: Option<i64>,
    pub schema_version: Option<String>,
    pub compatibility: String,
    pub compatibility_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compatibility {
    Compatible,
    Older,
    Newer,
    Invalid,
    Legacy,
}

impl Compatibility {
    fn as_str(self) -> &'static str {
        match self { Self::Compatible => "compatible", Self::Older => "older", Self::Newer => "newer", Self::Invalid => "invalid", Self::Legacy => "legacy" }
    }
}

struct StagedValidation {
    migration_version: Option<i64>,
    reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct PendingRestore {
    backup_id: String,
    archive: PathBuf,
    staging: PathBuf,
    rollback: PathBuf,
    user_id: Option<String>,
}

fn current_schema_version() -> String { format!("sql-migrations-{}", crate::current_migration_version()) }

fn backup_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app.path().app_config_dir().map_err(|_| "Unable to access backup storage.".to_string())?.join("backups");
    fs::create_dir_all(&root).map_err(|_| "Unable to access backup storage.".to_string())?;
    Ok(root)
}

fn safe_backup_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    if file_name.is_empty() || file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") { return Err("Invalid backup file name.".into()); }
    let root = backup_root(app)?;
    let path = root.join(file_name);
    if path.parent() != Some(root.as_path()) { return Err("Invalid backup file name.".into()); }
    Ok(path)
}

fn hash_path(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|_| "Unable to read backup content.".to_string())?;
    let mut hasher = Sha256::new(); let mut size = 0_u64; let mut buffer = [0_u8; 1024 * 1024];
    loop { let count = file.read(&mut buffer).map_err(|_| "Unable to read backup content.".to_string())?; if count == 0 { break; } hasher.update(&buffer[..count]); size += count as u64; }
    Ok((size, hex::encode(hasher.finalize())))
}

fn collect_files(root: &Path, current: &Path, output: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    for item in fs::read_dir(current).map_err(|_| "Unable to read managed storage.".to_string())? {
        let item = item.map_err(|_| "Unable to read managed storage.".to_string())?; let path = item.path();
        let relative = path.strip_prefix(root).map_err(|_| "Invalid managed storage path.".to_string())?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Unable to inspect managed storage.".to_string())?;
        if metadata.file_type().is_symlink() { return Err("Symbolic links are not allowed in managed storage.".into()); }
        if metadata.is_dir() { collect_files(root, &path, output)?; } else if metadata.is_file() { output.push((relative.to_string_lossy().replace('\\', "/"), path)); }
    }
    Ok(())
}

fn write_zip_file<W: Write + std::io::Seek>(zip: &mut ZipWriter<W>, archive_path: &str, source: &Path) -> Result<BackupFile, String> {
    let (size, sha256) = hash_path(source)?;
    zip.start_file(archive_path, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(|_| "Unable to create backup archive.".to_string())?;
    let mut file = fs::File::open(source).map_err(|_| "Unable to read backup content.".to_string())?;
    std::io::copy(&mut file, zip).map_err(|_| "Unable to create backup archive.".to_string())?;
    Ok(BackupFile { path: archive_path.into(), size, sha256 })
}

fn archive_entry_is_safe(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('/') && !name.contains('\\') && !name.split('/').any(|part| part == ".." || part.is_empty())
}

fn verify_archive(path: &Path) -> Result<Manifest, String> {
    let file = fs::File::open(path).map_err(|_| "Backup file could not be opened.".to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|_| "Backup archive is invalid.".to_string())?;
    let mut text = String::new();
    { let mut manifest_file = archive.by_name("manifest.json").map_err(|_| "Backup manifest is missing.".to_string())?; manifest_file.read_to_string(&mut text).map_err(|_| "Backup manifest is unreadable.".to_string())?; }
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| "Backup manifest is invalid.".to_string())?;
    if !matches!(manifest.format_version, LEGACY_FORMAT_VERSION | FORMAT_VERSION) || !manifest.files.iter().any(|file| file.path == "database/documents.db") { return Err("Backup format is unsupported or incomplete.".into()); }
    let expected: HashSet<&str> = manifest.files.iter().map(|file| file.path.as_str()).collect();
    if expected.len() != manifest.files.len() { return Err("Backup manifest contains duplicate paths.".into()); }
    let mut seen = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|_| "Backup archive is unreadable.".to_string())?;
        let name = entry.name().to_string();
        if name == "manifest.json" { continue; }
        if !archive_entry_is_safe(&name) || !expected.contains(name.as_str()) || !seen.insert(name) { return Err("Backup contains unexpected or duplicate content.".into()); }
    }
    if seen.len() != expected.len() { return Err("Backup content is incomplete.".into()); }
    for expected_file in &manifest.files {
        let mut entry = archive.by_name(&expected_file.path).map_err(|_| "Backup content is incomplete.".to_string())?;
        let mut hasher = Sha256::new(); let mut size = 0_u64; let mut buffer = [0_u8; 1024 * 1024];
        loop { let count = entry.read(&mut buffer).map_err(|_| "Backup content is unreadable.".to_string())?; if count == 0 { break; } hasher.update(&buffer[..count]); size += count as u64; }
        if size != expected_file.size || hex::encode(hasher.finalize()) != expected_file.sha256 { return Err("Backup verification failed: file contents do not match the manifest.".into()); }
    }
    Ok(manifest)
}

fn extract_archive(path: &Path, destination: &Path, manifest: &Manifest) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|_| "Unable to prepare backup validation staging.".to_string())?;
    fs::create_dir_all(destination.join("storage")).map_err(|_| "Unable to prepare backup validation staging.".to_string())?;
    let file = fs::File::open(path).map_err(|_| "Backup file could not be opened.".to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|_| "Backup archive is invalid.".to_string())?;
    let allowed: HashSet<&str> = manifest.files.iter().map(|item| item.path.as_str()).collect();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| "Backup archive is unreadable.".to_string())?;
        let name = entry.name().to_string();
        if name == "manifest.json" { continue; }
        if !archive_entry_is_safe(&name) || !allowed.contains(name.as_str()) || !(name == "database/documents.db" || name.starts_with("storage/")) { return Err("Backup contains an unsafe or unexpected path.".into()); }
        let target = destination.join(&name);
        if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(|_| "Unable to prepare backup validation staging.".to_string())?; }
        let mut output = fs::File::create_new(&target).map_err(|_| "Backup contains duplicate or invalid content.".to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|_| "Unable to extract backup content for validation.".to_string())?;
    }
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)", params![table], |row| row.get(0)).map_err(|error| error.to_string())
}

fn required_schema_check(connection: &Connection) -> Result<(), String> {
    let required = [
        ("documents", &["id", "title", "document_type", "document_date", "reference_number", "person_or_organization", "department", "description", "tags", "file_name", "stored_file_name", "file_path", "mime_type", "file_size", "file_hash", "ocr_text", "ocr_status", "created_by", "created_at", "updated_at", "status"] as &[&str]),
        ("roles", &["id", "name", "description"]), ("users", &["id", "username", "display_name", "password_hash", "active", "created_at", "updated_at", "last_login_at"]), ("user_roles", &["user_id", "role_id"]),
        ("audit_logs", &["id", "user_id", "action", "entity_type", "entity_id", "details", "created_at"]), ("backup_history", &["id", "file_name", "created_at", "completed_at", "verified_at", "file_size", "file_hash", "status", "error_message"]),
        ("categories", &["name", "created_at"]), ("tags", &["name", "created_at"]), ("document_favorites", &["user_id", "document_id", "created_at"]),
        ("recent_documents", &["user_id", "document_id", "last_opened_at"]), ("notifications", &["id", "action", "title", "message", "created_at"]),
        ("user_security_preferences", &["user_id", "inactivity_timeout_minutes", "updated_at"]), ("departments", &["id", "name", "description", "icon", "active", "sort_order", "created_at", "updated_at"]),
        ("department_memberships", &["user_id", "department_id", "role_id", "active"]), ("department_roles", &["id", "name", "description"]),
    ];
    for (table, columns) in required {
        if !table_exists(connection, table)? { return Err(format!("Required table is missing: {table}")); }
        let actual = connection.prepare(&format!("SELECT name FROM pragma_table_info('{table}')")).and_then(|mut statement| statement.query_map([], |row| row.get::<_, String>(0)).and_then(|rows| rows.collect::<Result<Vec<_>, _>>())).map_err(|error| error.to_string())?;
        for column in columns { if !actual.iter().any(|item| item == column) { return Err(format!("Required column is missing: {table}.{column}")); } }
    }
    Ok(())
}

fn migration_version(connection: &Connection) -> Result<i64, String> {
    if !table_exists(connection, "_sqlx_migrations")? { return Err("Migration history table is missing.".into()); }
    let mut statement = connection.prepare("SELECT version, success FROM _sqlx_migrations ORDER BY version").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?))).map_err(|error| error.to_string())?;
    let mut versions = Vec::new();
    for row in rows { let (version, success) = row.map_err(|error| error.to_string())?; if !success { return Err(format!("Migration {version} is not marked successful.")); } versions.push(version); }
    let highest = versions.iter().copied().max().ok_or_else(|| "Migration history is empty.".to_string())?;
    for expected in 1..=highest { if !versions.contains(&expected) { return Err(format!("Migration {expected} is missing from the contiguous history.")); } }
    Ok(highest)
}

fn staged_document_reference_check(connection: &Connection, storage: &Path) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT id, file_path, file_size, file_hash FROM documents").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, Option<String>>(3)?))).map_err(|error| error.to_string())?;
    for row in rows {
        let (id, file_path, expected_size, expected_hash) = row.map_err(|error| error.to_string())?;
        let normalized = file_path.replace('\\', "/");
        let relative = normalized.split_once("/storage/").map(|(_, value)| value).or_else(|| normalized.strip_prefix("storage/"));
        let relative = relative.ok_or_else(|| format!("Document {id} does not reference managed storage."))?;
        let relative_path = PathBuf::from(relative);
        if relative.is_empty() || relative_path.components().any(|component| !matches!(component, Component::Normal(_))) { return Err(format!("Document {id} references an unsafe storage path.")); }
        let path = storage.join(relative_path);
        if !path.is_file() { return Err(format!("Managed file for document {id} is missing from the backup.")); }
        let (actual_size, actual_hash) = hash_path(&path)?;
        if expected_size.is_some_and(|size| size as u64 != actual_size) || expected_hash.as_deref().is_some_and(|hash| hash != actual_hash) { return Err(format!("Managed file for document {id} does not match its database metadata.")); }
    }
    Ok(())
}

fn validate_managed_document_path(file_path: &str, managed_root: &Path) -> Result<(), String> {
    let path = PathBuf::from(file_path.replace('\\', "/"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| "Managed document path is unavailable.".to_string())?;
    if metadata.file_type().is_symlink() { return Err("Managed document path is a symbolic link.".into()); }
    let canonical_root = fs::canonicalize(managed_root).map_err(|_| "Managed storage root is unavailable.".to_string())?;
    let canonical_path = fs::canonicalize(&path).map_err(|_| "Managed document path is unavailable.".to_string())?;
    if !canonical_path.starts_with(&canonical_root) { return Err("Managed document path is outside the storage root.".into()); }
    Ok(())
}

fn validate_staged_database(database: &Path, storage: &Path, managed_root: Option<&Path>) -> StagedValidation {
    let connection = match Connection::open(database) { Ok(connection) => connection, Err(error) => return StagedValidation { migration_version: None, reason: Some(format!("Unable to open staged database: {error}")) } };
    if let Err(error) = connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;") { return StagedValidation { migration_version: None, reason: Some(error.to_string()) }; }
    let migration = migration_version(&connection).ok();
    let reason = connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)).map_err(|error| error.to_string()).and_then(|result| if result == "ok" { Ok(()) } else { Err(format!("SQLite integrity check failed: {result}")) }).and_then(|_| {
        let violations: i64 = connection.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| row.get(0)).map_err(|error| error.to_string())?;
        if violations == 0 { Ok(()) } else { Err(format!("SQLite foreign-key check found {violations} violation(s).")) }
    }).and_then(|_| required_schema_check(&connection)).and_then(|_| {
        if let Some(root) = managed_root {
            let canonical_root = fs::canonicalize(root).map_err(|error| format!("Unable to resolve managed storage root: {error}"))?;
            let mut statement = connection.prepare("SELECT id, file_path FROM documents").map_err(|error| error.to_string())?;
            let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
            for row in rows {
                let (id, file_path) = row.map_err(|error| error.to_string())?;
                validate_managed_document_path(&file_path, &canonical_root).map_err(|error| format!("Document {id}: {error}"))?;
            }
        }
        staged_document_reference_check(&connection, storage)
    }).err();
    StagedValidation { migration_version: migration, reason }
}

fn classify(manifest: &Manifest, staged: &StagedValidation) -> (Compatibility, Option<String>) {
    let current = crate::current_migration_version();
    if manifest.format_version == LEGACY_FORMAT_VERSION || manifest.application_version.is_none() || manifest.migration_version.is_none() || manifest.schema_version.is_none() {
        return (Compatibility::Legacy, Some("Legacy manifest metadata is missing application, migration, or schema version information.".into()));
    }
    if let Some(reason) = &staged.reason {
        if staged.migration_version.is_none() { return (Compatibility::Invalid, Some(reason.clone())); }
    }
    let declared = manifest.migration_version.unwrap_or_default();
    let actual = staged.migration_version.unwrap_or_default();
    if declared != actual { return (Compatibility::Invalid, Some(format!("Manifest migration version {declared} does not match database migration version {actual}."))); }
    if actual < current { return (Compatibility::Older, Some(format!("Backup database is at migration {actual}; application requires migration {current}."))); }
    if actual > current { return (Compatibility::Newer, Some(format!("Backup database is at migration {actual}; application supports migration {current}."))); }
    if manifest.application_version.as_deref() != Some(env!("CARGO_PKG_VERSION")) { return (Compatibility::Invalid, Some("Backup application version does not match the current application.".into())); }
    if manifest.schema_version.as_deref() != Some(current_schema_version().as_str()) { return (Compatibility::Invalid, Some("Backup schema version does not match the current application schema.".into())); }
    if let Some(reason) = &staged.reason { return (Compatibility::Invalid, Some(reason.clone())); }
    (Compatibility::Compatible, Some("ZIP, database, schema, migration history, and managed-file references passed validation.".into()))
}

fn backup_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<BackupRecord> { Ok(BackupRecord { id: row.get(0)?, file_name: row.get(1)?, created_at: row.get(2)?, completed_at: row.get(3)?, verified_at: row.get(4)?, file_size: row.get(5)?, file_hash: row.get(6)?, status: row.get(7)?, format_version: None, application_version: None, migration_version: None, schema_version: None, compatibility: "unknown".into(), compatibility_reason: None }) }

fn attach_manifest_metadata(app: &AppHandle, mut record: BackupRecord) -> BackupRecord {
    if let Ok(path) = safe_backup_path(app, &record.file_name) { if let Ok(manifest) = verify_archive(&path) { record.format_version = Some(manifest.format_version); record.application_version = manifest.application_version.clone(); record.migration_version = manifest.migration_version; record.schema_version = manifest.schema_version.clone(); let staged = validate_archive_compatibility(&path, &manifest); match staged { Ok((compatibility, reason)) => { record.compatibility = compatibility.as_str().into(); record.compatibility_reason = reason; }, Err(reason) => { record.compatibility = Compatibility::Invalid.as_str().into(); record.compatibility_reason = Some(reason); } } } }
    record
}

fn validate_archive_compatibility(path: &Path, manifest: &Manifest) -> Result<(Compatibility, Option<String>), String> {
    let staging = path.parent().ok_or_else(|| "Unable to prepare backup validation staging.".to_string())?.join(format!(".verify-{}", uuid::Uuid::new_v4()));
    let result: Result<(Compatibility, Option<String>), String> = (|| { extract_archive(path, &staging, manifest)?; let staged = validate_staged_database(&staging.join("database/documents.db"), &staging.join("storage"), None); Ok(classify(manifest, &staged)) })();
    let cleanup = fs::remove_dir_all(&staging).err().map(|error| format!("Backup validation cleanup failed: {error}"));
    match (result, cleanup) {
        (Ok(value), None) => Ok(value),
        (Ok(_), Some(error)) => Err(error),
        (Err(error), None) => Err(error),
        (Err(error), Some(cleanup_error)) => Err(format!("{error} {cleanup_error}")),
    }
}

fn validate_staged_backup(snapshot: &Path, storage: &Path) -> Result<(), String> {
    let staged = validate_staged_database(snapshot, storage, Some(storage));
    staged.reason.map_or(Ok(()), Err)
}

fn restore_marker(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map(|path| path.join("restore-pending.json")).map_err(|error| error.to_string())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|error| format!("Unable to inspect rollback source: {error}"))?;
    if metadata.file_type().is_symlink() { return Err("Rollback source contains a symbolic link.".into()); }
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|error| format!("Unable to prepare rollback copy: {error}"))?;
        for item in fs::read_dir(source).map_err(|error| format!("Unable to read rollback source: {error}"))? {
            let item = item.map_err(|error| format!("Unable to inspect rollback source: {error}"))?;
            copy_tree(&item.path(), &destination.join(item.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions()).map_err(|error| format!("Unable to preserve rollback permissions: {error}"))?;
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|error| format!("Unable to prepare rollback copy: {error}"))?; }
        fs::copy(source, destination).map_err(|error| format!("Unable to create rollback copy: {error}"))?;
        fs::set_permissions(destination, metadata.permissions()).map_err(|error| format!("Unable to preserve rollback permissions: {error}"))?;
    } else {
        return Err("Rollback source contains an unsupported filesystem entry.".into());
    }
    Ok(())
}

fn verify_tree_copy(source: &Path, copy: &Path) -> Result<(), String> {
    let mut source_files = Vec::new();
    let mut copy_files = Vec::new();
    if source.is_dir() { collect_files(source, source, &mut source_files)?; } else { source_files.push((String::new(), source.to_path_buf())); }
    if copy.is_dir() { collect_files(copy, copy, &mut copy_files)?; } else { copy_files.push((String::new(), copy.to_path_buf())); }
    if source_files.len() != copy_files.len() { return Err("Rollback copy is incomplete.".into()); }
    for (relative, source_path) in source_files {
        let copy_path = copy.join(&relative);
        let (_, source_hash) = hash_path(&source_path)?;
        let (_, copy_hash) = hash_path(&copy_path)?;
        if source_hash != copy_hash { return Err("Rollback copy failed hash verification.".into()); }
    }
    Ok(())
}

fn write_pending_restore(path: &Path, pending: &PendingRestore) -> Result<(), String> {
    let temporary = path.with_extension("json.part");
    let bytes = serde_json::to_vec_pretty(pending).map_err(|error| format!("Unable to record pending restore: {error}"))?;
    fs::write(&temporary, bytes).map_err(|error| format!("Unable to record pending restore: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Unable to publish pending restore: {error}"))
}

fn validate_installed_restore(database: &Path, storage: &Path) -> Result<(), String> {
    let staged = validate_staged_database(database, storage, None);
    staged.reason.map_or(Ok(()), Err)
}

fn rollback_installation(database: &Path, storage: &Path, displaced: &Path, database_moved: bool, storage_moved: bool) -> Result<(), String> {
    let failed = displaced.join("failed-install");
    fs::create_dir_all(&failed).map_err(|error| format!("Unable to prepare failed restore storage: {error}"))?;
    let displaced_database = displaced.join("documents.db");
    let displaced_storage = displaced.join("storage");
    if database_moved {
        if !displaced_database.exists() { return Err("The displaced original database is missing.".into()); }
        if database.exists() { fs::rename(database, failed.join("documents.db")).map_err(|error| format!("Unable to preserve failed database: {error}"))?; }
        fs::rename(displaced_database, database).map_err(|error| format!("Unable to restore the original database: {error}"))?;
    }
    if storage_moved {
        if !displaced_storage.exists() { return Err("The displaced original storage is missing.".into()); }
        if storage.exists() { fs::rename(storage, failed.join("storage")).map_err(|error| format!("Unable to preserve failed storage: {error}"))?; }
        fs::rename(displaced_storage, storage).map_err(|error| format!("Unable to restore the original storage: {error}"))?;
    }
    Ok(())
}

#[allow(dead_code)]
pub fn recover_pending_restore(app: &AppHandle) -> Result<(), String> {
    let marker = restore_marker(app)?;
    if !marker.exists() { return Ok(()); }
    let pending: PendingRestore = serde_json::from_slice(&fs::read(&marker).map_err(|error| format!("Unable to read pending restore: {error}"))?).map_err(|error| format!("Pending restore metadata is invalid: {error}"))?;
    let app_data = app.path().app_config_dir().map_err(|error| error.to_string())?;
    let database = database_path(app)?;
    let storage = storage_root(app)?;
    let displaced = app_data.join(format!(".restore-displaced-{}", pending.backup_id));
    let stage_database = pending.staging.join("database/documents.db");
    let stage_storage = pending.staging.join("storage");
    let manifest = verify_archive(&pending.archive)?;
    let (compatibility, reason) = validate_archive_compatibility(&pending.archive, &manifest)?;
    if compatibility != Compatibility::Compatible { return Err(reason.unwrap_or_else(|| "Pending restore archive is no longer compatible.".into())); }
    let connection = Connection::open(&database).map_err(|error| format!("Unable to open database before restore: {error}"))?;
    let _ = audit(&connection, pending.user_id.as_deref(), "backup_restore_started", "backup", Some(&pending.backup_id), Some("Installation started after controlled restart"));
    drop(connection);
    if let Err(error) = validate_installed_restore(&stage_database, &stage_storage) {
        let connection = Connection::open(&database).ok();
        if let Some(connection) = connection { let _ = audit(&connection, pending.user_id.as_deref(), "backup_restore_failed", "backup", Some(&pending.backup_id), Some(&error)); }
        return Err(format!("Staged restore validation failed after restart: {error}"));
    }
    fs::create_dir_all(pending.rollback.join("database")).map_err(|error| format!("Unable to prepare rollback copy: {error}"))?;
    copy_tree(&database, &pending.rollback.join("database/documents.db"))?;
    copy_tree(&storage, &pending.rollback.join("storage"))?;
    verify_tree_copy(&database, &pending.rollback.join("database/documents.db"))?;
    verify_tree_copy(&storage, &pending.rollback.join("storage"))?;
    fs::create_dir_all(&displaced).map_err(|error| format!("Unable to prepare restore installation: {error}"))?;
    let mut database_moved = false;
    let mut storage_moved = false;
    let install_result = (|| {
        fs::rename(&database, displaced.join("documents.db")).map_err(|error| format!("Unable to move the live database to safety: {error}"))?;
        database_moved = true;
        fs::rename(&storage, displaced.join("storage")).map_err(|error| format!("Unable to move managed storage to safety: {error}"))?;
        storage_moved = true;
        fs::rename(&stage_database, &database).map_err(|error| format!("Unable to install restored database: {error}"))?;
        fs::rename(&stage_storage, &storage).map_err(|error| format!("Unable to install restored storage: {error}"))?;
        validate_installed_restore(&database, &storage)
    })();
    if let Err(error) = install_result {
        let connection = Connection::open(&database).ok();
        let _ = connection.as_ref().map(|connection| audit(connection, pending.user_id.as_deref(), "backup_restore_rollback_started", "backup", Some(&pending.backup_id), Some(&error)));
        let rollback = rollback_installation(&database, &storage, &displaced, database_moved, storage_moved);
        if let Ok(connection) = Connection::open(&database) { let _ = audit(&connection, pending.user_id.as_deref(), if rollback.is_ok() { "backup_restore_rollback_completed" } else { "backup_restore_failed" }, "backup", Some(&pending.backup_id), rollback.as_ref().err().map(String::as_str)); }
        if let Err(rollback_error) = rollback { return Err(format!("Restore failed and rollback failed: {error}; {rollback_error}")); }
        return Err(format!("Restore failed and was rolled back: {error}"));
    }
    let mut connection = Connection::open(&database).map_err(|error| format!("Unable to open restored database for audit: {error}"))?;
    crate::security::ensure_department_role_schema(&mut connection)?;
    if let Err(error) = audit(&connection, pending.user_id.as_deref(), "backup_restore_completed", "backup", Some(&pending.backup_id), Some("Restore validated after controlled restart")) { log::warn!("Restore completed but audit logging failed: {error}"); }
    drop(connection);
    fs::remove_dir_all(&pending.staging).map_err(|error| format!("Restore completed, but staging cleanup failed: {error}"))?;
    fs::remove_dir_all(&displaced).map_err(|error| format!("Restore completed, but temporary installation cleanup failed: {error}"))?;
    fs::remove_file(&marker).map_err(|error| format!("Restore completed, but pending marker cleanup failed: {error}"))?;
    Ok(())
}

struct Publication {
    cleanup_warning: Option<String>,
}

fn publish_archive_with_cleanup<F>(temporary: &Path, destination: &Path, cleanup: F) -> Result<Publication, String>
where
    F: FnOnce(&Path) -> Result<(), std::io::Error>,
{
    if destination.exists() {
        return Err("A backup with the destination filename already exists.".into());
    }
    fs::hard_link(temporary, destination).map_err(|error| format!("Unable to publish backup without replacement: {error}"))?;
    let cleanup_warning = cleanup(temporary).err().map(|error| format!("Backup was published, but its temporary archive could not be removed: {error}"));
    Ok(Publication { cleanup_warning })
}

fn publish_archive_without_replacement(temporary: &Path, destination: &Path) -> Result<Publication, String> {
    publish_archive_with_cleanup(temporary, destination, |path| fs::remove_file(path))
}

fn update_backup_status(connection: &Connection, id: &str, status: &str, error_message: Option<&str>) -> Result<(), String> {
    connection.execute("UPDATE backup_history SET status = ?, error_message = ? WHERE id = ?", params![status, error_message, id]).map_err(|error| format!("Unable to mark backup {status}: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn create_backup(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<BackupRecord, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    let id = format!("BKP-{}", uuid::Uuid::new_v4()); let now = timestamp(); let file_name = format!("document-archive-{}.zip", id.to_ascii_lowercase()); let destination = safe_backup_path(&app, &file_name)?;
    connection.execute("INSERT INTO backup_history (id, file_name, created_at, status) VALUES (?, ?, ?, 'creating')", params![id, file_name, now]).map_err(|_| "Unable to record backup history.".to_string())?;
    let staging = destination.parent().ok_or_else(|| "Unable to prepare backup.".to_string())?.join(format!(".{}-staging", id)); let temporary_archive = staging.join("backup.zip"); let mut published = false; let result: Result<(i64, String, Option<String>), String> = (|| {
        audit(&connection, Some(&current.id), "backup_created", "backup", Some(&id), Some("Backup creation started"))?;
        fs::create_dir_all(staging.join("database")).map_err(|_| "Unable to prepare backup.".to_string())?;
        let snapshot = staging.join("database/documents.db");
        connection.execute("VACUUM INTO ?", params![snapshot.to_string_lossy().to_string()]).map_err(|_| "Unable to snapshot the archive database.".to_string())?;
        let storage = storage_root(&app)?; validate_staged_backup(&snapshot, &storage)?;
        let mut sources = Vec::new(); collect_files(&storage, &storage, &mut sources)?;
        let file = fs::File::create_new(&temporary_archive).map_err(|_| "Unable to create temporary backup archive.".to_string())?; let mut zip = ZipWriter::new(file); let mut entries = vec![write_zip_file(&mut zip, "database/documents.db", &snapshot)?];
        for (relative, source) in sources { entries.push(write_zip_file(&mut zip, &format!("storage/{relative}"), &source)?); }
        let manifest = Manifest { format_version: FORMAT_VERSION, application_version: Some(env!("CARGO_PKG_VERSION").into()), migration_version: Some(crate::current_migration_version()), schema_version: Some(current_schema_version()), created_at: now.clone(), files: entries }; let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| "Unable to create backup manifest.".to_string())?;
        zip.start_file("manifest.json", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(|_| "Unable to create backup manifest.".to_string())?; zip.write_all(&bytes).map_err(|_| "Unable to create backup manifest.".to_string())?; zip.finish().map_err(|_| "Unable to finalize backup file.".to_string())?;
        let manifest = verify_archive(&temporary_archive)?;
        validate_archive_compatibility(&temporary_archive, &manifest).and_then(|(compatibility, reason)| if compatibility == Compatibility::Compatible { if reason.is_some() { Ok(()) } else { Err("Backup compatibility validation did not produce a result.".into()) } } else { Err(reason.unwrap_or_else(|| "Backup compatibility validation failed.".into())) })?;
        update_backup_status(&connection, &id, "staged", None)?;
        let (size, hash) = hash_path(&temporary_archive)?;
        let publication = publish_archive_without_replacement(&temporary_archive, &destination)?;
        published = true;
        update_backup_status(&connection, &id, "published", publication.cleanup_warning.as_deref())?;
        Ok((size as i64, hash, publication.cleanup_warning))
    })();
    let cleanup_error = fs::remove_dir_all(&staging).err().map(|error| format!("Temporary backup cleanup failed: {error}"));
    match result {
        Ok((size, hash, publication_warning)) => {
            let diagnostics = [publication_warning, cleanup_error].into_iter().flatten().collect::<Vec<_>>().join(" ");
            if let Err(error) = connection.execute("UPDATE backup_history SET completed_at = ?, verified_at = ?, file_size = ?, file_hash = ?, status = 'verified', error_message = ? WHERE id = ?", params![timestamp(), timestamp(), size, hash, if diagnostics.is_empty() { None } else { Some(diagnostics.as_str()) }, id]) {
                let detail = format!("Backup archive was published but could not be marked verified: {error}. File retained at {}.", destination.display());
                let recovery_error = update_backup_status(&connection, &id, "published", Some(&detail)).err();
                let detail = recovery_error.map_or(detail.clone(), |recovery| format!("{detail} History recovery also failed: {recovery}"));
                return Err(detail);
            }
            let audit_error = audit(&connection, Some(&current.id), "backup_verified", "backup", Some(&id), None).err();
            notify_best_effort(&connection, Some(&current.id), "backup_created", "Backup created", "A backup archive was created and its file hashes were verified; compatibility still requires explicit validation.", Some("backup"), Some(&id), "info");
            let record = connection.query_row("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history WHERE id = ?", params![id], backup_record).map_err(|error| format!("Backup is verified, but its history record could not be loaded: {error}"))?;
            if let Some(error) = audit_error { return Err(format!("Backup is verified, but audit logging failed: {error}")); }
            Ok(attach_manifest_metadata(&app, record))
        }
        Err(error) => {
            if published {
                let detail = format!("Backup archive was published, but finalization failed: {error}. File retained at {}.", destination.display());
                let _ = update_backup_status(&connection, &id, "published", Some(&detail));
                Err(detail)
            } else {
                let mut detail = error;
                if let Some(cleanup) = cleanup_error { detail.push_str(&format!(" {cleanup}")); }
                if let Err(history_error) = update_backup_status(&connection, &id, "failed", Some(&detail)) { detail.push_str(&format!(" History status update failed: {history_error}")); }
                let _ = audit(&connection, Some(&current.id), "backup_failed", "backup", Some(&id), Some(&detail));
                notify_best_effort(&connection, Some(&current.id), "backup_failed", "Backup failed", "A backup could not be completed.", Some("backup"), Some(&id), "error");
                Err("Backup creation failed.".into())
            }
        }
    }
}

#[tauri::command]
pub fn list_backups(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<BackupRecord>, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    let mut statement = connection.prepare("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history ORDER BY created_at DESC LIMIT 100").map_err(|_| "Unable to load backup history.".to_string())?;
    let rows = statement.query_map([], backup_record).map_err(|_| "Unable to load backup history.".to_string())?; let records = rows.collect::<Result<Vec<_>, _>>().map_err(|_| "Unable to load backup history.".to_string())?; Ok(records.into_iter().map(|record| attach_manifest_metadata(&app, record)).collect())
}

#[tauri::command]
pub fn verify_backup(app: AppHandle, state: State<'_, SessionState>, token: String, backup_id: String) -> Result<BackupRecord, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?; let file_name: String = connection.query_row("SELECT file_name FROM backup_history WHERE id = ?", params![backup_id], |row| row.get(0)).map_err(|_| "Backup was not found.".to_string())?; let path = safe_backup_path(&app, &file_name)?;
    let manifest = verify_archive(&path)?; let (compatibility, reason) = validate_archive_compatibility(&path, &manifest)?; let (size, hash) = hash_path(&path)?; let mut record = connection.query_row("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history WHERE id = ?", params![backup_id], backup_record).map_err(|_| "Unable to load backup record.".to_string())?; record.file_size = size as i64; record.file_hash = Some(hash); record.format_version = Some(manifest.format_version); record.application_version = manifest.application_version; record.migration_version = manifest.migration_version; record.schema_version = manifest.schema_version; record.compatibility = compatibility.as_str().into(); record.compatibility_reason = reason; Ok(record)
}

#[tauri::command]
pub fn restore_backup(app: AppHandle, state: State<'_, SessionState>, token: String, backup_id: String) -> Result<(), String> {
    restore_disabled()?;
    let connection = database_connection(&app)?;
    let current = actor(&connection, &state, &token)?;
    require(&current, "admin")?;
    let marker = restore_marker(&app)?;
    if marker.exists() { return Err("A restore is already pending application restart.".into()); }
    let file_name: String = connection.query_row("SELECT file_name FROM backup_history WHERE id = ?", params![backup_id], |row| row.get(0)).map_err(|_| "Backup was not found.".to_string())?;
    let status: String = connection.query_row("SELECT status FROM backup_history WHERE id = ?", params![backup_id], |row| row.get(0)).map_err(|_| "Backup history could not be read.".to_string())?;
    if status != "verified" { return Err("Only verified backups can be restored.".into()); }
    let archive_path = safe_backup_path(&app, &file_name)?;
    let manifest = verify_archive(&archive_path)?;
    let (compatibility, reason) = validate_archive_compatibility(&archive_path, &manifest)?;
    if compatibility != Compatibility::Compatible { return Err(reason.unwrap_or_else(|| "Backup is not compatible with this application.".into())); }
    let app_data = app.path().app_config_dir().map_err(|_| "Unable to access application storage.".to_string())?;
    let staging = app_data.join(format!(".restore-{}-{}", backup_id, uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging).map_err(|_| "Unable to prepare restore staging.".to_string())?;
    if let Err(error) = extract_archive(&archive_path, &staging, &manifest).and_then(|_| validate_installed_restore(&staging.join("database/documents.db"), &staging.join("storage"))) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    audit(&connection, Some(&current.id), "backup_restore_requested", "backup", Some(&backup_id), Some("Restore staged; application restart required"))?;
    let pending = PendingRestore { backup_id: backup_id.clone(), archive: archive_path, staging: staging.clone(), rollback: app_data.join(format!(".restore-rollback-{}-{}", backup_id, uuid::Uuid::new_v4())), user_id: Some(current.id) };
    if let Err(error) = write_pending_restore(&marker, &pending) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    drop(connection);
    app.request_restart();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{archive_entry_is_safe, audit, classify, current_schema_version, publish_archive_with_cleanup, publish_archive_without_replacement, restore_disabled, rollback_installation, staged_document_reference_check, update_backup_status, validate_managed_document_path, validate_staged_database, Compatibility, Manifest, StagedValidation, LEGACY_FORMAT_VERSION, FORMAT_VERSION};
    use rusqlite::Connection;
    use std::{fs, path::PathBuf};

    fn test_directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("document-archive-backup-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn restore_remains_disabled_until_journal_recovery_is_complete() {
        assert!(restore_disabled().is_err());
    }

    fn manifest(format_version: u32, migration_version: Option<i64>) -> Manifest {
        Manifest { format_version, application_version: Some(env!("CARGO_PKG_VERSION").into()), migration_version, schema_version: Some(current_schema_version()), created_at: "test".into(), files: Vec::new() }
    }

    #[test]
    fn compatibility_classifies_legacy_older_newer_and_current_backups() {
        let current = crate::current_migration_version();
        let legacy = Manifest { format_version: LEGACY_FORMAT_VERSION, application_version: None, migration_version: None, schema_version: None, created_at: "test".into(), files: Vec::new() };
        assert_eq!(classify(&legacy, &StagedValidation { migration_version: Some(current), reason: None }).0, Compatibility::Legacy);
        assert_eq!(classify(&manifest(FORMAT_VERSION, Some(current - 1)), &StagedValidation { migration_version: Some(current - 1), reason: None }).0, Compatibility::Older);
        assert_eq!(classify(&manifest(FORMAT_VERSION, Some(current + 1)), &StagedValidation { migration_version: Some(current + 1), reason: None }).0, Compatibility::Newer);
        assert_eq!(classify(&manifest(FORMAT_VERSION, Some(current)), &StagedValidation { migration_version: Some(current), reason: None }).0, Compatibility::Compatible);
    }

    #[test]
    fn schema_failure_is_invalid_at_current_migration() {
        let current = crate::current_migration_version();
        assert_eq!(classify(&manifest(FORMAT_VERSION, Some(current)), &StagedValidation { migration_version: Some(current), reason: Some("missing table".into()) }).0, Compatibility::Invalid);
    }

    #[test]
    fn archive_paths_reject_traversal_and_absolute_names() {
        assert!(archive_entry_is_safe("database/documents.db"));
        assert!(!archive_entry_is_safe("../database/documents.db"));
        assert!(!archive_entry_is_safe("/database/documents.db"));
        assert!(!archive_entry_is_safe("storage\\document.pdf"));
        assert!(!archive_entry_is_safe("storage//document.pdf"));
    }

    #[test]
    fn document_reference_validation_rejects_missing_hash_and_unsafe_files() {
        let directory = test_directory("references");
        let storage = directory.join("storage");
        fs::create_dir_all(storage.join("2026/01")).unwrap();
        let file = storage.join("2026/01/DOC-1.pdf");
        fs::write(&file, b"document").unwrap();
        let (size, hash) = super::hash_path(&file).unwrap();
        let connection = Connection::open_in_memory().unwrap();
        connection.execute("CREATE TABLE documents (id TEXT, file_path TEXT, file_size INTEGER, file_hash TEXT)", []).unwrap();
        connection.execute("INSERT INTO documents VALUES ('DOC-1', ?, ?, ?)", rusqlite::params![file.to_string_lossy(), size as i64, hash]).unwrap();
        assert!(staged_document_reference_check(&connection, &storage).is_ok());

        connection.execute("UPDATE documents SET file_hash = 'wrong'", []).unwrap();
        assert!(staged_document_reference_check(&connection, &storage).is_err());
        connection.execute("UPDATE documents SET file_hash = ?, file_path = 'storage/../outside.pdf'", rusqlite::params![hash]).unwrap();
        assert!(staged_document_reference_check(&connection, &storage).is_err());
        connection.execute("UPDATE documents SET file_path = 'storage/2026/01/missing.pdf'", []).unwrap();
        assert!(staged_document_reference_check(&connection, &storage).is_err());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn staged_database_validation_reports_foreign_key_violations() {
        let directory = test_directory("foreign-keys");
        let database = directory.join("documents.db");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch("PRAGMA foreign_keys = OFF; CREATE TABLE parent (id TEXT PRIMARY KEY); CREATE TABLE child (parent_id TEXT REFERENCES parent(id)); INSERT INTO child VALUES ('missing');").unwrap();
        drop(connection);
        let validation = validate_staged_database(&database, &directory.join("storage"), None);
        assert!(validation.reason.unwrap().contains("foreign-key check"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn staged_database_validation_reports_invalid_database() {
        let directory = test_directory("integrity");
        let database = directory.join("documents.db");
        fs::write(&database, b"not a sqlite database").unwrap();
        let validation = validate_staged_database(&database, &directory.join("storage"), None);
        assert!(validation.reason.is_some());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn publication_rejects_existing_destination_without_replacing_it() {
        let directory = test_directory("destination-collision");
        let temporary = directory.join("staged.zip");
        let destination = directory.join("backup.zip");
        fs::write(&temporary, b"new").unwrap();
        fs::write(&destination, b"existing").unwrap();
        assert!(publish_archive_without_replacement(&temporary, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
        assert_eq!(fs::read(&temporary).unwrap(), b"new");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn publication_failure_preserves_existing_backup() {
        let directory = test_directory("publication-failure");
        let temporary = directory.join("missing.zip");
        let destination = directory.join("backup.zip");
        fs::write(&destination, b"existing").unwrap();
        assert!(publish_archive_without_replacement(&temporary, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn publication_cleanup_failure_reports_warning_without_invalidating_archive() {
        let directory = test_directory("cleanup-failure");
        let temporary = directory.join("staged.zip");
        let destination = directory.join("backup.zip");
        fs::write(&temporary, b"verified").unwrap();
        let publication = publish_archive_with_cleanup(&temporary, &destination, |_| Err(std::io::Error::other("injected cleanup failure"))).unwrap();
        assert!(publication.cleanup_warning.unwrap().contains("temporary archive could not be removed"));
        assert_eq!(fs::read(&destination).unwrap(), b"verified");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn history_update_failure_does_not_create_false_verified_status() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(update_backup_status(&connection, "BKP-1", "verified", None).is_err());
    }

    #[test]
    fn audit_failure_does_not_invalidate_verified_backup_status() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute("CREATE TABLE backup_history (id TEXT PRIMARY KEY, status TEXT NOT NULL)", []).unwrap();
        connection.execute("INSERT INTO backup_history VALUES ('BKP-1', 'verified')", []).unwrap();
        assert!(audit(&connection, None, "backup_verified", "backup", Some("BKP-1"), None).is_err());
        assert_eq!(connection.query_row("SELECT status FROM backup_history WHERE id = 'BKP-1'", [], |row| row.get::<_, String>(0)).unwrap(), "verified");
    }

    #[test]
    fn backup_status_transitions_are_explicit() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute("CREATE TABLE backup_history (id TEXT PRIMARY KEY, status TEXT NOT NULL, error_message TEXT)", []).unwrap();
        connection.execute("INSERT INTO backup_history VALUES ('BKP-1', 'creating', NULL)", []).unwrap();
        for status in ["staged", "published", "verified"] {
            update_backup_status(&connection, "BKP-1", status, None).unwrap();
            assert_eq!(connection.query_row("SELECT status FROM backup_history WHERE id = 'BKP-1'", [], |row| row.get::<_, String>(0)).unwrap(), status);
        }
    }

    #[test]
    fn managed_path_validation_rejects_unrelated_paths_and_accepts_storage_files() {
        let directory = test_directory("managed-paths");
        let root = directory.join("storage");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("document.pdf");
        fs::write(&file, b"document").unwrap();
        assert!(validate_managed_document_path(&file.to_string_lossy(), &root).is_ok());
        assert!(validate_managed_document_path(&directory.join("other/document.pdf").to_string_lossy(), &root).is_err());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rollback_restores_both_database_and_storage() {
        let directory = test_directory("rollback-success");
        let database = directory.join("database/documents.db");
        let storage = directory.join("storage");
        let displaced = directory.join("displaced");
        fs::create_dir_all(database.parent().unwrap()).unwrap();
        fs::create_dir_all(&storage).unwrap();
        fs::create_dir_all(&displaced).unwrap();
        fs::write(&database, b"new").unwrap();
        fs::write(storage.join("new.pdf"), b"new").unwrap();
        fs::write(displaced.join("documents.db"), b"original").unwrap();
        fs::create_dir_all(displaced.join("storage")).unwrap();
        fs::write(displaced.join("storage/original.pdf"), b"original").unwrap();
        rollback_installation(&database, &storage, &displaced, true, true).unwrap();
        assert_eq!(fs::read(&database).unwrap(), b"original");
        assert_eq!(fs::read(storage.join("original.pdf")).unwrap(), b"original");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rollback_failure_is_reported_when_displaced_copy_is_missing() {
        let directory = test_directory("rollback-failure");
        let database = directory.join("documents.db");
        let storage = directory.join("storage");
        let displaced = directory.join("displaced");
        fs::write(&database, b"new").unwrap();
        fs::create_dir_all(&storage).unwrap();
        fs::create_dir_all(&displaced).unwrap();
        assert!(rollback_installation(&database, &storage, &displaced, true, true).is_err());
        let _ = fs::remove_dir_all(directory);
    }
}

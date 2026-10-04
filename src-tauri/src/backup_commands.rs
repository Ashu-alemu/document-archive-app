use crate::security::{actor, audit, database_connection, database_path, require, timestamp, SessionState};
use crate::storage_root;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}};
use tauri::{AppHandle, Manager, State};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const FORMAT_VERSION: u32 = 1;

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
}

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

fn verify_archive(path: &Path) -> Result<Manifest, String> {
    let file = fs::File::open(path).map_err(|_| "Backup file could not be opened.".to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|_| "Backup archive is invalid.".to_string())?;
    let mut text = String::new();
    { let mut manifest_file = archive.by_name("manifest.json").map_err(|_| "Backup manifest is missing.".to_string())?; manifest_file.read_to_string(&mut text).map_err(|_| "Backup manifest is unreadable.".to_string())?; }
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| "Backup manifest is invalid.".to_string())?;
    if manifest.format_version != FORMAT_VERSION || !manifest.files.iter().any(|file| file.path == "database/documents.db") { return Err("Backup format is unsupported or incomplete.".into()); }
    for expected in &manifest.files {
        if expected.path.starts_with('/') || expected.path.contains("..") || expected.path.contains('\\') { return Err("Backup contains an unsafe path.".into()); }
        let mut entry = archive.by_name(&expected.path).map_err(|_| "Backup content is incomplete.".to_string())?;
        let mut hasher = Sha256::new(); let mut size = 0_u64; let mut buffer = [0_u8; 1024 * 1024];
        loop { let count = entry.read(&mut buffer).map_err(|_| "Backup content is unreadable.".to_string())?; if count == 0 { break; } hasher.update(&buffer[..count]); size += count as u64; }
        if size != expected.size || hex::encode(hasher.finalize()) != expected.sha256 { return Err("Backup verification failed: file contents do not match the manifest.".into()); }
    }
    Ok(manifest)
}

fn backup_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<BackupRecord> { Ok(BackupRecord { id: row.get(0)?, file_name: row.get(1)?, created_at: row.get(2)?, completed_at: row.get(3)?, verified_at: row.get(4)?, file_size: row.get(5)?, file_hash: row.get(6)?, status: row.get(7)? }) }

#[tauri::command]
pub fn create_backup(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<BackupRecord, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    let id = format!("BKP-{}", uuid::Uuid::new_v4()); let now = timestamp(); let file_name = format!("document-archive-{}.zip", id.to_ascii_lowercase()); let destination = safe_backup_path(&app, &file_name)?;
    connection.execute("INSERT INTO backup_history (id, file_name, created_at, status) VALUES (?, ?, ?, 'creating')", params![id, file_name, now]).map_err(|_| "Unable to record backup history.".to_string())?;
    let staging = destination.parent().ok_or_else(|| "Unable to prepare backup.".to_string())?.join(format!(".{}-staging", id)); let result: Result<(i64, String), String> = (|| {
        audit(&connection, Some(&current.id), "backup_created", "backup", Some(&id), Some("Backup creation started"))?;
        fs::create_dir_all(staging.join("database")).map_err(|_| "Unable to prepare backup.".to_string())?;
        let snapshot = staging.join("database/documents.db");
        connection.execute("VACUUM INTO ?", params![snapshot.to_string_lossy().to_string()]).map_err(|_| "Unable to snapshot the archive database.".to_string())?;
        let storage = storage_root(&app)?; let mut sources = Vec::new(); collect_files(&storage, &storage, &mut sources)?;
        let file = fs::File::create(&destination).map_err(|_| "Unable to create backup file.".to_string())?; let mut zip = ZipWriter::new(file); let mut entries = vec![write_zip_file(&mut zip, "database/documents.db", &snapshot)?];
        for (relative, source) in sources { entries.push(write_zip_file(&mut zip, &format!("storage/{relative}"), &source)?); }
        let manifest = Manifest { format_version: FORMAT_VERSION, created_at: now.clone(), files: entries }; let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| "Unable to create backup manifest.".to_string())?;
        zip.start_file("manifest.json", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(|_| "Unable to create backup manifest.".to_string())?; zip.write_all(&bytes).map_err(|_| "Unable to create backup manifest.".to_string())?; zip.finish().map_err(|_| "Unable to finalize backup file.".to_string())?;
        let (size, hash) = hash_path(&destination)?; verify_archive(&destination)?; Ok((size as i64, hash))
    })();
    let _ = fs::remove_dir_all(&staging);
    match result {
        Ok((size, hash)) => { let completed = timestamp(); connection.execute("UPDATE backup_history SET completed_at = ?, verified_at = ?, file_size = ?, file_hash = ?, status = 'verified' WHERE id = ?", params![completed, completed, size, hash, id]).map_err(|_| "Unable to update backup history.".to_string())?; audit(&connection, Some(&current.id), "backup_verified", "backup", Some(&id), None)?; connection.query_row("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history WHERE id = ?", params![id], backup_record).map_err(|_| "Unable to load backup record.".to_string()) }
        Err(error) => { let _ = fs::remove_file(&destination); let _ = connection.execute("UPDATE backup_history SET status = 'failed', error_message = ? WHERE id = ?", params![error, id]); Err("Backup creation failed.".into()) }
    }
}

#[tauri::command]
pub fn list_backups(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<BackupRecord>, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    let mut statement = connection.prepare("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history ORDER BY created_at DESC LIMIT 100").map_err(|_| "Unable to load backup history.".to_string())?;
    let rows = statement.query_map([], backup_record).map_err(|_| "Unable to load backup history.".to_string())?; rows.collect::<Result<Vec<_>, _>>().map_err(|_| "Unable to load backup history.".to_string())
}

#[tauri::command]
pub fn verify_backup(app: AppHandle, state: State<'_, SessionState>, token: String, backup_id: String) -> Result<BackupRecord, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?; let file_name: String = connection.query_row("SELECT file_name FROM backup_history WHERE id = ?", params![backup_id], |row| row.get(0)).map_err(|_| "Backup was not found.".to_string())?; let path = safe_backup_path(&app, &file_name)?;
    verify_archive(&path)?; let (size, hash) = hash_path(&path)?; let verified = timestamp(); connection.execute("UPDATE backup_history SET verified_at = ?, file_size = ?, file_hash = ?, status = 'verified', error_message = NULL WHERE id = ?", params![verified, size as i64, hash, backup_id]).map_err(|_| "Unable to update backup history.".to_string())?; audit(&connection, Some(&current.id), "backup_verified", "backup", Some(&backup_id), None)?; connection.query_row("SELECT id, file_name, created_at, completed_at, verified_at, file_size, file_hash, status FROM backup_history WHERE id = ?", params![backup_id], backup_record).map_err(|_| "Unable to load backup record.".to_string())
}

#[tauri::command]
pub fn restore_backup(app: AppHandle, state: State<'_, SessionState>, token: String, backup_id: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?; let file_name: String = connection.query_row("SELECT file_name FROM backup_history WHERE id = ?", params![backup_id], |row| row.get(0)).map_err(|_| "Backup was not found.".to_string())?; let archive_path = safe_backup_path(&app, &file_name)?; verify_archive(&archive_path)?; audit(&connection, Some(&current.id), "backup_restore_started", "backup", Some(&backup_id), None)?;
    let app_data = app.path().app_config_dir().map_err(|_| "Unable to access application storage.".to_string())?; let staging = app_data.join(format!(".restore-{backup_id}")); let _ = fs::remove_dir_all(&staging); fs::create_dir_all(&staging).map_err(|_| "Unable to prepare restore.".to_string())?;
    let result = (|| {
        let file = fs::File::open(&archive_path).map_err(|_| "Backup file could not be opened.".to_string())?; let mut archive = ZipArchive::new(file).map_err(|_| "Backup archive is invalid.".to_string())?;
        for index in 0..archive.len() { let mut entry = archive.by_index(index).map_err(|_| "Backup archive is unreadable.".to_string())?; let name = entry.name().to_string(); if name == "manifest.json" { continue; } if name.starts_with('/') || name.contains("..") || name.contains('\\') || !(name == "database/documents.db" || name.starts_with("storage/")) { return Err("Backup contains an unsafe path.".into()); } let target = staging.join(&name); if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(|_| "Unable to prepare restore.".to_string())?; } let mut output = fs::File::create(&target).map_err(|_| "Unable to prepare restore.".to_string())?; std::io::copy(&mut entry, &mut output).map_err(|_| "Unable to prepare restore.".to_string())?; }
        verify_archive(&archive_path)?;
        let db = database_path(&app)?; let storage = storage_root(&app)?; let old_db = db.with_extension(format!("db-before-{}", backup_id)); let old_storage = app_data.join(format!("storage-before-{backup_id}")); let _ = fs::remove_file(&old_db); let _ = fs::remove_dir_all(&old_storage);
        fs::rename(&storage, &old_storage).map_err(|_| "Unable to preserve current document storage.".to_string())?;
        if let Err(_) = fs::rename(staging.join("storage"), &storage) { let _ = fs::rename(&old_storage, &storage); return Err("Unable to install restored document storage.".into()); }
        if let Err(_) = fs::rename(&db, &old_db) { let _ = fs::remove_dir_all(&storage); let _ = fs::rename(&old_storage, &storage); return Err("Unable to preserve current database.".into()); }
        if let Err(_) = fs::rename(staging.join("database/documents.db"), &db) { let _ = fs::rename(&old_db, &db); let _ = fs::remove_dir_all(&storage); let _ = fs::rename(&old_storage, &storage); return Err("Unable to install restored database.".into()); }
        let _ = fs::remove_file(&old_db); let _ = fs::remove_dir_all(&old_storage); Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    match result { Ok(()) => { let restored = database_connection(&app)?; let restored_actor: Option<String> = restored.query_row("SELECT id FROM users WHERE id = ?", params![current.id], |row| row.get(0)).ok(); audit(&restored, restored_actor.as_deref(), "backup_restore_completed", "backup", Some(&backup_id), None)?; Ok(()) }, Err(error) => { audit(&connection, Some(&current.id), "backup_restore_failed", "backup", Some(&backup_id), Some("Restore failed"))?; Err(error) } }
}

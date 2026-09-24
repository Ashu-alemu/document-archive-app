use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{Migration, MigrationKind};

const DATABASE_URL: &str = "sqlite:database/documents.db";
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

const STORAGE_CATEGORIES: &[&str] = &[
    "letters",
    "receipts",
    "bills",
    "meetings",
    "reports",
    "certificates",
    "contracts",
    "payments",
    "photos",
    "other",
];

#[derive(Serialize)]
pub struct FileInspection {
    file_name: String,
    mime_type: String,
    file_size: u64,
    file_hash: String,
}

#[derive(Serialize)]
pub struct StoredFile {
    stored_file_name: String,
    file_path: String,
    file_size: u64,
    file_hash: String,
    mime_type: String,
}

fn allowed_extension(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "pdf" => Some("application/pdf"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "doc" => Some("application/msword"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "txt" => Some("text/plain"),
        _ => None,
    }
}

fn category_for(document_type: &str) -> &'static str {
    match document_type.to_ascii_lowercase().as_str() {
        value if value.contains("letter") => "letters",
        value if value.contains("receipt") => "receipts",
        value if value.contains("bill") || value.contains("invoice") => "bills",
        value if value.contains("meeting") => "meetings",
        value if value.contains("report") => "reports",
        value if value.contains("certificate") => "certificates",
        value if value.contains("contract") || value.contains("agreement") => "contracts",
        value if value.contains("payment") => "payments",
        value if value.contains("photo") || value.contains("scan") => "photos",
        _ => "other",
    }
}

fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|error| format!("Unable to read file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    let mut size = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Unable to read file: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size += count as u64;
        if size > MAX_FILE_SIZE {
            return Err("File exceeds the 100 MB size limit.".into());
        }
    }
    Ok((size, hex::encode(hasher.finalize())))
}

fn storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("storage"))
        .map_err(|error| error.to_string())
}

fn ensure_directories(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_data = app.path().app_config_dir()?;
    fs::create_dir_all(app_data.join("database"))?;
    fs::create_dir_all(app_data.join("thumbnails"))?;
    fs::create_dir_all(app_data.join("backups"))?;
    fs::create_dir_all(app_data.join("logs"))?;
    for category in STORAGE_CATEGORIES {
        fs::create_dir_all(app_data.join("storage").join(category))?;
    }
    Ok(())
}

#[tauri::command]
fn inspect_file(source_path: String) -> Result<FileInspection, String> {
    let path = PathBuf::from(&source_path);
    let mime_type = allowed_extension(&path).ok_or_else(|| {
        "Unsupported file type. Allowed: PDF, JPG, JPEG, PNG, WEBP, DOC, DOCX, TXT.".to_string()
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid file name.")?
        .to_string();
    let (file_size, file_hash) = hash_file(&path)?;
    Ok(FileInspection {
        file_name,
        mime_type: mime_type.into(),
        file_size,
        file_hash,
    })
}

#[tauri::command]
fn store_file(
    app: AppHandle,
    source_path: String,
    document_id: String,
    document_type: String,
    year: String,
    month: String,
) -> Result<StoredFile, String> {
    let source = PathBuf::from(&source_path);
    let mime_type =
        allowed_extension(&source).ok_or_else(|| "Unsupported file type.".to_string())?;
    let original_extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or("File has no extension.")?
        .to_ascii_lowercase();
    let (file_size, file_hash) = hash_file(&source)?;
    let root = storage_root(&app)?;
    let directory = root
        .join(category_for(&document_type))
        .join(&year)
        .join(&month);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Unable to create storage directory: {error}"))?;
    let stored_file_name = format!("{document_id}.{original_extension}");
    let destination = directory.join(&stored_file_name);
    if destination.exists() {
        return Err("A managed file with this identifier already exists.".into());
    }
    let temporary = destination.with_extension(format!("{original_extension}.part"));
    fs::copy(&source, &temporary).map_err(|error| format!("Unable to copy file: {error}"))?;
    if let Err(error) = fs::rename(&temporary, &destination) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Unable to finalize copied file: {error}"));
    }
    Ok(StoredFile {
        stored_file_name,
        file_path: destination.to_string_lossy().into_owned(),
        file_size,
        file_hash,
        mime_type: mime_type.into(),
    })
}

#[tauri::command]
fn remove_stored_file(app: AppHandle, file_path: String) -> Result<(), String> {
    let root = storage_root(&app)?;
    let path = PathBuf::from(&file_path);
    if !path.starts_with(&root) {
        return Err("Refusing to remove a file outside managed storage.".into());
    }
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Unable to clean up copied file: {error}"))?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = vec![Migration {
        version: 1,
        description: "create_documents",
        sql: include_str!("../migrations/0001_create_documents.sql"),
        kind: MigrationKind::Up,
    }];
    tauri::Builder::default()
        .setup(|app| {
            ensure_directories(app.handle()).map_err(|error| {
                Box::new(std::io::Error::other(error.to_string())) as Box<dyn std::error::Error>
            })?;
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(DATABASE_URL, migrations)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            inspect_file,
            store_file,
            remove_stored_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

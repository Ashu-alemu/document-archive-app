use argon2::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use rand::rngs::OsRng;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager, State};

#[derive(Default)]
pub struct SessionState {
    pub active: Mutex<Option<Session>>,
    pub login_failures: Mutex<HashMap<String, (u32, SystemTime)>>,
}

pub struct Session {
    pub token: String,
    pub user_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub token: String,
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub role_name: String,
}

#[derive(Clone)]
pub struct Actor {
    pub id: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct NativeUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub active: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
    pub role_id: String,
    pub role_name: String,
}

#[derive(Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

pub fn database_connection(app: &AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    let connection = Connection::open(path).map_err(|error| format!("Unable to open archive database: {error}"))?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;").map_err(|error| format!("Unable to configure archive database: {error}"))?;
    // Phase 6 moved application database access into Rust. Ensure the audit
    // schema exists even when the SQL plugin has not opened the database yet.
    connection.execute_batch("CREATE TABLE IF NOT EXISTS audit_logs (id TEXT PRIMARY KEY, user_id TEXT REFERENCES users(id) ON DELETE SET NULL, action TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT, details TEXT, created_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC); CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action); CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id); CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs(entity_type, entity_id); CREATE TABLE IF NOT EXISTS backup_history (id TEXT PRIMARY KEY, file_name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL, completed_at TEXT, verified_at TEXT, file_size INTEGER NOT NULL DEFAULT 0, file_hash TEXT, status TEXT NOT NULL, error_message TEXT); CREATE INDEX IF NOT EXISTS idx_backup_history_created_at ON backup_history(created_at DESC); CREATE INDEX IF NOT EXISTS idx_backup_history_status ON backup_history(status);").map_err(|error| format!("Unable to initialize audit and backup schemas: {error}"))?;
    Ok(connection)
}

pub fn database_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_config_dir().map(|path| path.join("database").join("documents.db")).map_err(|error| error.to_string())
}

pub fn timestamp() -> String { chrono_like_now() }
fn chrono_like_now() -> String {
    // Keep timestamps in the same UTC ISO-8601 shape as the existing frontend.
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or_default();
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let z = days as i64 + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", day_seconds / 3_600, (day_seconds % 3_600) / 60, day_seconds % 60)
}

pub fn audit(connection: &Connection, user_id: Option<&str>, action: &str, entity_type: &str, entity_id: Option<&str>, details: Option<&str>) -> Result<(), String> {
    connection.execute("INSERT INTO audit_logs (id, user_id, action, entity_type, entity_id, details, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)", params![format!("AUD-{}", uuid::Uuid::new_v4()), user_id, action, entity_type, entity_id, details, timestamp()]).map_err(|error| format!("Unable to write audit record: {error}"))?;
    Ok(())
}

pub fn actor(connection: &Connection, state: &State<'_, SessionState>, token: &str) -> Result<Actor, String> {
    let binding = state.active.lock().map_err(|_| "Session is unavailable.".to_string())?;
    let session = binding.as_ref().filter(|session| session.token == token).ok_or_else(|| "Authentication required.".to_string())?;
    let user_id = session.user_id.clone();
    connection.query_row("SELECT u.id, r.id FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id WHERE u.id = ? AND u.active = 1", params![user_id], |row| Ok(Actor { id: row.get(0)?, role: row.get(1)? })).map_err(|_| "Authentication required.".into())
}

pub fn require(actor: &Actor, permission: &str) -> Result<(), String> {
    let allowed = match permission {
        "view" | "open" => matches!(actor.role.as_str(), "admin" | "manager" | "contributor" | "viewer"),
        "create" | "update" => matches!(actor.role.as_str(), "admin" | "manager" | "contributor"),
        "archive" | "restore" => matches!(actor.role.as_str(), "admin" | "manager"),
        "delete" => matches!(actor.role.as_str(), "admin" | "manager"),
        "admin" | "audit" => actor.role == "admin",
        _ => false,
    };
    if allowed { Ok(()) } else { Err("You do not have permission to perform this action.".into()) }
}

fn hash_password(password: &str) -> Result<String, String> {
    if password.chars().count() < 12 { return Err("Password must be at least 12 characters.".into()); }
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt).map(|hash| hash.to_string()).map_err(|error| format!("Unable to protect password: {error}"))
}
fn verify_password(password: &str, hash: &str) -> bool { PasswordHash::new(hash).map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()).unwrap_or(false) }

fn login_key(username: &str) -> String { username.trim().to_ascii_lowercase() }

fn login_delay(state: &State<'_, SessionState>, username: &str) -> Result<(), String> {
    let previous = state.login_failures.lock().map_err(|_| "Authentication is unavailable.".to_string())?.get(&login_key(username)).copied();
    if let Some((count, last)) = previous {
        let elapsed = last.elapsed().unwrap_or_default();
        let delay = Duration::from_secs((1_u64 << count.min(3)).saturating_sub(1));
        if elapsed < delay { std::thread::sleep(delay - elapsed); }
    }
    Ok(())
}

fn record_login_failure(state: &State<'_, SessionState>, username: &str) {
    if let Ok(mut failures) = state.login_failures.lock() {
        let key = login_key(username);
        let count = failures.get(&key).map(|(count, _)| count.saturating_add(1)).unwrap_or(1);
        failures.insert(key, (count.min(8), SystemTime::now()));
    }
}

fn clear_login_failures(state: &State<'_, SessionState>, username: &str) {
    if let Ok(mut failures) = state.login_failures.lock() { failures.remove(&login_key(username)); }
}

fn user_response(connection: &Connection, token: String, id: &str) -> Result<SessionUser, String> {
    connection.query_row("SELECT u.id, u.username, u.display_name, r.id, r.name FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id WHERE u.id = ?", params![id], |row| Ok(SessionUser { token, id: row.get(0)?, username: row.get(1)?, display_name: row.get(2)?, role: row.get(3)?, role_name: row.get(4)? })).map_err(|error| format!("Unable to load session user: {error}"))
}

#[tauri::command]
pub fn auth_status(app: AppHandle) -> Result<bool, String> { let connection = database_connection(&app)?; let count: i64 = connection.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0)).map_err(|error| error.to_string())?; Ok(count > 0) }

#[tauri::command]
pub fn setup_admin(app: AppHandle, state: State<'_, SessionState>, username: String, display_name: String, password: String) -> Result<SessionUser, String> {
    let connection = database_connection(&app)?;
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0)).map_err(|error| error.to_string())?;
    if count != 0 { return Err("Administrator setup is already complete.".into()); }
    let username = username.trim(); let display_name = display_name.trim();
    if username.is_empty() || display_name.is_empty() { return Err("Username and display name are required.".into()); }
    let id = format!("USR-{}", &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()); let timestamp = timestamp(); let hash = hash_password(&password)?;
    connection.execute("INSERT INTO users (id, username, display_name, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)", params![id, username, display_name, hash, timestamp, timestamp]).map_err(|error| format!("Unable to create administrator: {error}"))?;
    connection.execute("INSERT INTO user_roles (user_id, role_id) VALUES (?, 'admin')", params![id]).map_err(|error| error.to_string())?;
    let token = uuid::Uuid::new_v4().to_string(); *state.active.lock().map_err(|_| "Session is unavailable.".to_string())? = Some(Session { token: token.clone(), user_id: id.clone() });
    audit(&connection, Some(&id), "user_created", "user", Some(&id), Some("Initial administrator account"))?;
    Ok(user_response(&connection, token, &id)?)
}

#[tauri::command]
pub fn login(app: AppHandle, state: State<'_, SessionState>, username: String, password: String) -> Result<SessionUser, String> {
    let connection = database_connection(&app)?;
    login_delay(&state, &username)?;
    let record = connection.query_row("SELECT u.id, u.password_hash FROM users u WHERE u.username = ? COLLATE NOCASE AND u.active = 1", params![username.trim()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).optional().map_err(|_| "Unable to authenticate.".to_string())?;
    let Some((id, hash)) = record else { record_login_failure(&state, &username); audit(&connection, None, "login_failure", "authentication", None, Some("invalid_credentials"))?; return Err("Invalid username or password.".into()); };
    if !verify_password(&password, &hash) { record_login_failure(&state, &username); audit(&connection, None, "login_failure", "authentication", None, Some("invalid_credentials"))?; return Err("Invalid username or password.".into()); }
    clear_login_failures(&state, &username);
    let timestamp = timestamp(); connection.execute("UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?", params![timestamp, timestamp, id]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&id), "login_success", "authentication", Some(&id), None)?;
    let token = uuid::Uuid::new_v4().to_string(); *state.active.lock().map_err(|_| "Session is unavailable.".to_string())? = Some(Session { token: token.clone(), user_id: id.clone() });
    user_response(&connection, token, &id)
}

#[tauri::command]
pub fn logout(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let actor_id = state.active.lock().map_err(|_| "Session is unavailable.".to_string())?.as_ref().filter(|session| session.token == token).map(|session| session.user_id.clone());
    if let Some(id) = actor_id.as_deref() { audit(&connection, Some(id), "logout", "authentication", Some(id), None)?; }
    *state.active.lock().map_err(|_| "Session is unavailable.".to_string())? = None; Ok(())
}

#[tauri::command]
pub fn list_users(app: AppHandle, state: State<'_, SessionState>, token: String) -> Result<Vec<NativeUser>, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    let mut statement = connection.prepare("SELECT u.id, u.username, u.display_name, u.active, u.created_at, u.updated_at, u.last_login_at, r.id, r.name FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id ORDER BY u.username").map_err(|_| "Unable to load users.".to_string())?;
    let rows = statement.query_map([], |row| Ok(NativeUser { id: row.get(0)?, username: row.get(1)?, display_name: row.get(2)?, active: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?, last_login_at: row.get(6)?, role_id: row.get(7)?, role_name: row.get(8)? })).map_err(|_| "Unable to load users.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| "Unable to load users.".to_string())
}

#[tauri::command]
pub fn create_user(app: AppHandle, state: State<'_, SessionState>, token: String, username: String, display_name: String, password: String, role: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    if !matches!(role.as_str(), "admin" | "manager" | "contributor" | "viewer") { return Err("Invalid role.".into()); }
    let id = format!("USR-{}", &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()); let timestamp = timestamp(); let hash = hash_password(&password)?;
    connection.execute("INSERT INTO users (id, username, display_name, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)", params![id, username.trim(), display_name.trim(), hash, timestamp, timestamp]).map_err(|error| format!("Unable to create user: {error}"))?;
    connection.execute("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)", params![id, role]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "user_created", "user", Some(&id), Some("Created by administrator"))
}

#[tauri::command]
pub fn update_user(app: AppHandle, state: State<'_, SessionState>, token: String, user_id: String, display_name: String, active: bool) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    if current.id == user_id && !active { return Err("You cannot deactivate the active account.".into()); }
    let previous: (String, i64) = connection.query_row("SELECT display_name, active FROM users WHERE id = ?", params![user_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|_| "Unable to update user.".to_string())?;
    connection.execute("UPDATE users SET display_name = ?, active = ?, updated_at = ? WHERE id = ?", params![display_name.trim(), if active { 1 } else { 0 }, timestamp(), user_id]).map_err(|_| "Unable to update user.".to_string())?;
    if previous.0 != display_name.trim() { audit(&connection, Some(&current.id), "user_updated", "user", Some(&user_id), Some("display_name_changed"))?; }
    if previous.1 != if active { 1 } else { 0 } { audit(&connection, Some(&current.id), if active { "user_activated" } else { "user_deactivated" }, "user", Some(&user_id), None)?; }
    Ok(())
}

#[tauri::command]
pub fn change_user_role(app: AppHandle, state: State<'_, SessionState>, token: String, user_id: String, role: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?;
    if !matches!(role.as_str(), "admin" | "manager" | "contributor" | "viewer") { return Err("Invalid role.".into()); }
    let current_role: String = connection.query_row("SELECT r.id FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ?", params![user_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if current_role == "admin" && role != "admin" { let admins: i64 = connection.query_row("SELECT COUNT(*) FROM user_roles WHERE role_id = 'admin'", [], |row| row.get(0)).map_err(|error| error.to_string())?; if admins <= 1 { return Err("The archive must retain at least one administrator.".into()); } }
    connection.execute("UPDATE user_roles SET role_id = ? WHERE user_id = ?", params![role, user_id]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "user_role_changed", "user", Some(&user_id), Some(&format!("Role changed to {role}")))
}

#[tauri::command]
pub fn delete_user(app: AppHandle, state: State<'_, SessionState>, token: String, user_id: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "admin")?; if current.id == user_id { return Err("You cannot delete the active account.".into()); }
    let role: String = connection.query_row("SELECT r.id FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ?", params![user_id], |row| row.get(0)).map_err(|error| error.to_string())?;
    if role == "admin" { let admins: i64 = connection.query_row("SELECT COUNT(*) FROM user_roles WHERE role_id = 'admin'", [], |row| row.get(0)).map_err(|error| error.to_string())?; if admins <= 1 { return Err("The archive must retain at least one administrator.".into()); } }
    connection.execute("DELETE FROM users WHERE id = ?", params![user_id]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "user_deleted", "user", Some(&user_id), None)
}

#[tauri::command]
pub fn change_password(app: AppHandle, state: State<'_, SessionState>, token: String, user_id: String, password: String) -> Result<(), String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; if current.id != user_id { require(&current, "admin")?; }
    let hash = hash_password(&password)?; connection.execute("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?", params![hash, timestamp(), user_id]).map_err(|error| error.to_string())?;
    audit(&connection, Some(&current.id), "password_changed", "user", Some(&user_id), None)
}

#[tauri::command]
pub fn get_audit_logs(app: AppHandle, state: State<'_, SessionState>, token: String, page: u32, page_size: u32, action: String, user_id: String, entity_type: String, from: String, to: String) -> Result<Vec<AuditLog>, String> {
    let connection = database_connection(&app)?; let current = actor(&connection, &state, &token)?; require(&current, "audit")?;
    let limit = page_size.clamp(1, 100) as i64; let offset = (page as i64).saturating_mul(limit); let action_filter = if action.trim().is_empty() { None } else { Some(action) }; let user_filter = if user_id.trim().is_empty() { None } else { Some(user_id) }; let entity_filter = if entity_type.trim().is_empty() { None } else { Some(entity_type) }; let from_filter = if from.trim().is_empty() { None } else { Some(format!("{}T00:00:00Z", from.trim())) }; let to_filter = if to.trim().is_empty() { None } else { Some(format!("{}T23:59:59Z", to.trim())) };
    let mut statement = connection.prepare("SELECT a.id, a.user_id, u.username, a.action, a.entity_type, a.entity_id, a.details, a.created_at FROM audit_logs a LEFT JOIN users u ON u.id = a.user_id WHERE (? IS NULL OR a.action = ?) AND (? IS NULL OR a.user_id = ?) AND (? IS NULL OR a.entity_type = ?) AND (? IS NULL OR a.created_at >= ?) AND (? IS NULL OR a.created_at <= ?) ORDER BY a.created_at DESC, a.id DESC LIMIT ? OFFSET ?").map_err(|_| "Unable to load audit logs.".to_string())?;
    let rows = statement.query_map(params![action_filter, action_filter, user_filter, user_filter, entity_filter, entity_filter, from_filter, from_filter, to_filter, to_filter, limit, offset], |row| Ok(AuditLog { id: row.get(0)?, user_id: row.get(1)?, username: row.get(2)?, action: row.get(3)?, entity_type: row.get(4)?, entity_id: row.get(5)?, details: row.get(6)?, created_at: row.get(7)? })).map_err(|_| "Unable to load audit logs.".to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{require, Actor};

    fn actor(role: &str) -> Actor { Actor { id: "test".into(), role: role.into() } }

    #[test]
    fn role_permissions_match_phase_six_matrix() {
        for role in ["admin", "manager", "contributor", "viewer"] { assert!(require(&actor(role), "view").is_ok()); assert!(require(&actor(role), "open").is_ok()); }
        for role in ["admin", "manager", "contributor"] { assert!(require(&actor(role), "create").is_ok()); assert!(require(&actor(role), "update").is_ok()); }
        for role in ["admin", "manager"] { assert!(require(&actor(role), "delete").is_ok()); }
        assert!(require(&actor("viewer"), "create").is_err()); assert!(require(&actor("contributor"), "delete").is_err()); assert!(require(&actor("manager"), "audit").is_err()); assert!(require(&actor("viewer"), "admin").is_err());
    }
}

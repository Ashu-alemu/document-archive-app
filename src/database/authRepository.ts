import { invoke } from '@tauri-apps/api/core'
import { openDatabase } from './connection'
import type { RoleId, UserRecord } from '../types/auth'

export async function hasUsers(): Promise<boolean> {
  const rows = await (await openDatabase()).select<{ count: number }[]>('SELECT COUNT(*) AS count FROM users')
  return Number(rows[0]?.count ?? 0) > 0
}

export async function findUser(username: string): Promise<UserRecord | null> {
  const rows = await (await openDatabase()).select<UserRecord[]>(`SELECT u.*, r.id AS role_id, r.name AS role_name
    FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id
    WHERE u.username = ? AND u.active = 1 LIMIT 1`, [username.trim()])
  return rows[0] ?? null
}

export async function listUsers(): Promise<UserRecord[]> {
  return (await openDatabase()).select<UserRecord[]>(`SELECT u.*, r.id AS role_id, r.name AS role_name
    FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id ORDER BY u.username`)
}

export async function createUser(username: string, displayName: string, password: string, role: RoleId): Promise<void> {
  if (!username.trim() || !displayName.trim()) throw new Error('Username and display name are required.')
  const database = await openDatabase()
  const id = `USR-${crypto.randomUUID().slice(0, 8).toUpperCase()}`
  const now = new Date().toISOString()
  const passwordHash = await invoke<string>('hash_password', { password })
  await database.execute('INSERT INTO users (id, username, display_name, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)', [id, username.trim(), displayName.trim(), passwordHash, now, now])
  await database.execute('INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)', [id, role])
}

export async function updateLastLogin(id: string): Promise<void> {
  await (await openDatabase()).execute('UPDATE users SET last_login_at = ?, updated_at = ? WHERE id = ?', [new Date().toISOString(), new Date().toISOString(), id])
}

export async function verifyUserPassword(user: UserRecord, password: string): Promise<boolean> {
  return invoke<boolean>('verify_password', { password, passwordHash: user.password_hash })
}

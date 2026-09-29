import { invoke } from '@tauri-apps/api/core'
import type { RoleId, UserRecord } from '../types/auth'

export function hasUsers(): Promise<boolean> { return invoke<boolean>('auth_status') }
export function setupAdmin(username: string, displayName: string, password: string): Promise<UserSession> { return invoke<UserSession>('setup_admin', { username, displayName, password }) }
export function login(username: string, password: string): Promise<UserSession> { return invoke<UserSession>('login', { username, password }) }
export function logout(token: string): Promise<void> { return invoke('logout', { token }) }
export function listUsers(token: string): Promise<UserRecord[]> { return invoke<UserRecord[]>('list_users', { token }) }
export function createUser(token: string, username: string, displayName: string, password: string, role: RoleId): Promise<void> { return invoke('create_user', { token, username, displayName, password, role }) }
export function updateUser(token: string, userId: string, displayName: string, active: boolean): Promise<void> { return invoke('update_user', { token, userId, displayName, active }) }
export function changeUserRole(token: string, userId: string, role: RoleId): Promise<void> { return invoke('change_user_role', { token, userId, role }) }
export function deleteUser(token: string, userId: string): Promise<void> { return invoke('delete_user', { token, userId }) }
export function changePassword(token: string, userId: string, password: string): Promise<void> { return invoke('change_password', { token, userId, password }) }

export interface UserSession { token: string; id: string; username: string; displayName: string; role: RoleId; roleName: string }

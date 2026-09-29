import { hasUsers, login, logout, setupAdmin } from '../database/authRepository'
import type { SessionUser } from '../types/auth'

export { hasUsers, logout }
export async function authenticate(username: string, password: string): Promise<SessionUser> { return login(username, password) }
export async function createInitialAdmin(username: string, displayName: string, password: string): Promise<SessionUser> { return setupAdmin(username, displayName, password) }

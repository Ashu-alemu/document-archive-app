import { findUser, hasUsers, updateLastLogin, verifyUserPassword } from '../database/authRepository'
import type { SessionUser } from '../types/auth'

export { hasUsers }

export async function authenticate(username: string, password: string): Promise<SessionUser> {
  const user = await findUser(username)
  if (!user || !(await verifyUserPassword(user, password))) throw new Error('Invalid username or password.')
  await updateLastLogin(user.id)
  return { id: user.id, username: user.username, displayName: user.display_name, role: user.role_id, roleName: user.role_name }
}

export type RoleId = 'admin' | 'manager' | 'contributor' | 'viewer'

export interface UserRecord {
  id: string
  username: string
  display_name: string
  password_hash: string
  active: number
  created_at: string
  updated_at: string
  last_login_at: string | null
  role_id: RoleId
  role_name: string
}

export interface SessionUser {
  id: string
  username: string
  displayName: string
  role: RoleId
  roleName: string
}

export const ROLE_OPTIONS: { id: RoleId; label: string; description: string }[] = [
  { id: 'admin', label: 'Administrator', description: 'Full access, including users and settings.' },
  { id: 'manager', label: 'Manager', description: 'Manage documents and view archive activity.' },
  { id: 'contributor', label: 'Contributor', description: 'Add and edit documents.' },
  { id: 'viewer', label: 'Viewer', description: 'View and search documents.' },
]

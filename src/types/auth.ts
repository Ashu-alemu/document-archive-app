export type RoleId = 'admin' | 'manager' | 'contributor' | 'viewer'

export interface UserRecord {
  id: string
  username: string
  display_name: string
  password_hash?: string
  active: number
  created_at: string
  updated_at: string
  last_login_at: string | null
  role_id: RoleId
  role_name: string
}

export interface SessionUser {
  token: string
  id: string
  username: string
  displayName: string
  role: RoleId
  roleName: string
  departmentId: string
  departmentName: string
  departmentRole: string
  departmentRoleName: string
  permissions: string[]
}

export interface AuditLogRecord { id: string; user_id: string | null; username: string | null; action: string; entity_type: string; entity_id: string | null; details: string | null; created_at: string }

export const ROLE_OPTIONS: { id: RoleId; label: string; description: string }[] = [
  { id: 'admin', label: 'Administrator', description: 'Full access, including users and settings.' },
  { id: 'manager', label: 'Manager', description: 'Manage documents and view archive activity.' },
  { id: 'contributor', label: 'Contributor', description: 'Add and edit documents.' },
  { id: 'viewer', label: 'Viewer', description: 'View and search documents.' },
]

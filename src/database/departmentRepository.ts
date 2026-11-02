import { invoke } from '@tauri-apps/api/core'
import type { Department } from '../types/department'

export interface DepartmentMember {
  id: string
  username: string
  displayName: string
  active: number
  globalRoleId: string
  globalRoleName: string
  departmentRoleId: string | null
  departmentRoleName: string | null
  member: boolean
}

export function listDepartments(): Promise<Department[]> { return invoke<Department[]>('list_departments') }
export function selectDepartment(departmentId: string): Promise<void> { return invoke('select_department', { departmentId }) }
export function listAllDepartments(token: string): Promise<Department[]> { return invoke<Department[]>('list_all_departments', { token }) }
export function listManageableDepartments(token: string): Promise<Department[]> { return invoke<Department[]>('list_manageable_departments', { token }) }
export function createDepartment(token: string, name: string, description: string, active: boolean): Promise<Department> { return invoke<Department>('create_department', { token, name, departmentDescription: description, active }) }
export function updateDepartment(token: string, departmentId: string, name: string, description: string): Promise<Department> { return invoke<Department>('update_department', { token, departmentId, name, departmentDescription: description }) }
export function setDepartmentActive(token: string, departmentId: string, active: boolean): Promise<Department> { return invoke<Department>('set_department_active', { token, departmentId, active }) }
export function listDepartmentUsers(token: string, departmentId: string): Promise<DepartmentMember[]> { return invoke<DepartmentMember[]>('list_department_users', { token, departmentId }) }
export function setDepartmentMembership(token: string, departmentId: string, userId: string, departmentRole: string, active: boolean): Promise<void> { return invoke('set_department_membership', { token, departmentId, userId, departmentRole, active }) }

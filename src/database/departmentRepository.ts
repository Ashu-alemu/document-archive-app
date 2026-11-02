import { invoke } from '@tauri-apps/api/core'
import type { Department } from '../types/department'
import type { UserRecord } from '../types/auth'

export interface DepartmentMember extends Pick<UserRecord, 'id' | 'username' | 'display_name' | 'active' | 'role_id' | 'role_name'> {}

export function listDepartments(): Promise<Department[]> { return invoke<Department[]>('list_departments') }
export function selectDepartment(departmentId: string): Promise<void> { return invoke('select_department', { departmentId }) }
export function listAllDepartments(token: string): Promise<Department[]> { return invoke<Department[]>('list_all_departments', { token }) }
export function createDepartment(token: string, name: string, description: string, active: boolean): Promise<Department> { return invoke<Department>('create_department', { token, name, departmentDescription: description, active }) }
export function updateDepartment(token: string, departmentId: string, name: string, description: string): Promise<Department> { return invoke<Department>('update_department', { token, departmentId, name, departmentDescription: description }) }
export function setDepartmentActive(token: string, departmentId: string, active: boolean): Promise<Department> { return invoke<Department>('set_department_active', { token, departmentId, active }) }
export function listDepartmentMembers(token: string, departmentId: string): Promise<DepartmentMember[]> { return invoke<DepartmentMember[]>('list_department_members', { token, departmentId }) }
export function setDepartmentMembership(token: string, departmentId: string, userId: string, active: boolean): Promise<void> { return invoke('set_department_membership', { token, departmentId, userId, active }) }

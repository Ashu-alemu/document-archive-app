import { useEffect, useState, type FormEvent } from 'react'
import { listUsers } from '../database/authRepository'
import { createDepartment, listAllDepartments, listDepartmentMembers, setDepartmentActive, setDepartmentMembership, updateDepartment, type DepartmentMember } from '../database/departmentRepository'
import type { UserRecord } from '../types/auth'
import type { Department } from '../types/department'

function message(reason: unknown, fallback: string) { return reason instanceof Error ? reason.message : typeof reason === 'string' ? reason : fallback }

export function DepartmentManagement({ token }: { token: string }) {
  const [departments, setDepartments] = useState<Department[]>([])
  const [users, setUsers] = useState<UserRecord[]>([])
  const [members, setMembers] = useState<DepartmentMember[]>([])
  const [selectedId, setSelectedId] = useState('')
  const [editing, setEditing] = useState<Department | null>(null)
  const [values, setValues] = useState({ name: '', description: '', active: true })
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')

  const refresh = async (keepSelection = true) => {
    setLoading(true); setError('')
    try {
      const [nextDepartments, nextUsers] = await Promise.all([listAllDepartments(token), listUsers(token)])
      setDepartments(nextDepartments); setUsers(nextUsers)
      if (!keepSelection || !nextDepartments.some(department => department.id === selectedId)) setSelectedId(nextDepartments[0]?.id ?? '')
    } catch (reason) { setError(message(reason, 'Unable to load department management.')) } finally { setLoading(false) }
  }

  useEffect(() => { void refresh(false) }, [token])
  useEffect(() => {
    if (!selectedId) { setMembers([]); return }
    void listDepartmentMembers(token, selectedId).then(setMembers).catch(reason => setError(message(reason, 'Unable to load department members.')))
  }, [selectedId, token])

  const submit = async (event: FormEvent) => {
    event.preventDefault(); setBusy(true); setError(''); setNotice('')
    try { await createDepartment(token, values.name, values.description, values.active); setValues({ name: '', description: '', active: true }); await refresh(); setNotice('Department created successfully.') } catch (reason) { setError(message(reason, 'Unable to create department.')) } finally { setBusy(false) }
  }
  const saveEdit = async (event: FormEvent) => {
    event.preventDefault(); if (!editing) return; setBusy(true); setError(''); setNotice('')
    try { await updateDepartment(token, editing.id, values.name, values.description); setEditing(null); await refresh(); setNotice('Department updated successfully.') } catch (reason) { setError(message(reason, 'Unable to update department.')) } finally { setBusy(false) }
  }
  const toggle = async (department: Department) => {
    setBusy(true); setError(''); setNotice('')
    try { await setDepartmentActive(token, department.id, department.active !== 1); await refresh(); setNotice(`Department ${department.active ? 'deactivated' : 'activated'} successfully.`) } catch (reason) { setError(message(reason, 'Unable to update department status.')) } finally { setBusy(false) }
  }
  const changeMembership = async (userId: string, active: boolean) => {
    if (!selectedId) return; setBusy(true); setError(''); setNotice('')
    try { await setDepartmentMembership(token, selectedId, userId, active); setMembers(await listDepartmentMembers(token, selectedId)); setNotice(active ? 'User assigned to department.' : 'User removed from department.') } catch (reason) { setError(message(reason, 'Unable to update membership.')) } finally { setBusy(false) }
  }
  const selected = departments.find(department => department.id === selectedId)
  const memberIds = new Set(members.map(member => member.id))

  if (loading) return <section className="settings-panel"><div className="empty-state loading-state" role="status">Loading department management…</div></section>
  return <section className="settings-panel department-management"><div className="section-heading"><div><span className="eyebrow">Administrator only</span><h2>Department management</h2><p className="muted">Create departments, control availability, and assign existing users to one or more departments.</p></div></div>{error && <p className="form-error" role="alert">{error}</p>}{notice && <p className="notice" role="status">✓ {notice}</p>}<form className="department-create-form" onSubmit={editing ? saveEdit : submit}><label>{editing ? 'Edit department name' : 'Department name'}<input required maxLength={80} value={values.name} onChange={event => setValues({ ...values, name: event.target.value })} placeholder="e.g. Outreach" /></label><label>Description<textarea maxLength={240} rows={2} value={values.description} onChange={event => setValues({ ...values, description: event.target.value })} placeholder="Short description for the portal card" /></label>{!editing && <label className="department-active-check"><input type="checkbox" checked={values.active} onChange={event => setValues({ ...values, active: event.target.checked })} /> Active immediately</label>}<div className="form-actions"><button className="button primary" disabled={busy}>{editing ? 'Save changes' : 'Create department'}</button>{editing && <button type="button" className="button secondary" disabled={busy} onClick={() => setEditing(null)}>Cancel</button>}</div></form><div className="department-admin-grid"><div className="catalog-panel"><div className="section-heading"><div><span className="eyebrow">Departments</span><h2>{departments.length} configured</h2></div></div><div className="department-admin-list">{departments.map(department => <div className={`department-admin-row ${selectedId === department.id ? 'selected' : ''}`} key={department.id}><button type="button" className="department-admin-select" onClick={() => setSelectedId(department.id)}><span className="department-icon" aria-hidden="true">{department.icon}</span><span><strong>{department.name}</strong><small>{department.active ? 'Active' : 'Inactive'} · {department.description || 'No description'}</small></span></button><div className="department-admin-actions"><button type="button" className="button ghost" disabled={busy} onClick={() => { setEditing(department); setValues({ name: department.name, description: department.description, active: department.active === 1 }) }}>Edit</button><button type="button" className="button archive" disabled={busy || department.id === 'administration'} onClick={() => void toggle(department)}>{department.active ? 'Deactivate' : 'Activate'}</button></div></div>)}</div></div><div className="catalog-panel membership-panel"><div className="section-heading"><div><span className="eyebrow">Memberships</span><h2>{selected?.name ?? 'Select a department'}</h2></div></div>{selected ? <><p className="muted">Select users who may enter this department. Administrators can belong to multiple departments.</p><div className="membership-list">{users.map(user => <label className="membership-row" key={user.id}><input type="checkbox" checked={memberIds.has(user.id)} disabled={busy || user.active !== 1} onChange={event => void changeMembership(user.id, event.target.checked)} /><span><strong>{user.display_name}</strong><small>{user.username} · {user.role_name}{user.active !== 1 ? ' · Inactive account' : ''}</small></span></label>)}</div>{users.length === 0 && <p className="portal-empty">No users are available.</p>}</> : <p className="portal-empty">Select a department to view and manage its members.</p>}</div></div></section>
}

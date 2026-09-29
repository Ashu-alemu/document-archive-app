import { useEffect, useState, type FormEvent, type ReactNode } from 'react'
import { createUser, listUsers } from '../database/authRepository'
import { authenticate } from '../services/authService'
import { ROLE_OPTIONS, type SessionUser } from '../types/auth'

function AuthError({ message }: { message: string }) { return message ? <p className="form-error" role="alert">{message}</p> : null }

export function SetupScreen({ onComplete }: { onComplete: (user: SessionUser) => void }) {
  const [values, setValues] = useState({ username: '', displayName: '', password: '', confirm: '' })
  const [error, setError] = useState(''); const [saving, setSaving] = useState(false)
  const submit = async (event: FormEvent) => { event.preventDefault(); setError(''); if (values.password !== values.confirm) { setError('Passwords do not match.'); return } if (values.password.length < 12) { setError('Password must be at least 12 characters.'); return } setSaving(true); try { await createUser(values.username, values.displayName, values.password, 'admin'); onComplete(await authenticate(values.username, values.password)) } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to create administrator.') } finally { setSaving(false) } }
  return <AuthLayout eyebrow="First-run setup" title="Create your administrator account" subtitle="This account controls local access to the archive. Your password is protected locally and is never sent anywhere."><form className="auth-form" onSubmit={submit}><label>Username<input required autoFocus value={values.username} onChange={e => setValues({ ...values, username: e.target.value })} /></label><label>Display name<input required value={values.displayName} onChange={e => setValues({ ...values, displayName: e.target.value })} /></label><label>Password<input required type="password" minLength={12} value={values.password} onChange={e => setValues({ ...values, password: e.target.value })} /><small>Use at least 12 characters.</small></label><label>Confirm password<input required type="password" value={values.confirm} onChange={e => setValues({ ...values, confirm: e.target.value })} /></label><AuthError message={error} /><button className="button primary" disabled={saving}>{saving ? 'Creating…' : 'Create administrator'}</button></form></AuthLayout>
}

export function LoginScreen({ onLogin }: { onLogin: (user: SessionUser) => void }) {
  const [username, setUsername] = useState(''); const [password, setPassword] = useState(''); const [error, setError] = useState(''); const [busy, setBusy] = useState(false)
  const submit = async (event: FormEvent) => { event.preventDefault(); setError(''); setBusy(true); try { onLogin(await authenticate(username, password)) } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to sign in.') } finally { setBusy(false) } }
  return <AuthLayout eyebrow="Local archive" title="Sign in" subtitle="Use your local archive account to continue."><form className="auth-form" onSubmit={submit}><label>Username<input required autoFocus value={username} onChange={e => setUsername(e.target.value)} /></label><label>Password<input required type="password" value={password} onChange={e => setPassword(e.target.value)} /></label><AuthError message={error} /><button className="button primary" disabled={busy}>{busy ? 'Signing in…' : 'Sign in'}</button></form></AuthLayout>
}

function AuthLayout({ eyebrow, title, subtitle, children }: { eyebrow: string; title: string; subtitle: string; children: ReactNode }) { return <main className="auth-shell"><section className="auth-card"><div className="brand-mark">DA</div><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{subtitle}</p>{children}</section></main> }

export function UserManagement() {
  const [users, setUsers] = useState<Awaited<ReturnType<typeof import('../database/authRepository').listUsers>>>([])
  const [loaded, setLoaded] = useState(false); const [error, setError] = useState(''); const [values, setValues] = useState({ username: '', displayName: '', password: '', role: 'viewer' as const })
  useEffect(() => { void listUsers().then(items => { setUsers(items); setLoaded(true) }) }, [loaded])
  const submit = async (event: FormEvent) => { event.preventDefault(); setError(''); try { await createUser(values.username, values.displayName, values.password, values.role); setValues({ username: '', displayName: '', password: '', role: 'viewer' }); setLoaded(false) } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to create user.') } }
  return <section className="settings-panel"><div className="section-heading"><div><span className="eyebrow">Access control</span><h2>Users and roles</h2></div></div><form className="user-form" onSubmit={submit}><input required placeholder="Username" value={values.username} onChange={e => setValues({ ...values, username: e.target.value })} /><input required placeholder="Display name" value={values.displayName} onChange={e => setValues({ ...values, displayName: e.target.value })} /><input required minLength={12} type="password" placeholder="Temporary password (12+ characters)" value={values.password} onChange={e => setValues({ ...values, password: e.target.value })} /><select value={values.role} onChange={e => setValues({ ...values, role: e.target.value as typeof values.role })}>{ROLE_OPTIONS.map(role => <option key={role.id} value={role.id}>{role.label}</option>)}</select><button className="button primary">Add user</button></form><AuthError message={error} /><div className="user-list">{users.map(user => <div className="user-row" key={user.id}><div><strong>{user.display_name}</strong><small>{user.username}</small></div><span className="status-pill">{user.role_name}</span></div>)}</div></section>
}

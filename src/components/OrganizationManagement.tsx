import { useCallback, useEffect, useState, type FormEvent } from 'react'
import { createCategory, createTag, deleteCategory, deleteTag, listCategories, listTags, renameCategory, renameTag } from '../database/documentRepository'
import type { ManagedValue } from '../types/document'

interface EditorProps {
  title: string
  values: ManagedValue[]
  onCreate: (name: string) => Promise<void>
  onRename: (oldName: string, newName: string) => Promise<void>
  onDelete: (name: string) => Promise<void>
}

function CatalogEditor({ title, values, onCreate, onRename, onDelete }: EditorProps) {
  const singular = title === 'Categories' ? 'category' : 'tag'
  const [newName, setNewName] = useState('')
  const [editing, setEditing] = useState('')
  const [editedName, setEditedName] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  const run = async (action: () => Promise<void>) => {
    setError('')
    setBusy(true)
    try { await action() } catch (reason) { setError(reason instanceof Error ? reason.message : `Unable to update ${title.toLowerCase()}.`) } finally { setBusy(false) }
  }
  const submitNew = (event: FormEvent) => {
    event.preventDefault()
    void run(async () => { await onCreate(newName); setNewName('') })
  }
  const submitRename = (oldName: string) => {
    void run(async () => { await onRename(oldName, editedName); setEditing('') })
  }

  return <section className="catalog-panel">
    <div className="section-heading"><div><span className="eyebrow">Archive organization</span><h2>{title}</h2></div></div>
    <form className="catalog-create" onSubmit={submitNew}><label className="sr-only" htmlFor={`${title}-new`}>New {singular}</label><input id={`${title}-new`} value={newName} onChange={(event) => setNewName(event.target.value)} maxLength={80} placeholder={`Add a ${singular}`} required /><button className="button secondary" disabled={busy}>Add</button></form>
    {error && <p className="form-error" role="alert">{error}</p>}
    <div className="catalog-list">{values.map(({ name }) => <div className="catalog-row" key={name}>{editing === name ? <><input aria-label={`Rename ${name}`} value={editedName} onChange={(event) => setEditedName(event.target.value)} maxLength={80} autoFocus /><button className="button secondary" onClick={() => submitRename(name)} disabled={busy || !editedName.trim()}>Save</button><button className="button ghost" onClick={() => setEditing('')}>Cancel</button></> : <><strong>{name}</strong><div><button className="button ghost" onClick={() => { setEditing(name); setEditedName(name); setError('') }}>Rename</button><button className="button archive" onClick={() => void run(() => onDelete(name))} disabled={busy}>Delete</button></div></>}</div>)}</div>
  </section>
}

interface Props { token: string; onChanged: () => void }
export function OrganizationManagement({ token, onChanged }: Props) {
  const [categories, setCategories] = useState<ManagedValue[]>([])
  const [tags, setTags] = useState<ManagedValue[]>([])
  const [error, setError] = useState('')

  const reload = useCallback(async () => {
    try {
      const [nextCategories, nextTags] = await Promise.all([listCategories(token), listTags(token)])
      setCategories(nextCategories)
      setTags(nextTags)
      setError('')
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to load archive categories and tags.')
    }
  }, [token])
  useEffect(() => { void reload() }, [reload]) // oxlint-disable-line react(set-state-in-effect)
  const changed = async () => { await reload(); onChanged() }

  return <section className="organization-management">
    <p className="muted">Categories organize document types. Renaming one updates its existing documents; a category in use must be reassigned before it can be deleted. Tag changes are applied to document metadata.</p>
    {error && <p className="form-error" role="alert">{error}</p>}
    <CatalogEditor title="Categories" values={categories} onCreate={async (name) => { await createCategory(token, name); await changed() }} onRename={async (oldName, newName) => { await renameCategory(token, oldName, newName); await changed() }} onDelete={async (name) => { await deleteCategory(token, name); await changed() }} />
    <CatalogEditor title="Tags" values={tags} onCreate={async (name) => { await createTag(token, name); await changed() }} onRename={async (oldName, newName) => { await renameTag(token, oldName, newName); await changed() }} onDelete={async (name) => { await deleteTag(token, name); await changed() }} />
  </section>
}

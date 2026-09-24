import { DOCUMENT_TYPES } from '../types/document'

interface SearchBarProps { search: string; type: string; from: string; to: string; onChange: (values: { search: string; type: string; from: string; to: string }) => void }

export function SearchBar({ search, type, from, to, onChange }: SearchBarProps) {
  return <section className="filters" aria-label="Search and filters">
    <label className="search-box"><span aria-hidden="true">⌕</span><input value={search} onChange={(event) => onChange({ search: event.target.value, type, from, to })} placeholder="Search title, reference, organization, tags, or filename" /><kbd>⌘ K</kbd></label>
    <div className="filter-row"><label>Type<select value={type} onChange={(event) => onChange({ search, type: event.target.value, from, to })}><option value="">All types</option>{DOCUMENT_TYPES.map((item) => <option key={item}>{item}</option>)}</select></label><label>From<input type="date" value={from} onChange={(event) => onChange({ search, type, from: event.target.value, to })} /></label><label>To<input type="date" value={to} onChange={(event) => onChange({ search, type, from, to: event.target.value })} /></label></div>
  </section>
}

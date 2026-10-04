import { useEffect, useRef, useState } from 'react'
import type { DocumentSearchFilters } from '../types/document'

interface SearchBarProps {
  filters: DocumentSearchFilters
  categories: string[]
  tags: string[]
  onChange: (filters: DocumentSearchFilters) => void
}

const EMPTY_FILTERS: DocumentSearchFilters = {
  search: '', searchMode: 'all', type: '', from: '', to: '', organization: '', tag: '',
  favoritesOnly: false, sortBy: 'date', sortDirection: 'desc',
}

export function SearchBar({ filters, categories, tags, onChange }: SearchBarProps) {
  const [draftSearch, setDraftSearch] = useState(filters.search)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (draftSearch === filters.search) return
    const timer = window.setTimeout(() => onChange({ ...filters, search: draftSearch }), 300)
    return () => window.clearTimeout(timer)
  }, [draftSearch, filters, onChange])
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        inputRef.current?.focus()
      }
    }
    window.addEventListener('keydown', focusSearch)
    return () => window.removeEventListener('keydown', focusSearch)
  }, [])

  const update = <K extends keyof DocumentSearchFilters>(key: K, value: DocumentSearchFilters[K]) => onChange({ ...filters, [key]: value })

  return <section className="filters" aria-label="Search and filters">
    <label className="search-box"><span aria-hidden="true">⌕</span><input ref={inputRef} value={draftSearch} onChange={(event) => setDraftSearch(event.target.value)} placeholder="Search title, reference, organization, tags, filename, or OCR text" aria-label="Search archived documents" /><kbd>Ctrl/⌘ K</kbd></label>
    <div className="filter-row query-builder">
      <label>Match<select value={filters.searchMode} onChange={(event) => update('searchMode', event.target.value as DocumentSearchFilters['searchMode'])}><option value="all">All search words</option><option value="any">Any search word</option></select></label>
      <label>Category<select value={filters.type} onChange={(event) => update('type', event.target.value)}><option value="">All categories</option>{categories.map((category) => <option key={category}>{category}</option>)}</select></label>
      <label>Tag<select value={filters.tag} onChange={(event) => update('tag', event.target.value)}><option value="">All tags</option>{tags.map((tag) => <option key={tag}>{tag}</option>)}</select></label>
      <label>Organization<input value={filters.organization} onChange={(event) => update('organization', event.target.value)} placeholder="Any organization" /></label>
    </div>
    <div className="filter-row query-builder">
      <label>From<input type="date" value={filters.from} onChange={(event) => update('from', event.target.value)} /></label>
      <label>To<input type="date" value={filters.to} onChange={(event) => update('to', event.target.value)} /></label>
      <label>Sort by<select value={filters.sortBy} onChange={(event) => update('sortBy', event.target.value as DocumentSearchFilters['sortBy'])}><option value="date">Document date</option><option value="created">Date added</option><option value="updated">Recently updated</option><option value="title">Title</option><option value="category">Category</option><option value="size">File size</option></select></label>
      <label>Order<select value={filters.sortDirection} onChange={(event) => update('sortDirection', event.target.value as DocumentSearchFilters['sortDirection'])}><option value="desc">Descending</option><option value="asc">Ascending</option></select></label>
    </div>
    <div className="filter-actions"><label className="favorite-filter"><input type="checkbox" checked={filters.favoritesOnly} onChange={(event) => update('favoritesOnly', event.target.checked)} /> Show favorites only</label><button type="button" className="button ghost" onClick={() => { setDraftSearch(''); onChange(EMPTY_FILTERS) }}>Clear filters</button></div>
  </section>
}

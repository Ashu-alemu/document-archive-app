import Database from '@tauri-apps/plugin-sql'

const DATABASE_URL = 'sqlite:database/documents.db'

/** Opens the future metadata database in Tauri's application-data directory. */
export async function openDatabase() {
  return Database.load(DATABASE_URL)
}

/** Used by the foundation screen; no tables or schema are created in Phase 2. */
export async function testDatabaseConnection(): Promise<'ready' | 'preview'> {
  if (!('__TAURI_INTERNALS__' in window)) return 'preview'
  try {
    const db = await openDatabase()
    await db.close()
    return 'ready'
  } catch (error) {
    console.error('SQLite initialization failed', error)
    return 'preview'
  }
}

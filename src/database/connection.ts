import Database from '@tauri-apps/plugin-sql'

export const DATABASE_URL = 'sqlite:database/documents.db'
let databasePromise: Promise<Database> | undefined

/** Opens the migrated metadata database in Tauri's application-data directory. */
export function openDatabase(): Promise<Database> {
  databasePromise ??= Database.load(DATABASE_URL)
  return databasePromise
}

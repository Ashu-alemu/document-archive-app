import { invoke } from '@tauri-apps/api/core'

export interface BackupRecord { id: string; fileName: string; createdAt: string; completedAt: string | null; verifiedAt: string | null; fileSize: number; fileHash: string | null; status: string; formatVersion: number | null; applicationVersion: string | null; migrationVersion: number | null; schemaVersion: string | null; compatibility: string; compatibilityReason: string | null }

export function listBackups(token: string): Promise<BackupRecord[]> { return invoke<BackupRecord[]>('list_backups', { token }) }
export function createBackup(token: string): Promise<BackupRecord> { return invoke<BackupRecord>('create_backup', { token }) }
export function verifyBackup(token: string, backupId: string): Promise<BackupRecord> { return invoke<BackupRecord>('verify_backup', { token, backupId }) }
export function restoreBackup(token: string, backupId: string): Promise<void> { return invoke('restore_backup', { token, backupId }) }

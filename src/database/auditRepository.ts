import { invoke } from '@tauri-apps/api/core'
import type { AuditLogRecord } from '../types/auth'

export function findAuditLogs(token: string, page: number, action: string, userId: string, entityType: string, from: string, to: string): Promise<AuditLogRecord[]> { return invoke<AuditLogRecord[]>('get_audit_logs', { token, page, pageSize: 25, action, userId, entityType, from, to }) }

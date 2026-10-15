export type NotificationSeverity = 'info' | 'warning' | 'error'

export interface AdminNotification {
  id: string
  action: string
  title: string
  message: string
  entityType: string | null
  entityId: string | null
  actorId: string | null
  severity: NotificationSeverity
  createdAt: string
  readAt: string | null
}

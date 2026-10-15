import { invoke } from '@tauri-apps/api/core'
import type { AdminNotification } from '../types/notifications'

export function listNotifications(token: string, limit = 100): Promise<AdminNotification[]> { return invoke<AdminNotification[]>('list_notifications', { token, limit }) }
export function countUnreadNotifications(token: string): Promise<number> { return invoke<number>('count_unread_notifications', { token }) }
export function markNotificationRead(token: string, notificationId: string): Promise<void> { return invoke('mark_notification_read', { token, notificationId }) }
export function markAllNotificationsRead(token: string): Promise<void> { return invoke('mark_all_notifications_read', { token }) }

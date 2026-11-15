import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { VaultRecord } from './model';

export interface VaultStatus {
  exists: boolean;
  unlocked: boolean;
  session: string | null;
}

export interface TotpCode {
  code: string;
  remaining: number;
  period: number;
}

export interface ImportPreview {
  columns: string[];
  sampleRows: string[][];
  rows: { index: number; title: string; status: 'valid' | 'duplicate' | 'error'; message: string }[];
  validCount: number;
  errorCount: number;
  duplicateCount: number;
}

export interface ImportFile {
  cancelled: boolean;
  importId?: string;
  fileName?: string;
  sheets?: string[];
}

export interface Results {
  status: VaultStatus;
  create: VaultStatus;
  unlock: VaultStatus;
  lock: null;
  activity: null;
  list: VaultRecord[];
  detail: VaultRecord;
  save: VaultRecord;
  delete: null;
  reveal: string;
  copy: null;
  totp: TotpCode;
  change_password: VaultStatus;
  export_backup: { cancelled: boolean };
  inspect_backup: { cancelled: boolean; backupId?: string };
  restore_preview: { count: number };
  restore_backup: VaultStatus;
  import_open: ImportFile;
  import_preview: ImportPreview;
  import_commit: { imported: number; skipped: number };
  import_cancel: null;
}

export type Operation = keyof Results;
export type CommandArgs = Record<string, unknown>;
export type Run = <T extends Operation>(op: T, args?: CommandArgs) => Promise<Results[T]>;

export function commands<T extends Operation>(op: T, args: CommandArgs = {}, session?: string): Promise<Results[T]> {
  return invoke<Results[T]>('rpc', { request: { op, ...args, session } });
}

export const desktopAvailable = isTauri;
export const subscribeToLock = (handler: () => void) => listen('vault-locked', handler);

export class StaleRequest extends Error {}

export function errorMessage(error: unknown): string {
  if (error instanceof StaleRequest) return '';
  return typeof error === 'string' ? error : '操作未完成，请重试。';
}

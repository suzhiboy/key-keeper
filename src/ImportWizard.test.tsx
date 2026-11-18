import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ImportWizard } from './ImportWizard';
import type { Run } from './rpc';

afterEach(cleanup);

it('reuses file columns across types and pages through a whole batch before committing', async () => {
  const rows = Array.from({ length: 51 }, (_, i) => ({
    index: i + 1, title: `记录-${i + 1}`, status: 'valid', message: '',
  }));
  const run = vi.fn(async (op: string, args?: Record<string, unknown>) => {
    if (op === 'import_open') return { cancelled: false, importId: 'batch', fileName: 'cards.txt', sheets: [] };
    if (op === 'import_preview') return {
      columns: ['第 1 列'], sampleRows: [], rows: args?.mapping && Object.keys(args.mapping).length ? rows : [],
      validCount: rows.length, errorCount: 0, duplicateCount: 0,
    };
    if (op === 'import_commit') return { imported: rows.length, skipped: 0 };
    throw new Error(op);
  });
  const notify = vi.fn();
  render(<ImportWizard run={run as Run} close={() => {}} notify={notify} imported={() => {}} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择文件' }));
  await user.click(screen.getByRole('button', { name: '读取文件列' }));
  await user.selectOptions(screen.getByLabelText('记录类型'), 'card');
  expect((screen.getByLabelText('兑换码或密码') as HTMLSelectElement).value).toBe('0');
  await user.click(screen.getByRole('button', { name: '预览导入' }));
  expect(screen.getByText('记录-1')).toBeTruthy();
  expect(screen.queryByText('记录-51')).toBeNull();
  await user.click(screen.getByRole('button', { name: '下一页' }));
  expect(screen.getByText('记录-51')).toBeTruthy();
  expect(screen.queryByText('记录-1')).toBeNull();
  await user.click(screen.getByRole('button', { name: '确认导入' }));
  expect(notify).toHaveBeenCalledWith('已导入 51 条记录');
  expect(run.mock.calls.filter(([op]) => op === 'import_preview')).toHaveLength(2);
});

it('replacing a selected file uses the new native import session', async () => {
  let count = 0;
  const run = vi.fn(async (op: string) => {
    if (op === 'import_open') return { cancelled: false, importId: `file-${++count}`, fileName: `file-${count}.txt`, sheets: [] };
    if (op === 'import_cancel') throw new Error('old session has already been replaced');
    throw new Error(op);
  });
  render(<ImportWizard run={run as Run} close={() => {}} notify={() => {}} imported={() => {}} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', {name: '选择文件'}));
  await screen.findByText('file-1.txt');
  await user.click(screen.getByRole('button', {name: '更换文件'}));
  expect(await screen.findByText('file-2.txt')).toBeTruthy();
  expect(run).not.toHaveBeenCalledWith('import_cancel', expect.anything());
});

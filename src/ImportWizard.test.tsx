import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ImportWizard } from './ImportWizard';
import type { Run } from './rpc';

afterEach(cleanup);

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

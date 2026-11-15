import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './App';
import { commands, subscribeToLock } from './rpc';
import type { VaultRecord } from './model';

vi.mock('./rpc', async (importOriginal) => ({
  ...await importOriginal<typeof import('./rpc')>(),
  desktopAvailable: () => true,
  commands: vi.fn(),
  subscribeToLock: vi.fn(),
}));

const session = { exists: true, unlocked: true, session: 'test-session' };
const partial: VaultRecord = {
  id: 'one', kind: 'login', title: '工作帐号', tags: [], notes: '',
  fields: { platform: '工作帐号', phone: '00123' }, used: false, secretFields: ['password'],
  createdAt: '', updatedAt: '',
};

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe('vault UI', () => {
  it('persists a manually selected theme without persisting records', async () => {
    localStorage.clear();
    vi.mocked(subscribeToLock).mockResolvedValue(() => {});
    vi.mocked(commands).mockResolvedValue({exists:false,unlocked:false,session:null});
    render(<App />);
    await userEvent.setup().click(screen.getByRole('button', {name:'切换到深色主题'}));
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(localStorage.getItem('key-keeper-theme')).toBe('dark');
    expect(localStorage.length).toBe(1);
    localStorage.clear();
  });
  it('saves a partially filled login and preserves unrelated fields and hidden secrets', async () => {
    vi.mocked(subscribeToLock).mockResolvedValue(() => {});
    vi.mocked(commands).mockImplementation(async (op) => {
      if (op === 'status') return session;
      if (op === 'list') return [partial];
      if (op === 'detail' || op === 'save') return partial;
      throw new Error(op);
    });
    render(<App />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: /工作帐号/ }));
    await user.click(await screen.findByRole('button', { name: '编辑' }));
    await user.type(screen.getByLabelText('邮箱'), '仍在确认');
    await user.click(screen.getByRole('button', { name: '保存记录' }));
    expect(commands).toHaveBeenCalledWith('save', {
      record: {
        id: 'one', kind: 'login', title: '工作帐号', notes: '', tags: [],
        fields: { platform: '工作帐号', phone: '00123', email: '仍在确认' },
        used: false, createdAt: '', updatedAt: '',
      },
    }, session.session);
    expect(vi.mocked(commands).mock.calls.some(([op]) => op === 'reveal')).toBe(false);
  });

  it('discards a reveal result that arrives after the native lock event', async () => {
    let lock = () => {};
    let resolveReveal: (value: string) => void = () => {};
    vi.mocked(subscribeToLock).mockImplementation(async (handler) => {
      lock = handler;
      return () => {};
    });
    vi.mocked(commands).mockImplementation(async (op) => {
      if (op === 'status') return session;
      if (op === 'list') return [partial];
      if (op === 'detail') return partial;
      if (op === 'activity') return null;
      if (op === 'reveal') return new Promise((resolve) => { resolveReveal = resolve; });
      throw new Error(op);
    });
    render(<App />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: /工作帐号/ }));
    await user.click(await screen.findByRole('button', { name: '显示密码' }));
    await act(async () => lock());
    await act(async () => resolveReveal('LATE-SECRET'));
    expect(await screen.findByRole('heading', { name: '解锁保险库' })).toBeTruthy();
    expect(screen.queryByText('LATE-SECRET')).toBeNull();
    expect(screen.queryByRole('button', { name: /工作帐号/ })).toBeNull();
  });
});

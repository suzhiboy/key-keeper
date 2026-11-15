import { useState, type FormEvent } from 'react';
import { Download, FileKey2, HardDrive, KeyRound } from 'lucide-react';
import { errorMessage, type Run } from './rpc';
import { Modal, Notice } from './ui';

type DialogProps = { run: Run; close: () => void; notify: (message: string) => void };

export function Restore({ run, close, notify }: DialogProps) {
  const [backupId, setBackupId] = useState('');
  const [password, setPassword] = useState('');
  const [count, setCount] = useState<number>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const dismiss = () => { if (!busy) close(); };

  async function choose() {
    setBusy(true);
    setError('');
    try {
      const file = await run('inspect_backup');
      if (!file.cancelled && file.backupId) {
        setBackupId(file.backupId);
        setPassword('');
        setCount(undefined);
      }
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }

  async function validate(event: FormEvent) {
    event.preventDefault();
    if (busy || !password) return;
    setBusy(true);
    setError('');
    try {
      const result = await run('restore_preview', { backupId, password });
      setCount(result.count);
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setPassword(''); setBusy(false); }
  }

  async function restore() {
    if (busy || count === undefined) return;
    setBusy(true);
    setError('');
    try {
      await run('restore_backup', { backupId });
      close();
      notify('已从加密备份恢复。请使用备份的主密码解锁。');
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }

  return <Modal title="从加密备份恢复" close={dismiss}>
    {!backupId && <>
      <p>选择一个 Key Keeper 加密备份，输入创建该备份时使用的主密码。</p>
      <Notice>{error}</Notice>
      <div className="action-row">
        <button className="button secondary" onClick={dismiss} disabled={busy}>取消</button>
        <button className="button primary" onClick={() => void choose()} disabled={busy}>
          <HardDrive size={16} />{busy ? '正在打开…' : '选择备份文件'}
        </button>
      </div>
    </>}
    {backupId && count === undefined && <form onSubmit={validate}>
      <label className="field">备份的主密码
        <input type="password" autoComplete="current-password" autoFocus value={password}
          onChange={(event) => setPassword(event.target.value)} disabled={busy} />
      </label>
      <Notice>{error}</Notice>
      <div className="action-row">
        <button type="button" className="button secondary" onClick={() => void choose()} disabled={busy}>重新选择文件</button>
        <button className="button primary" disabled={busy || !password}>{busy ? '正在验证…' : '验证备份'}</button>
      </div>
    </form>}
    {count !== undefined && <>
      <p>备份已验证，内含 <strong>{count}</strong> 条记录。</p>
      <p>确认后，此备份将替换当前保险库（如有）。恢复后请使用备份的主密码解锁。</p>
      <Notice>{error}</Notice>
      <div className="action-row">
        <button className="button secondary" onClick={dismiss} disabled={busy}>取消</button>
        <button className="button danger" onClick={() => void restore()} disabled={busy}>
          {busy ? '正在恢复…' : '确认恢复并替换'}
        </button>
      </div>
    </>}
  </Modal>;
}

export function Settings({ run, close, notify }: DialogProps) {
  const [view, setView] = useState<'settings' | 'password' | 'restore'>('settings');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [previous, setPrevious] = useState('');
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const dismiss = () => { if (!busy) close(); };

  async function exportBackup() {
    setBusy(true);
    setError('');
    try {
      const result = await run('export_backup');
      if (!result.cancelled) notify('已导出加密备份');
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }

  async function changePassword(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    if (!previous) return setError('请输入当前主密码。');
    if ([...password].length < 12) return setError('新主密码至少需要 12 个字符。');
    if (password !== confirmation) return setError('两次输入的新主密码不一致。');
    setError('');
    setBusy(true);
    try {
      await run('change_password', { oldPassword: previous, newPassword: password });
      close();
      notify('主密码已更改');
    } catch (reason) { setError(errorMessage(reason)); }
    finally {
      setPrevious(''); setPassword(''); setConfirmation(''); setBusy(false);
    }
  }

  if (view === 'restore') return <Restore run={run} close={close} notify={notify} />;

  return <Modal key={view} title={view === 'settings' ? '设置' : '更改主密码'} close={dismiss}>
    {view === 'settings'
      ? <div className="settings-list">
          <button className="settings-row" disabled={busy} onClick={() => { setError(''); setView('password'); }}>
            <KeyRound size={19} /><span><strong>更改主密码</strong><small>保护此设备上的保险库</small></span>
          </button>
          <button className="settings-row" disabled={busy} onClick={() => void exportBackup()}>
            <Download size={19} /><span><strong>{busy ? '正在导出…' : '导出加密备份'}</strong><small>将加密副本保存到文件</small></span>
          </button>
          <button className="settings-row" disabled={busy} onClick={() => setView('restore')}>
            <FileKey2 size={19} /><span><strong>从加密备份恢复</strong><small>用备份替换当前保险库</small></span>
          </button>
          <Notice>{error}</Notice>
        </div>
      : <form onSubmit={changePassword}>
          <label className="field">当前主密码
            <input type="password" autoComplete="current-password" autoFocus value={previous}
              disabled={busy} onChange={(event) => setPrevious(event.target.value)} />
          </label>
          <label className="field">新主密码
            <input type="password" autoComplete="new-password" value={password} disabled={busy}
              onChange={(event) => setPassword(event.target.value)} />
          </label>
          <label className="field">确认新主密码
            <input type="password" autoComplete="new-password" value={confirmation} disabled={busy}
              onChange={(event) => setConfirmation(event.target.value)} />
          </label>
          <p className="hint">旧备份仍需使用创建备份时的主密码。</p>
          <Notice>{error}</Notice>
          <div className="action-row">
            <button type="button" className="button secondary" onClick={dismiss} disabled={busy}>取消</button>
            <button className="button primary" disabled={busy}>{busy ? '正在更改…' : '保存新主密码'}</button>
          </div>
        </form>}
  </Modal>;
}

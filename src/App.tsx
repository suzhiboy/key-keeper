import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { ArrowRight, HardDrive, LockKeyhole, ShieldCheck } from 'lucide-react';
import { commands, desktopAvailable, errorMessage, StaleRequest, subscribeToLock, type CommandArgs, type Operation, type Run, type VaultStatus } from './rpc';
import { Brand, Notice, ThemeButton, Toast } from './ui';
import { Restore } from './dialogs';
import { Vault } from './Vault';

interface Scope {
  epoch: number;
  status: VaultStatus;
}

const initialStatus: VaultStatus = { exists: false, unlocked: false, session: null };
const statusOperations: Operation[] = ['create', 'unlock', 'change_password', 'restore_backup'];

export function App() {
  const native = desktopAvailable();
  const current = useRef<Scope>({ epoch: 0, status: initialStatus });
  const transitions = useRef(0);
  const [state, setState] = useState(current.current);
  const [ready, setReady] = useState(!native);
  const [startupError, setStartupError] = useState('');
  const [dark, setDark] = useState(() => localStorage.getItem('key-keeper-theme') === 'dark');
  const [toast, setToast] = useState('');
  const toastTimer = useRef<number | undefined>(undefined);
  const notify = useCallback((message: string) => {
    window.clearTimeout(toastTimer.current);
    setToast(message);
    toastTimer.current = window.setTimeout(() => setToast(''), 3000);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
    localStorage.setItem('key-keeper-theme', dark ? 'dark' : 'light');
  }, [dark]);
  useEffect(() => () => window.clearTimeout(toastTimer.current), []);

  const updateStatus = useCallback((status: VaultStatus, invalidate = false) => {
    const previous = current.current;
    const epoch = previous.epoch + Number(invalidate || previous.status.session !== status.session);
    const next = { epoch, status };
    current.current = next;
    if (epoch !== previous.epoch) {
      window.clearTimeout(toastTimer.current);
      setToast('');
    }
    setState(next);
  }, []);

  const run: Run = useCallback(async <T extends Operation>(op: T, args?: CommandArgs) => {
    const before = current.current;
    const transition = statusOperations.includes(op) || op === 'lock';
    if (transition) transitions.current += 1;
    try {
      const value = await commands(op, args, before.status.session ?? undefined);
      if (before.epoch !== current.current.epoch) throw new StaleRequest();
      if (statusOperations.includes(op)) updateStatus(value as VaultStatus);
      return value;
    } catch (error) {
      if (before.epoch !== current.current.epoch) throw new StaleRequest();
      throw error;
    } finally {
      if (transition) transitions.current -= 1;
    }
  }, [updateStatus]);

  const lock = useCallback(async () => {
    updateStatus({ ...current.current.status, unlocked: false, session: null }, true);
    try { await run('lock'); }
    catch (error) { if (errorMessage(error)) notify(errorMessage(error)); }
  }, [notify, run, updateStatus]);

  useEffect(() => {
    if (!native) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    let initialized = false;
    let pending = false;
    const poll = async () => {
      if (pending || transitions.current > 0) return;
      pending = true;
      const before = current.current;
      try {
        const status = await commands('status');
        if (!active || before.epoch !== current.current.epoch || transitions.current > 0) return;
        setStartupError('');
        if (!initialized) {
          initialized = true;
          updateStatus(status);
        } else if (before.status.session && status.session !== before.status.session) {
          updateStatus({ ...status, unlocked: false, session: null }, true);
        } else if (status.exists !== before.status.exists) {
          updateStatus({ ...before.status, exists: status.exists });
        }
      } catch (error) {
        if (!active || before.epoch !== current.current.epoch) return;
        if (!initialized) setStartupError(errorMessage(error));
      } finally {
        pending = false;
        if (active) setReady(true);
      }
    };
    void subscribeToLock(() => {
      if (!active) return;
      initialized = true;
      updateStatus({ ...current.current.status, unlocked: false, session: null }, true);
    }).then((dispose) => {
      if (active) unlisten = dispose;
      else dispose();
    }).catch(() => {});
    void poll();
    const timer = window.setInterval(poll, 2000);
    return () => {
      active = false;
      unlisten?.();
      window.clearInterval(timer);
    };
  }, [native, updateStatus]);

  useEffect(() => {
    if (!state.status.session) return;
    let last = -Infinity;
    const input = (event: Event) => {
      if (!event.isTrusted || (event instanceof KeyboardEvent && event.repeat)) return;
      const time = performance.now();
      if (time - last < 1000) return;
      last = time;
      void run('activity').catch(() => {});
    };
    const inputs = ['pointerdown', 'keydown', 'input', 'wheel'] as const;
    inputs.forEach((name) => window.addEventListener(name, input, { capture: true, passive: true }));
    return () => inputs.forEach((name) => window.removeEventListener(name, input, true));
  }, [run, state.status.session]);

  const switchTheme = () => setDark((value) => !value);
  return <>
    {state.status.unlocked && state.status.session
      ? <Vault key={state.epoch} run={run} lock={lock} notify={notify} dark={dark} switchTheme={switchTheme} />
      : <Welcome key={state.epoch} exists={state.status.exists} run={run} native={native} dark={dark}
        switchTheme={switchTheme} notify={notify} ready={ready} startupError={startupError} />}
    <Toast text={toast} />
  </>;
}

function Welcome({ exists, run, native, dark, switchTheme, notify, ready, startupError }: {
  exists: boolean; run: Run; native: boolean; dark: boolean; switchTheme: () => void;
  notify: (message: string) => void; ready: boolean; startupError: string;
}) {
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [restore, setRestore] = useState(false);
  const enabled = native && ready && !startupError;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!enabled || busy) return;
    setError('');
    if (!exists && [...password].length < 12) return setError('主密码至少需要 12 个字符。');
    if (!exists && password !== confirmation) return setError('两次输入的主密码不一致。');
    if (exists && !password) return setError('请输入主密码。');
    setBusy(true);
    try { await run(exists ? 'unlock' : 'create', { password }); }
    catch (reason) { setError(errorMessage(reason)); }
    finally {
      setPassword('');
      setConfirmation('');
      setBusy(false);
    }
  }

  return <main className="welcome">
    <header className="welcome-top"><Brand /><ThemeButton dark={dark} onClick={switchTheme} /></header>
    <div className="welcome-content">
      <div className="vault-illustration" aria-hidden>
        <div className="orbit orbit-outside" /><div className="orbit orbit-middle" /><div className="orbit orbit-inside" />
        <div className="orbit-line horizontal" /><div className="orbit-line vertical" />
        <div className="vault-dial"><LockKeyhole size={39} strokeWidth={1.45} /></div>
        <span className="orbit-dot north" /><span className="orbit-dot west" /><span className="orbit-dot east" />
      </div>
      <div className="welcome-form">
        <h1>{exists ? '解锁保险库' : '创建保险库'}</h1>
        <p className="welcome-intro">{exists ? '输入主密码以访问保存在此设备上的记录。' : '把登录信息、密钥和动态验证码集中保存在此设备上。'}</p>
        <form onSubmit={submit} noValidate>
          <label className="field">主密码
            <input aria-describedby={!exists ? 'master-hint' : undefined} autoComplete={exists ? 'current-password' : 'new-password'}
              type="password" value={password} onChange={(event) => setPassword(event.target.value)} disabled={!enabled || busy} />
          </label>
          {!exists && <>
            <p className="hint master-hint" id="master-hint">至少 12 个字符。请妥善保管，忘记后无法找回。</p>
            <label className="field">确认主密码
              <input autoComplete="new-password" type="password" value={confirmation}
                onChange={(event) => setConfirmation(event.target.value)} disabled={!enabled || busy} />
            </label>
          </>}
          <Notice>{!native ? '请在 Key Keeper 桌面应用中创建或打开保险库。' : startupError || error}</Notice>
          <button className="button primary full" disabled={!enabled || busy}>
            {!ready ? '正在读取保险库…' : busy ? '正在处理…' : exists ? '解锁保险库' : '创建保险库'}
            <ArrowRight size={16} />
          </button>
        </form>
        <button className="text-button restore-entry" disabled={!enabled || busy} onClick={() => {
          setPassword(''); setConfirmation(''); setError(''); setRestore(true);
        }}><HardDrive size={16} /> 从加密备份恢复</button>
      </div>
    </div>
    <footer className="welcome-footer"><ShieldCheck size={15} /> 离线存储，加密保护</footer>
    {restore && <Restore run={run} close={() => setRestore(false)} notify={notify} />}
  </main>;
}

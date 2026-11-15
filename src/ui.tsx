import { useEffect, useRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { BadgeCheck, Fingerprint, KeyRound, LogIn, Moon, Shield, Sun, Ticket, X } from 'lucide-react';
import type { Kind } from './model';

const kindIcons = { login: LogIn, api: KeyRound, totp: Fingerprint, card: Ticket };

export function KindIcon({ kind, size = 19 }: { kind: Kind; size?: number }) {
  const Icon = kindIcons[kind];
  return <Icon size={size} strokeWidth={1.8} aria-hidden />;
}

export function Brand() {
  return <div className="brand"><span className="brand-mark"><Shield size={24} strokeWidth={1.7} /><i /></span><span>Key Keeper</span></div>;
}

export function IconButton({ label, children, className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  children: ReactNode;
}) {
  return <button type="button" className={`icon-button ${className}`} aria-label={label} title={label} {...props}>{children}</button>;
}

export function ThemeButton({ dark, onClick }: { dark: boolean; onClick: () => void }) {
  return <IconButton label={dark ? '切换到浅色主题' : '切换到深色主题'} onClick={onClick}>
    {dark ? <Sun size={18} /> : <Moon size={18} />}
  </IconButton>;
}

export function Notice({ children }: { children: ReactNode }) {
  return children ? <div className="notice" role="alert">{children}</div> : null;
}

export function Modal({ title, children, close, wide = false }: {
  title: string; children: ReactNode; close: () => void; wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (typeof dialog.showModal === 'function') dialog.showModal();
    else dialog.setAttribute('open', '');
    return () => { if (dialog.open && typeof dialog.close === 'function') dialog.close(); };
  }, []);

  return <dialog
    ref={ref}
    aria-label={title}
    className={wide ? 'modal wide' : 'modal'}
    onCancel={(event) => { event.preventDefault(); close(); }}
  >
    <div className="modal-heading">
      <h2>{title}</h2>
      <IconButton label="关闭" onClick={close}><X size={18} /></IconButton>
    </div>
    {children}
  </dialog>;
}

export function Toast({ text }: { text: string }) {
  return text ? <div className="toast" role="status"><BadgeCheck size={18} />{text}</div> : null;
}

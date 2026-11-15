import { useEffect, useState, type FormEvent } from 'react';
import { Check, Copy, Eye, EyeOff, Fingerprint, Pencil, Trash2 } from 'lucide-react';
import { fieldLabels, kindLabels, kinds, primaryFields, isSecret, recordDate, supportsTotp, totpFields, type Draft, type Kind, type VaultRecord } from './model';
import { errorMessage, type Run, type TotpCode } from './rpc';
import { IconButton, KindIcon, Notice } from './ui';

export interface EditState {
  record?: VaultRecord;
  draft: Draft;
  tagsText: string;
}

export function RecordEditor({ edit, change, save, cancel, busy, error }: {
  edit: EditState; change: (next: EditState) => void; save: () => void; cancel: () => void;
  busy: boolean; error: string;
}) {
  const { draft, record } = edit;
  const [shown, setShown] = useState<Record<string, boolean>>({});
  const update = (value: Partial<Draft>) => change({ ...edit, draft: { ...draft, ...value } });
  const changeKind = (kind: Kind) => { setShown({}); update({ kind, fields: {}, used: false }); };

  function field(name: string) {
    const secret = isSecret(name);
    const hadSecret = record?.secretFields?.includes(name);
    const defaultValues: Record<string, string> = { totpAlgorithm: 'SHA1', totpDigits: '6', totpPeriod: '30' };
    return <label className="field" key={name}>{fieldLabels[name]}
      <span className="input-wrap">
        <input type={secret && !shown[name] ? 'password' : 'text'} className={secret ? 'with-action' : ''}
          value={draft.fields[name] ?? ''} autoComplete={secret ? 'new-password' : undefined}
          spellCheck={false} placeholder={hadSecret && draft.fields[name] === undefined ? '保留现有值' : defaultValues[name]}
          onChange={(event) => update({ fields: { ...draft.fields, [name]: event.target.value } })} />
        {secret && <IconButton
          label={shown[name] ? `隐藏输入的${fieldLabels[name]}` : `显示输入的${fieldLabels[name]}`}
          onClick={() => setShown((values) => ({ ...values, [name]: !values[name] }))}
        >{shown[name] ? <EyeOff size={16} /> : <Eye size={16} />}</IconButton>}
      </span>
      {hadSecret && draft.fields[name] === undefined && <button type="button" className="text-button remove-secret"
        onClick={() => update({ fields: { ...draft.fields, [name]: '' } })}>清除现有值</button>}
      {hadSecret && draft.fields[name] !== undefined && <button type="button" className="text-button remove-secret"
        onClick={() => {
          const fields = { ...draft.fields };
          delete fields[name];
          update({ fields });
        }}>保留现有值</button>}
    </label>;
  }

  return <form className="record-editor" noValidate onSubmit={(event: FormEvent) => { event.preventDefault(); save(); }}>
    <header className="detail-top"><h2>{record ? '编辑记录' : '新建记录'}</h2><p>按需填写，留空的字段也可保存。</p></header>
    <fieldset className="editor-fields" disabled={busy}>
      {!record && <div className="kind-picker" role="group" aria-label="记录类型">
        {kinds.map((kind) => <button type="button" key={kind} aria-pressed={draft.kind === kind}
          className={draft.kind === kind ? 'kind-option selected' : 'kind-option'} onClick={() => changeKind(kind)}>
          <KindIcon kind={kind} size={16} />{kindLabels[kind]}
        </button>)}
      </div>}
      <label className="field">名称
        <input value={draft.title} autoFocus spellCheck={false} onChange={(event) => update({ title: event.target.value })} />
      </label>
      <div className="edit-grid">{primaryFields[draft.kind].map(field)}</div>
      {supportsTotp(draft.kind) && <details className="totp-editor" open={draft.kind === 'totp' || undefined}>
        <summary><Fingerprint size={17} /> 动态验证码</summary>
        <div className="edit-grid">{totpFields.map(field)}</div>
      </details>}
      {draft.kind === 'card' && <label className="checkbox">
        <input type="checkbox" checked={draft.used} onChange={(event) => update({ used: event.target.checked })} /> 已使用
      </label>}
      <label className="field">标签
        <input value={edit.tagsText} placeholder="用逗号分隔" onChange={(event) => change({ ...edit, tagsText: event.target.value })} />
      </label>
      <label className="field">备注
        <textarea rows={3} value={draft.notes} onChange={(event) => update({ notes: event.target.value })} />
      </label>
    </fieldset>
    <div className="editor-actions">
      <Notice>{error}</Notice>
      <div className="action-row">
        <button type="button" className="button secondary" onClick={cancel} disabled={busy}>取消</button>
        <button type="submit" className="button primary" disabled={busy}><Check size={16} />{busy ? '正在保存…' : '保存记录'}</button>
      </div>
    </div>
  </form>;
}

export function RecordDetails({ record, run, notify, edit, remove }: {
  record: VaultRecord; run: Run; notify: (message: string) => void; edit: () => void; remove: () => void;
}) {
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [totp, setTotp] = useState<TotpCode>();
  const [totpError, setTotpError] = useState('');
  const secretFields = record.secretFields ?? [];
  const hasTotp = supportsTotp(record.kind) && secretFields.includes('totpSecret');

  useEffect(() => {
    if (!hasTotp) return;
    let active = true;
    let pending = false;
    let timer: number | undefined;
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try {
        const code = await run('totp', { id: record.id });
        if (active) { setTotp(code); setTotpError(''); }
      } catch (reason) {
        if (active) { setTotp(undefined); setTotpError(errorMessage(reason)); }
        window.clearInterval(timer);
      } finally { pending = false; }
    };
    timer = window.setInterval(refresh, 1000);
    void refresh();
    return () => { active = false; window.clearInterval(timer); };
  }, [hasTotp, record.id, run]);

  async function reveal(field: string) {
    if (field in revealed) {
      const next = { ...revealed };
      delete next[field];
      setRevealed(next);
      return;
    }
    setError('');
    setLoading((fields) => [...fields, field]);
    try {
      const value = await run('reveal', { id: record.id, field });
      setRevealed((values) => ({ ...values, [field]: value }));
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setLoading((fields) => fields.filter((name) => name !== field)); }
  }

  async function copy(field: string) {
    setError('');
    try {
      await run('copy', { id: record.id, field });
      notify('已复制，剪贴板将在 30 秒后自动清理。');
    } catch (reason) { setError(errorMessage(reason)); }
  }

  function detailField(field: string) {
    const secret = secretFields.includes(field);
    const value = secret ? revealed[field] : record.fields[field];
    if (!secret && !value) return null;
    const shown = !secret || field in revealed;
    return <div className="detail-field" key={field}>
      <dt>{fieldLabels[field]}</dt>
      <dd>
        <span className={shown ? 'detail-value' : 'detail-value masked'}>
          {shown ? value : '••••••••••••'}
        </span>
        {secret && <IconButton label={`${shown ? '隐藏' : '显示'}${fieldLabels[field]}`}
          disabled={loading.includes(field)} onClick={() => void reveal(field)}>
          {shown ? <EyeOff size={16} /> : <Eye size={16} />}
        </IconButton>}
        <IconButton label={`复制${fieldLabels[field]}`} onClick={() => void copy(field)}><Copy size={16} /></IconButton>
      </dd>
    </div>;
  }

  const updated = recordDate(record.updatedAt);
  const visibleFields = primaryFields[record.kind].filter((field) => record.fields[field] || secretFields.includes(field));

  return <div className="record-details">
    <header className="record-heading">
      <div className={`record-symbol ${record.kind}`}><KindIcon kind={record.kind} size={25} /></div>
      <div className="record-actions">
        <button className="button secondary" onClick={edit}><Pencil size={15} />编辑</button>
        <IconButton className="danger-icon" label="删除记录" onClick={remove}><Trash2 size={18} /></IconButton>
      </div>
      <div className="record-name"><span>{kindLabels[record.kind]}</span><h2>{record.title}</h2></div>
    </header>
    {record.tags.length > 0 && <div className="chips">{record.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}
    <Notice>{error}</Notice>
    {visibleFields.length > 0 && <dl className="detail-fields">{visibleFields.map(detailField)}</dl>}
    {record.kind === 'card' && <p className="status-label">{record.used ? '已使用' : '未使用'}</p>}
    {supportsTotp(record.kind) && (hasTotp || totpFields.some((field) => record.fields[field])) &&
      <section className="totp-display">
        <h3><Fingerprint size={17} /> 动态验证码</h3>
        {totp && <div className="code-row">
          <span className="code">{totp.code}</span>
          <span className="code-timer" title={`剩余 ${totp.remaining} 秒`}>{totp.remaining} 秒</span>
          <IconButton label="复制动态验证码" onClick={() => void copy('totp')}><Copy size={16} /></IconButton>
          <progress max={totp.period} value={totp.remaining} aria-label="验证码剩余时间" />
        </div>}
        {totpError && <p className="hint">{totpError}</p>}
        <dl className="detail-fields compact">{totpFields.map(detailField)}</dl>
      </section>}
    {record.notes && <section className="record-notes"><h3>备注</h3><p>{record.notes}</p></section>}
    {!visibleFields.length && !record.notes && !hasTotp && <p className="hint">暂无更多内容，可点击编辑补充信息。</p>}
    {updated && <footer className="record-footer">更新于 {updated}</footer>}
  </div>;
}

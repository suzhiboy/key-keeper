import { useState, type FormEvent } from 'react';
import { ArrowLeft, ArrowRight, FileSpreadsheet, Upload } from 'lucide-react';
import { fieldsFor, fieldLabels, kinds, kindLabels, type Kind } from './model';
import { errorMessage, type ImportFile, type ImportPreview, type Run } from './rpc';
import { Modal, Notice } from './ui';

type Source = { encoding: 'utf8' | 'gb18030'; delimiter: string; sheet: string; hasHeader: boolean };
type Columns = { columns: string[]; mapping: Record<string, number> };
const rowStatus = { valid: '可导入', duplicate: '重复', error: '错误' };
const previewPageSize = 50;

function importFields(kind: Kind) {
  return ['title', ...fieldsFor(kind), ...(kind === 'card' ? ['used'] : []), 'tags', 'notes'];
}

function initialSource(file: ImportFile): Source {
  const txt = file.fileName?.toLowerCase().endsWith('.txt');
  return { encoding: 'utf8', delimiter: txt ? '' : ',', sheet: file.sheets?.[0] ?? '', hasHeader: !txt };
}

export function ImportWizard({ run, close, notify, imported }: {
  run: Run; close: () => void; notify: (message: string) => void; imported: () => void;
}) {
  const [file, setFile] = useState<ImportFile>();
  const [kind, setKind] = useState<Kind>('login');
  const [source, setSource] = useState<Source>({ encoding: 'utf8', delimiter: ',', sheet: '', hasHeader: true });
  const [columns, setColumns] = useState<Columns>();
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [skipErrors, setSkipErrors] = useState(false);
  const [preview, setPreview] = useState<ImportPreview>();
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const applicableFields = importFields(kind);

  async function action(operation: () => Promise<void>) {
    setBusy(true);
    setError('');
    try { await operation(); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }

  function closeWizard() {
    if (busy) return;
    const importId = file?.importId;
    if (importId) void run('import_cancel', { importId }).catch(() => {});
    close();
  }

  function choose() {
    void action(async () => {
      const result = await run('import_open');
      if (result.cancelled || !result.importId) return;
      setFile(result);
      setSource(initialSource(result));
      setColumns(undefined);
      setPreview(undefined);
    });
  }

  function previewRequest(mapping: Record<string, number>) {
    return run('import_preview', { importId: file?.importId, kind, ...source, sheet: source.sheet || undefined,
      mapping, skipDuplicates });
  }

  function matchColumns(names: string[], targetKind: Kind): Columns {
    const mapping: Record<string, number> = {};
    if (source.hasHeader) {
      for (const field of importFields(targetKind)) {
        const index = names.findIndex((column) => [field.toLowerCase(), fieldLabels[field]].includes(column.trim().toLowerCase()));
        if (index >= 0) mapping[field] = index;
      }
    }
    const oneColumnTxt = file?.fileName?.toLowerCase().endsWith('.txt') && names.length === 1;
    if (oneColumnTxt && targetKind === 'card') mapping.secretCode = 0;
    if (oneColumnTxt && targetKind === 'totp') mapping.totpSecret = 0;
    return { columns: names, mapping };
  }

  function readColumns() {
    setColumns(undefined);
    void action(async () => {
      const data = await previewRequest({});
      setColumns(matchColumns(data.columns, kind));
    });
  }

  function updateSource(next: Partial<Source>) {
    setColumns(undefined);
    setError('');
    setSource((value) => ({ ...value, ...next }));
  }

  function mapField(field: string, index: string) {
    if (!columns) return;
    const mapping = { ...columns.mapping };
    if (index === '') delete mapping[field];
    else mapping[field] = Number(index);
    setColumns({ ...columns, mapping });
  }

  function review(event: FormEvent) {
    event.preventDefault();
    if (!columns || busy) return;
    if (!Object.keys(columns.mapping).length) { setError('至少选择一个要导入的字段。'); return; }
    void action(async () => {
      setSkipErrors(false);
      setPage(0);
      setPreview(await previewRequest(columns.mapping));
    });
  }

  function commit() {
    void action(async () => {
      const { imported: count, skipped } = await run('import_commit', { importId: file?.importId, skipErrors });
      close();
      imported();
      notify(`已导入 ${count} 条记录${skipped ? `，跳过 ${skipped} 条` : ''}`);
    });
  }

  const step = preview ? 3 : file ? 2 : 1;
  return <Modal title="导入记录" close={closeWizard} wide>
    <ol className="steps" aria-label="导入步骤">
      {['选择文件', '匹配字段', '预览与导入'].map((label, index) =>
        <li key={label} className={step >= index + 1 ? 'current' : ''} aria-current={step === index + 1 ? 'step' : undefined}>
          <span>{index + 1}</span>{label}
        </li>)}
    </ol>
    {!file && <div className="import-start">
      <div className="file-mark"><FileSpreadsheet size={27} strokeWidth={1.5} /></div>
      <p>选择要导入的 TXT、CSV、XLS 或 XLSX 文件。</p>
      <button className="button primary" disabled={busy} onClick={choose}>
        <Upload size={16} /> {busy ? '正在打开…' : '选择文件'}
      </button>
      <Notice>{error}</Notice>
    </div>}
    {file && !preview && <form onSubmit={review}>
      <div className="selected-file"><FileSpreadsheet size={18} /><strong>{file.fileName}</strong>
        <button type="button" className="text-button" disabled={busy} onClick={choose}>更换文件</button>
      </div>
      <fieldset className="import-options" disabled={busy}>
        <label className="field">记录类型<select value={kind} onChange={(event) => {
          const next = event.target.value as Kind;
          setKind(next);
          if (columns) setColumns(matchColumns(columns.columns, next));
          setError('');
        }}>{kinds.map((value) => <option key={value} value={value}>{kindLabels[value]}</option>)}</select></label>
        {file.sheets?.length
          ? <label className="field">工作表<select value={source.sheet} onChange={(event) => updateSource({ sheet: event.target.value })}>
              {file.sheets.map((sheet) => <option key={sheet}>{sheet}</option>)}
            </select></label>
          : <>
              <label className="field">文件编码<select value={source.encoding}
                onChange={(event) => updateSource({ encoding: event.target.value as Source['encoding'] })}>
                <option value="utf8">UTF-8</option><option value="gb18030">GB18030</option>
              </select></label>
              <label className="field">分隔符<select value={source.delimiter} onChange={(event) => updateSource({ delimiter: event.target.value })}>
                <option value="">单列文本</option><option value=",">逗号</option><option value={String.fromCharCode(9)}>制表符</option>
                <option value=";">分号</option><option value="|">竖线</option><option value="----">四个连字符</option>
                {!['', ',', '\t', ';', '|', '----'].includes(source.delimiter) && <option value={source.delimiter}>自定义</option>}
              </select></label>
              <label className="field">自定义分隔符<input value={source.delimiter} onChange={(event) => updateSource({ delimiter: event.target.value })} placeholder="留空表示单列" /></label>
            </>}
      </fieldset>
      <div className="import-header-option">
        <label className="checkbox"><input type="checkbox" checked={source.hasHeader}
          disabled={busy} onChange={(event) => updateSource({ hasHeader: event.target.checked })} />首行是列标题</label>
        <button type="button" className="button secondary" onClick={readColumns} disabled={busy}>
          {busy && !columns ? '正在读取…' : columns ? '重新读取列' : '读取文件列'}
        </button>
      </div>
      {columns && <>
        <div className="mapping-heading"><h3>字段对应的源列</h3><p className="hint">只导入已匹配的字段。</p></div>
        <fieldset className="mapping-grid" disabled={busy}>
          {applicableFields.map((field) => <label className="field" key={field}>{fieldLabels[field]}
            <select value={columns.mapping[field] ?? ''} onChange={(event) => mapField(field, event.target.value)}>
              <option value="">不导入</option>
              {columns.columns.map((name, index) => <option key={index} value={index}>{index + 1}. {name}</option>)}
            </select>
          </label>)}
        </fieldset>
        <label className="checkbox skip-duplicates"><input type="checkbox" checked={skipDuplicates} disabled={busy}
          onChange={(event) => setSkipDuplicates(event.target.checked)} />跳过完全相同的记录</label>
      </>}
      <Notice>{error}</Notice>
      <div className="action-row">
        <button type="button" className="button secondary" disabled={busy} onClick={closeWizard}>取消</button>
        <button className="button primary" disabled={!columns || busy}>预览导入 <ArrowRight size={16} /></button>
      </div>
    </form>}
    {preview && <>
      {preview.sampleRows.length > 0 && <div className="preview-table-wrap"><table className="preview-table">
        <caption>字段预览（最多前几条，敏感列已遮挡）</caption>
        <thead><tr>{preview.columns.map((name, i) => <th key={i}>{name}</th>)}</tr></thead>
        <tbody>{preview.sampleRows.map((row, i) => <tr key={i}>{row.map((value, j) => <td key={j}>{value}</td>)}</tr>)}</tbody>
      </table></div>}
      <div className="import-summary">
        <span><strong>{preview.validCount}</strong> 可导入</span>
        <span><strong>{preview.duplicateCount}</strong> 重复</span>
        <span><strong>{preview.errorCount}</strong> 错误</span>
      </div>
      {preview.rows.length > 0 && <div className="preview-table-wrap"><table className="preview-table">
        <thead><tr><th>行</th><th>记录</th><th>状态</th><th>说明</th></tr></thead>
        <tbody>{preview.rows.slice(page * previewPageSize, (page + 1) * previewPageSize).map((row) => <tr key={row.index}>
          <td>{row.index}</td><td>{row.title}</td><td><span className={`preview-status ${row.status}`}>{rowStatus[row.status]}</span></td><td>{row.message}</td>
        </tr>)}</tbody>
      </table></div>}
      {preview.rows.length > previewPageSize && <nav className="preview-pages" aria-label="预览分页">
        <button type="button" className="text-button" disabled={page === 0}
          onClick={() => setPage(page - 1)}>上一页</button>
        <span aria-live="polite">第 {page + 1} / {Math.ceil(preview.rows.length / previewPageSize)} 页</span>
        <button type="button" className="text-button" disabled={(page + 1) * previewPageSize >= preview.rows.length}
          onClick={() => setPage(page + 1)}>下一页</button>
      </nav>}
      {preview.errorCount > 0 && <label className="checkbox skip-duplicates"><input type="checkbox" checked={skipErrors} disabled={busy}
        onChange={(event) => setSkipErrors(event.target.checked)} />跳过错误行，导入其余记录</label>}
      <Notice>{error}</Notice>
      <div className="action-row">
        <button className="button secondary" disabled={busy} onClick={() => { setPreview(undefined); setError(''); }}>
          <ArrowLeft size={16} /> 返回匹配
        </button>
        <button className="button primary" disabled={busy || (preview.errorCount > 0 && !skipErrors) ||
          preview.validCount + preview.duplicateCount === 0} onClick={commit}>
          {busy ? '正在导入…' : '确认导入'}
        </button>
      </div>
    </>}
  </Modal>;
}

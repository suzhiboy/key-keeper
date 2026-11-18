import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Archive, ArrowLeft, ChevronRight, Download, FolderOpen, LockKeyhole, Plus, Search, Settings2, Tag, Upload, X } from 'lucide-react';
import { kindLabels, kinds, makeDraft, recordSubtitle, type Kind, type VaultRecord } from './model';
import { errorMessage, type Run } from './rpc';
import { Brand, IconButton, KindIcon, Modal, Notice, ThemeButton } from './ui';
import { RecordDetails, RecordEditor, type EditState } from './RecordView';
import { Settings } from './dialogs';
import { ImportWizard } from './ImportWizard';

type Filter = { kind?: Kind; tag?: string };
type Confirm = { title: string; message: string; label: string; action: () => void | Promise<void> };

export function Vault({ run, lock, notify, dark, switchTheme }: {
  run: Run; lock: () => Promise<void>; notify: (message: string) => void; dark: boolean; switchTheme: () => void;
}) {
  const [filter, setFilter] = useState<Filter>({});
  const [query, setQuery] = useState('');
  const [all, setAll] = useState<VaultRecord[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState('');
  const [refresh, setRefresh] = useState(0);
  const [selected, setSelected] = useState<string>();
  const [detail, setDetail] = useState<VaultRecord>();
  const [detailError, setDetailError] = useState('');
  const [edit, setEdit] = useState<EditState>();
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editError, setEditError] = useState('');
  const [confirm, setConfirm] = useState<Confirm>();
  const [confirmBusy, setConfirmBusy] = useState(false);
  const [confirmError, setConfirmError] = useState('');
  const [dialog, setDialog] = useState<'settings' | 'import'>();
  const searchRef = useRef<HTMLInputElement>(null);
  const searchId = useId();
  const keyword = query.trim().toLowerCase();
  const filtered = Boolean(filter.kind || filter.tag || keyword);
  const searchIndex = useMemo(() => all.map((record) => ({
    record,
    terms: [record.title, record.notes, ...record.tags, ...Object.values(record.fields)]
      .map((value) => value.toLowerCase()),
  })), [all]);
  const records = useMemo(() => {
    if (!filtered) return all;
    return searchIndex.filter(({ record, terms }) =>
      (!filter.kind || filter.kind === record.kind) &&
      (!filter.tag || record.tags.includes(filter.tag)) &&
      (!keyword || terms.some((value) => value.includes(keyword)))
    ).map(({ record }) => record);
  }, [all, filter, filtered, keyword, searchIndex]);

  useEffect(() => {
    let active = true;
    setListLoading(true);
    setListError('');
    async function load() {
      try {
        const catalogue = await run('list');
        if (active) setAll(catalogue);
      } catch (error) {
        if (active) { setAll([]); setListError(errorMessage(error)); }
      } finally { if (active) setListLoading(false); }
    }
    void load();
    return () => { active = false; };
  }, [refresh, run]);

  useEffect(() => {
    let active = true;
    setDetail(undefined);
    setDetailError('');
    if (selected) {
      void run('detail', { id: selected }).then((record) => {
        if (active) setDetail(record);
      }).catch((error) => { if (active) setDetailError(errorMessage(error)); });
    }
    return () => { active = false; };
  }, [selected, refresh, run]);

  function ask(next: Confirm) {
    setConfirmError('');
    setConfirm(next);
  }

  function changeView(action: () => void) {
    if (saving) return;
    const proceed = () => { setEdit(undefined); setDirty(false); action(); };
    if (dirty) {
      ask({ title: '放弃未保存的更改？', message: '本次编辑的内容将被清除。', label: '放弃更改', action: proceed });
    } else proceed();
  }

  function navigate(next: Filter) {
    changeView(() => {
      setFilter(next);
      setQuery('');
      setSelected(undefined);
    });
  }

  function select(id?: string) {
    if (!edit && selected === id) return;
    changeView(() => setSelected(id));
  }

  function startNew() {
    changeView(() => {
      const draft = makeDraft(undefined, filter.kind, filter.tag);
      setEditError('');
      setEdit({ draft, tagsText: draft.tags.join(', ') });
    });
  }

  function startEdit(record: VaultRecord) {
    setEditError('');
    setEdit({ record, draft: makeDraft(record), tagsText: record.tags.join(', ') });
  }

  async function save() {
    if (!edit || saving) return;
    setSaving(true);
    setEditError('');
    try {
      const record = await run('save', {
        record: { ...edit.draft, tags: [...new Set(edit.tagsText.split(/[,，]/u).map((tag) => tag.trim()).filter(Boolean))] },
      });
      setEdit(undefined);
      setDirty(false);
      setSelected(record.id);
      setRefresh((value) => value + 1);
      notify('记录已保存');
    } catch (error) { setEditError(errorMessage(error)); }
    finally { setSaving(false); }
  }

  function remove(record: VaultRecord) {
    ask({
      title: '删除这条记录？', message: `「${record.title}」将从保险库中移除。`, label: '删除记录',
      action: async () => {
        await run('delete', { id: record.id });
        setSelected(undefined);
        setRefresh((value) => value + 1);
        notify('记录已删除');
      },
    });
  }

  async function confirmAction() {
    if (!confirm || confirmBusy) return;
    setConfirmBusy(true);
    setConfirmError('');
    try { await confirm.action(); setConfirm(undefined); }
    catch (error) { setConfirmError(errorMessage(error)); }
    finally { setConfirmBusy(false); }
  }

  const { tags, counts } = useMemo(() => {
    const uniqueTags = new Set<string>();
    const counts: Record<Kind, number> = { login: 0, api: 0, totp: 0, card: 0 };
    for (const record of all) {
      counts[record.kind]++;
      record.tags.forEach((tag) => uniqueTags.add(tag));
    }
    return { tags: [...uniqueTags].sort(new Intl.Collator('zh-CN').compare), counts };
  }, [all]);
  const title = filter.kind ? kindLabels[filter.kind] : filter.tag ?? '全部记录';
  const emptyTitle = filtered ? '未找到匹配的记录' : '保险库还没有记录';
  const emptyPrompt = query.trim() ? '换个关键词试试。' : filtered ? '这里还没有对应的记录。' : '创建第一条记录，或从文件中批量导入。';

  return <main className={`workspace${edit || selected ? ' has-detail' : ''}`}>
    <aside className="sidebar">
      <div className="sidebar-brand"><Brand /></div>
      <div className="sidebar-body">
        <div className="nav-group">
          <p className="nav-heading">保险库</p>
          <button className={!filter.kind && !filter.tag ? 'nav-item selected' : 'nav-item'}
            aria-current={!filter.kind && !filter.tag || undefined} onClick={() => navigate({})}>
            <Archive size={17} /><span>全部记录</span><small>{all.length}</small>
          </button>
          {kinds.map((kind) => <button className={filter.kind === kind ? 'nav-item selected' : 'nav-item'}
            aria-current={filter.kind === kind || undefined} key={kind} onClick={() => navigate({ kind })}>
            <KindIcon kind={kind} size={17} /><span>{kindLabels[kind]}</span>
            <small>{counts[kind]}</small>
          </button>)}
        </div>
        <div className="nav-group tag-group">
          <p className="nav-heading">标签</p>
          {tags.length === 0 && <p className="nav-empty">暂无标签</p>}
          {tags.map((tag) => <button className={filter.tag === tag ? 'nav-item selected' : 'nav-item'}
            aria-current={filter.tag === tag || undefined} key={tag} onClick={() => navigate({ tag })}>
            <Tag size={16} /><span>{tag}</span>
          </button>)}
        </div>
      </div>
      <div className="sidebar-bottom">
        <button className="nav-item" onClick={() => changeView(() => setDialog('import'))}><Upload size={17} /><span>导入记录</span></button>
        <button className="nav-item" onClick={() => changeView(() => setDialog('settings'))}><Settings2 size={17} /><span>设置</span></button>
        <div className="sidebar-controls">
          <button className="text-button lock-button" onClick={() => void lock()}><LockKeyhole size={16} /> 锁定保险库</button>
          <ThemeButton dark={dark} onClick={switchTheme} />
        </div>
      </div>
    </aside>
    <section className="list-panel" aria-label="记录列表">
      <header className="list-heading">
        <div><h1>{title}</h1><p>{listLoading ? '正在读取…' : `${records.length} 条记录`}</p></div>
        <IconButton label="新建记录" className="new-button" onClick={startNew}><Plus size={21} /></IconButton>
      </header>
      <div className="search">
        <label htmlFor={searchId}><Search size={17} /><span className="sr-only">搜索记录</span></label>
        <input id={searchId} ref={searchRef} type="search" placeholder="搜索记录" spellCheck={false}
          value={query} onChange={(event) => setQuery(event.target.value)} />
        {query && <IconButton label="清空搜索" onClick={() => { setQuery(''); searchRef.current?.focus(); }}><X size={15} /></IconButton>}
      </div>
      <Notice>{listError}</Notice>
      {listError && <button className="text-button" onClick={() => setRefresh((value) => value + 1)}>重新读取记录</button>}
      <div className="record-list">
        {!listLoading && !listError && records.length === 0 && <div className="list-empty">
          <FolderOpen size={25} strokeWidth={1.45} /><p>{emptyTitle}</p>
          {!filtered && <button className="text-button" onClick={startNew}>新建记录</button>}
        </div>}
        {records.map((record) => <button key={record.id} className={record.id === selected && !edit ? 'record-list-item selected' : 'record-list-item'}
          onClick={() => select(record.id)}>
          <span className={`record-symbol ${record.kind}`}><KindIcon kind={record.kind} size={18} /></span>
          <span className="row-text"><strong>{record.title}</strong><small>{recordSubtitle(record)}</small></span>
          <ChevronRight className="row-arrow" size={15} />
        </button>)}
      </div>
    </section>
    <section className="detail-panel" aria-label="记录详情">
      <button className="text-button back-to-list" onClick={() => select()}><ArrowLeft size={16} /> 返回记录列表</button>
      {edit
        ? <RecordEditor key={edit.draft.id || 'new'} edit={edit} change={(next) => { setEdit(next); setDirty(true); }}
            save={() => void save()} cancel={() => changeView(() => {})} busy={saving} error={editError} />
        : selected
          ? detail?.id === selected
            ? <RecordDetails key={`${detail.id}-${refresh}`} record={detail} run={run} notify={notify}
                edit={() => startEdit(detail)} remove={() => remove(detail)} />
            : <div className="detail-loading"><Notice>{detailError}</Notice>{!detailError && <p>正在读取记录…</p>}</div>
          : !listError && <div className="detail-empty">
            <div className="empty-motif" aria-hidden><i /><i /><Archive size={30} strokeWidth={1.45} /></div>
            <h2>{all.length > 0 || listLoading ? '选择一条记录' : '保险库还没有记录'}</h2>
            <p>{all.length > 0 || listLoading ? '从列表打开记录，查看详情或编辑。' : emptyPrompt}</p>
            {!all.length && !listLoading && <div className="empty-actions">
              <button className="button primary" onClick={startNew}><Plus size={16} /> 新建记录</button>
              <button className="button secondary" onClick={() => setDialog('import')}><Download size={16} /> 导入记录</button>
            </div>}
          </div>}
    </section>
    {confirm && <Modal title={confirm.title} close={() => { if (!confirmBusy) setConfirm(undefined); }}>
      <p>{confirm.message}</p><Notice>{confirmError}</Notice>
      <div className="action-row">
        <button className="button secondary" disabled={confirmBusy} onClick={() => setConfirm(undefined)}>取消</button>
        <button className="button danger" disabled={confirmBusy} onClick={() => void confirmAction()}>{confirm.label}</button>
      </div>
    </Modal>}
    {dialog === 'settings' && <Settings run={run} close={() => setDialog(undefined)} notify={notify} />}
    {dialog === 'import' && <ImportWizard run={run} close={() => setDialog(undefined)} notify={notify}
      imported={() => setRefresh((value) => value + 1)} />}
  </main>;
}

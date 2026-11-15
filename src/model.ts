export type Kind = 'login' | 'api' | 'totp' | 'card';

export interface VaultRecord {
  id: string;
  kind: Kind;
  title: string;
  tags: string[];
  notes: string;
  fields: Record<string, string>;
  used: boolean;
  createdAt: string;
  updatedAt: string;
  secretFields?: string[];
}

export type Draft = Omit<VaultRecord, 'secretFields'>;

export const kinds: Kind[] = ['login', 'api', 'totp', 'card'];
export const kindLabels: Record<Kind, string> = {
  login: '登录信息', api: 'API 密钥', totp: '动态验证码', card: '卡密',
};

export const fieldLabels: Record<string, string> = {
  title: '名称', tags: '标签', notes: '备注', used: '已使用状态',
  platform: '平台', username: '用户名', email: '邮箱', phone: '手机号', password: '密码', website: '网站',
  provider: '服务商', endpoint: '接口地址', secret: 'API 密钥',
  issuer: '签发方', accountName: '账户名', totpSecret: '验证码密钥',
  totpAlgorithm: '算法', totpDigits: '位数', totpPeriod: '周期（秒）',
  cardNumber: '卡号', secretCode: '兑换码或密码', expiryDate: '到期日期',
};

export const primaryFields: Record<Kind, string[]> = {
  login: ['platform', 'username', 'email', 'phone', 'password', 'website'],
  api: ['provider', 'endpoint', 'secret'],
  totp: ['issuer', 'accountName'],
  card: ['platform', 'cardNumber', 'secretCode', 'expiryDate'],
};

export const totpFields = ['totpSecret', 'totpAlgorithm', 'totpDigits', 'totpPeriod'];
export const supportsTotp = (kind: Kind) => kind === 'login' || kind === 'totp';
export const isSecret = (field: string) => ['password', 'secret', 'totpSecret', 'secretCode'].includes(field);
export const fieldsFor = (kind: Kind) => [
  ...primaryFields[kind],
  ...(supportsTotp(kind) ? totpFields : []),
];

export function makeDraft(record?: VaultRecord, kind: Kind = 'login', tag?: string): Draft {
  if (record) {
    const { secretFields: _secretFields, ...draft } = record;
    return { ...draft, fields: { ...draft.fields }, tags: [...draft.tags] };
  }
  return {
    id: '', kind, title: '', tags: tag ? [tag] : [], notes: '',
    fields: {}, used: false, createdAt: '', updatedAt: '',
  };
}

export function recordSubtitle(record: VaultRecord): string {
  const fields = record.fields;
  const alternatives: Record<Kind, string[]> = {
    login: [fields.username, fields.email, fields.phone, fields.platform, fields.website],
    api: [fields.provider, fields.endpoint],
    totp: [fields.issuer, fields.accountName],
    card: [fields.platform, fields.cardNumber, fields.expiryDate],
  };
  return alternatives[record.kind].find((value) => value && value !== record.title) ?? kindLabels[record.kind];
}

export function recordDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : new Intl.DateTimeFormat('zh-CN').format(date);
}

mod crypto;
mod import;
mod record;
mod storage;
mod totp;

use self::crypto::{Key, Salt, INVALID_VAULT, MAX_RECORDS};
use self::import::{ImportDocument, PreparedImport, PreviewRequest};
use self::record::{is_secret_field, Kind, Record};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LOCKED: &str = "保险库已锁定，请重新解锁";
const NOT_FOUND: &str = "找不到该记录";
const RECORD_LIMIT: &str = "保险库最多可保存 1000 条记录";

struct ActiveVault {
    session: Zeroizing<String>,
    last_activity: Instant,
    key: Key,
    salt: Salt,
    records: Vec<Record>,
}

struct Backup {
    id: String,
    bytes: Vec<u8>,
    validated_at: Option<Instant>,
}

struct Import {
    id: String,
    document: ImportDocument,
    prepared: Option<PreparedImport>,
}

pub struct Core {
    path: PathBuf,
    active: Option<ActiveVault>,
    backup: Option<Backup>,
    import: Option<Import>,
}

impl Core {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            path: data_dir.as_ref().join("vault.kkv"),
            active: None,
            backup: None,
            import: None,
        }
    }

    pub fn lock(&mut self) {
        self.active = None;
        self.backup = None;
        self.import = None;
    }

    pub fn expire(&mut self) -> bool {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.last_activity.elapsed() >= IDLE_TIMEOUT)
        {
            self.lock();
            return true;
        }
        false
    }

    pub fn execute(&mut self, request: Value) -> Result<Value, String> {
        self.expire();
        let request = SensitiveRequest(request);
        self.dispatch(&request.0)
    }

    fn dispatch(&mut self, request: &Value) -> Result<Value, String> {
        match required_text(request, "op")? {
            "status" => Ok(self.status()),
            "create" => self.create(required_text(request, "password")?),
            "unlock" => self.unlock(required_text(request, "password")?),
            "lock" => {
                self.lock();
                Ok(Value::Null)
            }
            "inspect_backup" => self.inspect_backup(Path::new(required_text(request, "path")?)),
            "restore_preview" => self.restore_preview(request),
            "restore_backup" => self.restore_backup(required_text(request, "backupId")?),
            operation => {
                self.require_session(request)?;
                self.dispatch_unlocked(operation, request)
            }
        }
    }

    fn dispatch_unlocked(&mut self, operation: &str, request: &Value) -> Result<Value, String> {
        match operation {
            "activity" => {
                self.active_mut().last_activity = Instant::now();
                Ok(Value::Null)
            }
            "list" => self.list(request),
            "detail" => Ok(self
                .record(request)?
                .view(request["includeSecrets"].as_bool().unwrap_or(false))),
            "save" => self.save(&request["record"]),
            "delete" => self.delete(required_text(request, "id")?),
            "reveal" => {
                let field = required_text(request, "field")?;
                if !is_secret_field(field) {
                    return Err("该字段不可显示为密文".to_owned());
                }
                Ok(Value::String(
                    field_value(self.record(request)?, field)?.to_owned(),
                ))
            }
            "totp" => Ok(json!(totp::generate(self.record(request)?)?)),
            "change_password" => self.change_password(request),
            "export_backup" => self.export_backup(Path::new(required_text(request, "path")?)),
            "import_open" => self.import_open(Path::new(required_text(request, "path")?)),
            "import_preview" => self.import_preview(request),
            "import_commit" => self.import_commit(request),
            "import_cancel" => {
                self.checked_import(request)?;
                self.import = None;
                Ok(Value::Null)
            }
            _ => Err("不支持此操作".to_owned()),
        }
    }

    pub fn copy_value(&mut self, request: &Value) -> Result<Zeroizing<String>, String> {
        self.expire();
        self.require_session(request)?;
        let record = self.record(request)?;
        let field = required_text(request, "field")?;
        let value = if field == "totp" {
            totp::generate(record)?.code
        } else {
            field_value(record, field)?.to_owned()
        };
        Ok(Zeroizing::new(value))
    }

    fn exists(&self) -> bool {
        match fs::symlink_metadata(&self.path) {
            Ok(_) => true,
            Err(error) => error.kind() != ErrorKind::NotFound,
        }
    }

    fn status(&self) -> Value {
        json!({
            "exists": self.exists(),
            "unlocked": self.active.is_some(),
            "session": self.active.as_ref().map(|active| active.session.as_str()),
        })
    }

    fn create(&mut self, password: &str) -> Result<Value, String> {
        if self.exists() {
            return Err("保险库已存在".to_owned());
        }
        validate_password(password)?;
        let salt = crypto::random_bytes()?;
        let key = crypto::derive_key(password.as_bytes(), &salt)?;
        let session = crypto::random_id()?;
        let records = Vec::new();
        let bytes = crypto::encrypt(&records, &key, &salt)?;
        storage::write_encrypted(&self.path, &bytes, false)?;
        Ok(self.start_session(records, key, salt, session))
    }

    fn unlock(&mut self, password: &str) -> Result<Value, String> {
        let bytes = storage::read_encrypted(&self.path)?;
        let (records, key, salt) = crypto::decrypt(&bytes, password.as_bytes())?;
        let session = crypto::random_id()?;
        Ok(self.start_session(records, key, salt, session))
    }

    fn start_session(
        &mut self,
        records: Vec<Record>,
        key: Key,
        salt: Salt,
        session: String,
    ) -> Value {
        self.lock();
        self.active = Some(ActiveVault {
            session: Zeroizing::new(session),
            last_activity: Instant::now(),
            key,
            salt,
            records,
        });
        self.status()
    }

    fn require_session(&self, request: &Value) -> Result<(), String> {
        if let Some(active) = &self.active {
            if request["session"].as_str().is_some_and(|session| {
                bool::from(session.as_bytes().ct_eq(active.session.as_bytes()))
            }) {
                return Ok(());
            }
        }
        Err(LOCKED.to_owned())
    }

    fn active(&self) -> &ActiveVault {
        self.active.as_ref().expect("session was checked")
    }

    fn active_mut(&mut self) -> &mut ActiveVault {
        self.active.as_mut().expect("session was checked")
    }

    fn record(&self, request: &Value) -> Result<&Record, String> {
        let id = required_text(request, "id")?;
        self.active()
            .records
            .iter()
            .find(|record| record.id == id)
            .ok_or_else(|| NOT_FOUND.to_owned())
    }

    fn list(&self, request: &Value) -> Result<Value, String> {
        let query = request["query"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let tag = request["tag"].as_str().filter(|tag| !tag.is_empty());
        let kind = if request["kind"].is_null() || request["kind"].as_str() == Some("") {
            None
        } else {
            Some(Kind::deserialize(&request["kind"]).map_err(|_| "记录类型无效".to_owned())?)
        };
        let views = self
            .active()
            .records
            .iter()
            .filter(|record| kind.is_none_or(|kind| kind == record.kind))
            .filter(|record| tag.is_none_or(|tag| record.tags.iter().any(|value| value == tag)))
            .filter(|record| record.matches_query(&query))
            .map(|record| record.view(false))
            .collect();
        Ok(Value::Array(views))
    }

    fn save(&mut self, request_record: &Value) -> Result<Value, String> {
        let mut record =
            Record::deserialize(request_record).map_err(|_| "记录格式无效".to_owned())?;
        let mut records = self.active().records.clone();
        let index = if record.id.is_empty() {
            if records.len() >= MAX_RECORDS {
                return Err(RECORD_LIMIT.to_owned());
            }
            record.prepare_new()?;
            records.len()
        } else {
            let index = records
                .iter()
                .position(|saved| saved.id == record.id)
                .ok_or_else(|| NOT_FOUND.to_owned())?;
            record.prepare_update(&records[index])?;
            index
        };
        let view = record.view(false);
        if index == records.len() {
            records.push(record);
        } else {
            records[index] = record;
        }
        self.commit(records)?;
        Ok(view)
    }

    fn delete(&mut self, id: &str) -> Result<Value, String> {
        let mut records = self.active().records.clone();
        let index = records
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| NOT_FOUND.to_owned())?;
        records.remove(index);
        self.commit(records)?;
        Ok(Value::Null)
    }

    fn commit(&mut self, records: Vec<Record>) -> Result<(), String> {
        let active = self.active();
        let bytes = crypto::encrypt(&records, &active.key, &active.salt)?;
        storage::write_encrypted(&self.path, &bytes, true)?;
        self.active_mut().records = records;
        if let Some(import) = &mut self.import {
            import.prepared = None;
        }
        Ok(())
    }

    fn change_password(&mut self, request: &Value) -> Result<Value, String> {
        let old_password = required_text(request, "oldPassword")?;
        let new_password = required_text(request, "newPassword")?;
        validate_password(new_password)?;
        let active = self.active();
        let old_key = crypto::derive_key(old_password.as_bytes(), &active.salt)?;
        if !bool::from(old_key.as_slice().ct_eq(active.key.as_slice())) {
            return Err(INVALID_VAULT.to_owned());
        }
        let salt = crypto::random_bytes()?;
        let key = crypto::derive_key(new_password.as_bytes(), &salt)?;
        let session = crypto::random_id()?;
        let bytes = crypto::encrypt(&active.records, &key, &salt)?;
        storage::write_encrypted(&self.path, &bytes, true)?;
        let records = std::mem::take(&mut self.active_mut().records);
        Ok(self.start_session(records, key, salt, session))
    }

    fn export_backup(&self, path: &Path) -> Result<Value, String> {
        let active = self.active();
        let bytes = crypto::encrypt(&active.records, &active.key, &active.salt)?;
        storage::write_encrypted(path, &bytes, true)?;
        Ok(json!({"cancelled": false}))
    }

    fn inspect_backup(&mut self, path: &Path) -> Result<Value, String> {
        self.backup = None;
        let bytes = storage::read_encrypted(path)?;
        crypto::check_header(&bytes)?;
        let id = crypto::random_id()?;
        let response = json!({"cancelled": false, "backupId": id});
        self.backup = Some(Backup {
            id,
            bytes,
            validated_at: None,
        });
        Ok(response)
    }

    fn restore_preview(&mut self, request: &Value) -> Result<Value, String> {
        let id = required_text(request, "backupId")?;
        let password = required_text(request, "password")?;
        let backup = self
            .backup
            .as_mut()
            .filter(|backup| backup.id == id)
            .ok_or_else(|| "请重新选择备份文件".to_owned())?;
        backup.validated_at = None;
        let (records, _, _) = crypto::decrypt(&backup.bytes, password.as_bytes())?;
        backup.validated_at = Some(Instant::now());
        // The exact authenticated bytes remain available for confirmation without storing plaintext.
        Ok(json!({"count": records.len()}))
    }

    fn restore_backup(&mut self, id: &str) -> Result<Value, String> {
        let backup = self
            .backup
            .as_ref()
            .filter(|backup| backup.id == id)
            .ok_or_else(|| "请重新选择备份文件".to_owned())?;
        if backup
            .validated_at
            .is_none_or(|validated_at| validated_at.elapsed() >= IDLE_TIMEOUT)
        {
            return Err("请先验证备份密码".to_owned());
        }
        storage::write_encrypted(&self.path, &backup.bytes, true)?;
        self.lock();
        Ok(self.status())
    }

    fn import_open(&mut self, path: &Path) -> Result<Value, String> {
        self.import = None;
        let document = ImportDocument::open(path)?;
        let id = crypto::random_id()?;
        let response = json!({
            "cancelled": false,
            "importId": id,
            "fileName": document.file_name(),
            "sheets": document.sheets(),
        });
        self.import = Some(Import {
            id,
            document,
            prepared: None,
        });
        Ok(response)
    }

    fn checked_import(&self, request: &Value) -> Result<&Import, String> {
        let id = required_text(request, "importId")?;
        self.import
            .as_ref()
            .filter(|import| import.id == id)
            .ok_or_else(|| "请重新选择导入文件".to_owned())
    }

    fn import_preview(&mut self, request: &Value) -> Result<Value, String> {
        self.checked_import(request)?;
        let import = self.import.as_mut().expect("import ID was checked");
        import.prepared = None;
        let request =
            PreviewRequest::deserialize(request).map_err(|_| "导入设置无效".to_owned())?;
        let active = self.active.as_ref().expect("session was checked");
        let prepared = import.document.preview(&request, &active.records)?;
        let response = json!(&prepared.preview);
        import.prepared = Some(prepared);
        Ok(response)
    }

    fn import_commit(&mut self, request: &Value) -> Result<Value, String> {
        let import = self.checked_import(request)?;
        let prepared = import
            .prepared
            .as_ref()
            .filter(|prepared| prepared.ready)
            .ok_or_else(|| "请先预览导入数据".to_owned())?;
        let errors = prepared.preview.error_count;
        if errors > 0 && request["skipErrors"].as_bool() != Some(true) {
            return Err("请修正错误行或选择跳过错误行".to_owned());
        }
        let imported = prepared.records.len();
        let skipped = errors + prepared.preview.duplicate_count;
        if self.active().records.len() + imported > MAX_RECORDS {
            return Err(RECORD_LIMIT.to_owned());
        }
        if imported > 0 {
            let mut records = self.active().records.clone();
            records.extend_from_slice(&prepared.records);
            self.commit(records)?;
        }
        self.import = None;
        Ok(json!({"imported": imported, "skipped": skipped}))
    }
}

fn required_text<'a>(request: &'a Value, key: &str) -> Result<&'a str, String> {
    request[key]
        .as_str()
        .ok_or_else(|| "请求参数无效".to_owned())
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 12 {
        return Err("主密码至少需要 12 个字符".to_owned());
    }
    Ok(())
}

fn field_value<'a>(record: &'a Record, field: &str) -> Result<&'a str, String> {
    let value = record.field(field);
    if value.is_empty() {
        return Err("该字段为空".to_owned());
    }
    Ok(value)
}

struct SensitiveRequest(Value);

impl Drop for SensitiveRequest {
    fn drop(&mut self) {
        fn scrub(value: &mut Value) {
            match value {
                Value::String(value) => value.zeroize(),
                Value::Array(values) => values.iter_mut().for_each(scrub),
                Value::Object(values) => values.values_mut().for_each(scrub),
                _ => {}
            }
        }
        scrub(&mut self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_poll_does_not_extend_idle_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Core::new(dir.path());
        core.start_session(
            Vec::new(),
            Key::new([0; 32]),
            [0; 16],
            "test-session".into(),
        );
        let activity = core.active().last_activity;
        let status = core.execute(json!({"op": "status"})).unwrap();
        assert_eq!(status["unlocked"], true);
        assert_eq!(core.active().last_activity, activity);
        core.active_mut().last_activity = activity - IDLE_TIMEOUT;
        let status = core.execute(json!({"op": "status"})).unwrap();
        assert_eq!(status["unlocked"], false);
        assert!(core
            .execute(json!({"op": "activity", "session":"test-session"}))
            .is_err());
    }
}

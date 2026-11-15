#[path = "../src/core/mod.rs"]
mod core;

use core::Core;
use serde_json::{json, Value};
use std::fs;
use tempfile::{tempdir, TempDir};

const PASSWORD: &str = "test master password";
const NEW_PASSWORD: &str = "replacement master password";
const TEST_SECRET: &str = "synthetic-private-value";

#[test]
fn each_login_field_can_be_saved_on_its_own() {
    let (_dir, mut core, session) = initialized();
    for field in ["platform", "username", "email", "phone", "password"] {
        let mut fields = serde_json::Map::new();
        fields.insert(field.into(), json!("待补全 00123"));
        let record = save(&mut core, &session, json!({"kind":"login","fields":fields}));
        let detail = core
            .execute(
                json!({"op":"detail","session":session,"id":record["id"],"includeSecrets":true}),
            )
            .unwrap();
        assert_eq!(detail["fields"][field], "待补全 00123");
    }
}

#[test]
fn imports_are_confirmed_as_a_batch_and_invalidated_by_lock() {
    let (dir, mut core, session) = initialized();
    let input = dir.path().join("accounts.csv");
    fs::write(&input, "platform,email,password\nExample,demo@example.test,00123\nbroken\nExample,other@example.test,00456\n").unwrap();
    let file = core
        .execute(json!({"op":"import_open","session":session,"path":input}))
        .unwrap();
    let id = &file["importId"];
    let preview = core.execute(json!({"op":"import_preview","session":session,"importId":id,"kind":"login","delimiter":",","hasHeader":true,"mapping":{"platform":0,"email":1,"password":2}})).unwrap();
    assert_eq!(preview["validCount"], 2);
    assert_eq!(preview["errorCount"], 1);
    let bytes_before = fs::read(dir.path().join("vault.kkv")).unwrap();
    assert!(core
        .execute(json!({"op":"import_commit","session":session,"importId":id,"skipErrors":false}))
        .is_err());
    assert_eq!(
        fs::read(dir.path().join("vault.kkv")).unwrap(),
        bytes_before
    );
    let result = core
        .execute(json!({"op":"import_commit","session":session,"importId":id,"skipErrors":true}))
        .unwrap();
    assert_eq!(result, json!({"imported":2,"skipped":1}));
    let file = core
        .execute(json!({"op":"import_open","session":session,"path":input}))
        .unwrap();
    core.lock();
    let next = core
        .execute(json!({"op":"unlock","password":PASSWORD}))
        .unwrap();
    assert!(core.execute(json!({"op":"import_commit","session":next["session"],"importId":file["importId"],"skipErrors":true})).is_err());
    assert_eq!(
        core.execute(json!({"op":"list","session":next["session"]}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn thousand_record_import_and_search_preserve_leading_zeros() {
    let (dir, mut core, session) = initialized();
    let input = dir.path().join("cards.txt");
    let contents: String = (0..1000).map(|i| format!("Card {i}----{i:08}\n")).collect();
    fs::write(&input, contents).unwrap();
    let file = core
        .execute(json!({"op":"import_open","session":session,"path":input}))
        .unwrap();
    let preview = core.execute(json!({"op":"import_preview","session":session,"importId":file["importId"],"kind":"card","delimiter":"----","hasHeader":false,"mapping":{"title":0,"secretCode":1}})).unwrap();
    assert_eq!(preview["validCount"], 1000);
    core.execute(json!({"op":"import_commit","session":session,"importId":file["importId"]}))
        .unwrap();
    let found = core
        .execute(json!({"op":"list","session":session,"query":"Card 999"}))
        .unwrap();
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(
        core.execute(
            json!({"op":"reveal","session":session,"id":found[0]["id"],"field":"secretCode"})
        )
        .unwrap(),
        "00000999"
    );
}

fn initialized() -> (TempDir, Core, String) {
    let dir = tempdir().unwrap();
    let mut core = Core::new(dir.path());
    let status = core
        .execute(json!({"op":"create", "password":PASSWORD}))
        .unwrap();
    let session = status["session"].as_str().unwrap().to_owned();
    (dir, core, session)
}

fn save(core: &mut Core, session: &str, record: Value) -> Value {
    core.execute(json!({"op":"save", "session":session, "record":record}))
        .unwrap()
}

#[test]
fn partial_records_preserve_omitted_secrets_and_stay_redacted() {
    let (_dir, mut core, session) = initialized();
    let saved = save(
        &mut core,
        &session,
        json!({"kind":"login", "tags":["家庭"], "fields":{"platform":"邮件", "password":TEST_SECRET}}),
    );
    assert_eq!(saved["title"], "邮件");
    assert_eq!(saved["secretFields"], json!(["password"]));
    assert!(saved["fields"].get("password").is_none());

    let id = &saved["id"];
    let edited = save(
        &mut core,
        &session,
        json!({"id":id, "kind":"login", "tags":["家庭"], "fields":{"platform":"邮件服务", "totpSecret":"not-base32!"}}),
    );
    assert_eq!(edited["title"], "邮件服务");
    let details = core
        .execute(json!({"op":"detail", "session":session, "id":id}))
        .unwrap();
    assert_eq!(details["fields"], json!({"platform":"邮件服务"}));
    let copied = core
        .copy_value(&json!({"session":session, "id":id, "field":"password"}))
        .unwrap();
    assert_eq!(&**copied, TEST_SECRET);
    assert!(core
        .execute(json!({"op":"totp", "session":session, "id":id}))
        .is_err());
    let list = core
        .execute(json!({"op":"list", "session":session, "query":"邮件", "tag":"家庭"}))
        .unwrap();
    assert_eq!(list[0]["id"], *id);
    let secret_search = core
        .execute(json!({"op":"list", "session":session, "query":TEST_SECRET}))
        .unwrap();
    assert_eq!(secret_search, json!([]));

    let cleared = save(
        &mut core,
        &session,
        json!({"id":id, "kind":"login", "fields":{"password":""}}),
    );
    assert_eq!(cleared["secretFields"], json!(["totpSecret"]));
}

#[test]
fn supports_empty_records_for_all_four_kinds_and_rejects_stale_sessions() {
    let (_dir, mut core, session) = initialized();
    for kind in ["login", "api", "totp", "card"] {
        let saved = save(&mut core, &session, json!({"kind":kind}));
        assert_eq!(saved["kind"], kind);
        assert!(!saved["title"].as_str().unwrap().is_empty());
        let id = &saved["id"];
        core.execute(json!({"op":"delete", "session":session, "id":id}))
            .unwrap();
        assert!(core
            .execute(json!({"op":"detail", "session":session, "id":id}))
            .is_err());
    }
    assert!(!core.expire());
    core.lock();
    let status = core
        .execute(json!({"op":"unlock", "password":PASSWORD}))
        .unwrap();
    assert_ne!(status["session"], json!(session));
    assert!(core
        .execute(json!({"op":"list", "session":session}))
        .is_err());
}

#[test]
fn vault_bytes_hide_fields_and_tampering_never_overwrites_the_vault() {
    let (dir, mut core, session) = initialized();
    let path = dir.path().join("vault.kkv");
    let initial = fs::read(&path).unwrap();
    save(
        &mut core,
        &session,
        json!({"kind":"api", "fields":{"secret":TEST_SECRET}}),
    );
    let authentic = fs::read(&path).unwrap();
    assert_ne!(initial, authentic);
    assert!(!authentic
        .windows(TEST_SECRET.len())
        .any(|bytes| bytes == TEST_SECRET.as_bytes()));
    core.lock();
    assert!(core
        .execute(json!({"op":"unlock", "password":NEW_PASSWORD}))
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), authentic);
    let mut tampered = authentic;
    *tampered.last_mut().unwrap() ^= 1;
    fs::write(&path, &tampered).unwrap();
    assert!(core
        .execute(json!({"op":"unlock", "password":PASSWORD}))
        .is_err());
    assert!(core
        .execute(json!({"op":"create", "password":PASSWORD}))
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), tampered);
}

#[test]
fn write_failure_keeps_the_existing_session_data() {
    let (dir, mut core, session) = initialized();
    let initial = save(&mut core, &session, json!({"kind":"card", "title":"初始"}));
    let path = dir.path().join("vault.kkv");
    let original = fs::read(&path).unwrap();
    let moved = dir.path().join("moved.kkv");
    fs::rename(&path, &moved).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(core.execute(json!({"op":"save", "session":session, "record":{"id":initial["id"], "kind":"card", "title":"未提交"}})).is_err());
    assert_eq!(
        core.execute(json!({"op":"list", "session":session}))
            .unwrap()[0]["title"],
        "初始"
    );
    assert_eq!(fs::read(&moved).unwrap(), original);
}

#[test]
fn encrypted_backup_restores_into_empty_directory_with_its_own_password() {
    let (dir, mut core, session) = initialized();
    let saved = save(
        &mut core,
        &session,
        json!({"kind":"card", "used":true, "fields":{"secretCode":TEST_SECRET, "cardNumber":"0012"}}),
    );
    let path = dir.path().join("export.kkv");
    assert_eq!(
        core.execute(json!({"op":"export_backup", "session":session, "path":path}))
            .unwrap(),
        json!({"cancelled":false})
    );
    let changed = core.execute(json!({"op":"change_password", "session":session, "oldPassword":PASSWORD, "newPassword":NEW_PASSWORD})).unwrap();
    assert_ne!(changed["session"], json!(session));
    assert!(core
        .execute(json!({"op":"list", "session":session}))
        .is_err());

    let dir = tempdir().unwrap();
    let mut restored = Core::new(dir.path());
    let inspected = restored
        .execute(json!({"op":"inspect_backup", "path":path}))
        .unwrap();
    let backup = &inspected["backupId"];
    assert!(restored
        .execute(json!({"op":"restore_backup", "backupId":backup}))
        .is_err());
    assert!(restored
        .execute(json!({"op":"restore_preview", "backupId":backup, "password":NEW_PASSWORD}))
        .is_err());
    assert_eq!(
        restored
            .execute(json!({"op":"restore_preview", "backupId":backup, "password":PASSWORD}))
            .unwrap(),
        json!({"count":1})
    );
    let status = restored
        .execute(json!({"op":"restore_backup", "backupId":backup}))
        .unwrap();
    assert_eq!(
        status,
        json!({"exists":true, "unlocked":false, "session":null})
    );
    let unlocked = restored
        .execute(json!({"op":"unlock", "password":PASSWORD}))
        .unwrap();
    assert_eq!(restored.execute(json!({"op":"reveal", "session":unlocked["session"], "id":saved["id"], "field":"secretCode"})).unwrap(), json!(TEST_SECRET));
}

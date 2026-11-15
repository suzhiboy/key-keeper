use super::record::Record;
use aes_gcm::{aead::AeadInPlace, Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Block, Params, Version};
use std::collections::HashSet;
use std::fmt::Write;
use zeroize::Zeroizing;

pub(super) const MAX_RECORDS: usize = 1000;
pub(super) const MAX_VAULT_SIZE: usize = 64 * 1024 * 1024;
pub(super) const INVALID_VAULT: &str = "密码错误或保险库文件已损坏";
const MAGIC: &[u8; 8] = b"KKVAULT\0";
const VERSION: u8 = 1;
const SALT_START: usize = MAGIC.len() + 1;
const SALT_END: usize = SALT_START + 16;
const HEADER_LEN: usize = SALT_END + 12;
const TAG_LEN: usize = 16;

pub(super) type Key = Zeroizing<[u8; 32]>;
pub(super) type Salt = [u8; 16];

pub(super) fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).map_err(|_| "系统随机源不可用".to_owned())?;
    Ok(bytes)
}

pub(super) fn random_id() -> Result<String, String> {
    let mut id = String::with_capacity(32);
    for byte in random_bytes::<16>()? {
        write!(id, "{byte:02x}").expect("writing to String succeeds");
    }
    Ok(id)
}

pub(super) fn derive_key(password: &[u8], salt: &Salt) -> Result<Key, String> {
    const MEMORY_KIB: u32 = 64 * 1024;
    let params =
        Params::new(MEMORY_KIB, 3, 1, Some(32)).expect("fixed Argon2id parameters are valid");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut blocks = Zeroizing::new(vec![Block::default(); MEMORY_KIB as usize]);
    let mut key = Key::new([0; 32]);
    argon
        .hash_password_into_with_memory(password, salt, key.as_mut(), blocks.as_mut_slice())
        .map_err(|_| "无法派生保险库密钥".to_owned())?;
    Ok(key)
}

pub(super) fn check_header(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < HEADER_LEN + TAG_LEN || !bytes.starts_with(MAGIC) {
        return Err("保险库文件格式无效".to_owned());
    }
    if bytes[MAGIC.len()] != VERSION {
        return Err("保险库文件版本不受支持".to_owned());
    }
    Ok(())
}

pub(super) fn encrypt(records: &[Record], key: &Key, salt: &Salt) -> Result<Vec<u8>, String> {
    let mut contents = Zeroizing::new(Vec::new());
    serde_json::to_writer(&mut *contents, records).map_err(|_| "无法序列化保险库".to_owned())?;
    if contents.len() > MAX_VAULT_SIZE - HEADER_LEN - TAG_LEN {
        return Err("保险库文件超过大小限制".to_owned());
    }

    let nonce = random_bytes::<12>()?;
    let mut bytes = Vec::with_capacity(HEADER_LEN + contents.len() + TAG_LEN);
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.extend_from_slice(salt);
    bytes.extend_from_slice(&nonce);

    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).expect("AES-256 takes a 32-byte key");
    let tag = cipher
        .encrypt_in_place_detached(&Nonce::from(nonce), &bytes, &mut contents)
        .map_err(|_| "无法加密保险库".to_owned())?;
    bytes.extend_from_slice(&contents);
    bytes.extend_from_slice(&tag);
    Ok(bytes)
}

pub(super) fn decrypt(bytes: &[u8], password: &[u8]) -> Result<(Vec<Record>, Key, Salt), String> {
    check_header(bytes)?;
    let salt = bytes[SALT_START..SALT_END]
        .try_into()
        .expect("validated header");
    let nonce: [u8; 12] = bytes[SALT_END..HEADER_LEN]
        .try_into()
        .expect("validated header");
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).expect("AES-256 takes a 32-byte key");
    let mut contents = Zeroizing::new(bytes[HEADER_LEN..].to_vec());
    cipher
        .decrypt_in_place(&Nonce::from(nonce), &bytes[..HEADER_LEN], &mut *contents)
        .map_err(|_| INVALID_VAULT.to_owned())?;

    let records: Vec<Record> =
        serde_json::from_slice(&contents).map_err(|_| "保险库数据格式无效".to_owned())?;
    let mut ids = HashSet::new();
    if records.len() > MAX_RECORDS
        || records.iter().any(|record| {
            record.id.is_empty()
                || !ids.insert(&record.id)
                || record
                    .fields
                    .keys()
                    .any(|field| !record.kind.field_allowed(field))
        })
    {
        return Err("保险库数据格式无效".to_owned());
    }
    Ok((records, key, salt))
}

use super::crypto::random_id;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use zeroize::Zeroize;

const TOTP_FIELDS: &[&str] = &[
    "issuer",
    "accountName",
    "totpSecret",
    "totpAlgorithm",
    "totpDigits",
    "totpPeriod",
];

#[derive(Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    #[default]
    Login,
    Api,
    Totp,
    Card,
}

impl Kind {
    pub(super) fn field_allowed(self, field: &str) -> bool {
        match self {
            Self::Login => {
                [
                    "platform", "username", "email", "phone", "password", "website",
                ]
                .contains(&field)
                    || TOTP_FIELDS.contains(&field)
            }
            Self::Api => ["provider", "endpoint", "secret"].contains(&field),
            Self::Totp => TOTP_FIELDS.contains(&field),
            Self::Card => ["platform", "cardNumber", "secretCode", "expiryDate"].contains(&field),
        }
    }
}

pub(super) fn is_secret_field(field: &str) -> bool {
    matches!(field, "password" | "secret" | "totpSecret" | "secretCode")
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Record {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) kind: Kind,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    pub(super) tags: Vec<String>,
    #[serde(default)]
    pub(super) notes: String,
    #[serde(default)]
    pub(super) fields: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) used: bool,
    #[serde(default)]
    pub(super) created_at: String,
    #[serde(default)]
    pub(super) updated_at: String,
}

impl Drop for Record {
    fn drop(&mut self) {
        self.id.zeroize();
        self.title.zeroize();
        self.tags.zeroize();
        self.notes.zeroize();
        self.created_at.zeroize();
        self.updated_at.zeroize();
        for value in self.fields.values_mut() {
            value.zeroize();
        }
    }
}

impl Record {
    pub(super) fn prepare_new(&mut self) -> Result<(), String> {
        self.id = random_id()?;
        self.prepare()?;
        self.created_at.clone_from(&self.updated_at);
        Ok(())
    }

    pub(super) fn prepare_update(&mut self, original: &Self) -> Result<(), String> {
        self.created_at.clone_from(&original.created_at);
        for (field, value) in &original.fields {
            if is_secret_field(field) && self.kind.field_allowed(field) {
                self.fields
                    .entry(field.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        self.prepare()
    }

    fn prepare(&mut self) -> Result<(), String> {
        if self
            .fields
            .keys()
            .any(|field| !self.kind.field_allowed(field))
        {
            return Err("记录含有不适用的字段".to_owned());
        }
        self.fields.retain(|_, value| !value.is_empty());
        if self.title.trim().is_empty() {
            self.title = self.derived_title();
        }
        self.updated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        Ok(())
    }

    fn derived_title(&self) -> String {
        let (fields, fallback): (&[&str], &str) = match self.kind {
            Kind::Login => (&["platform", "website", "username", "email"], "未命名登录"),
            Kind::Api => (&["provider", "endpoint"], "未命名 API 密钥"),
            Kind::Totp => (&["issuer", "accountName"], "未命名验证码"),
            Kind::Card => (&["platform"], "未命名卡密"),
        };
        fields
            .iter()
            .map(|field| self.field(field).trim())
            .find(|value| !value.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }

    pub(super) fn field(&self, field: &str) -> &str {
        self.fields.get(field).map_or("", String::as_str)
    }

    pub(super) fn same_content(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.title == other.title
            && self.tags == other.tags
            && self.notes == other.notes
            && self.fields == other.fields
            && self.used == other.used
    }

    pub(super) fn matches_query(&self, query: &str) -> bool {
        query.is_empty()
            || [&self.title, &self.notes]
                .into_iter()
                .chain(&self.tags)
                .chain(
                    self.fields
                        .iter()
                        .filter(|(key, _)| !is_secret_field(key))
                        .map(|(_, value)| value),
                )
                .any(|value| value.to_lowercase().contains(query))
    }

    pub(super) fn view(&self, include_secrets: bool) -> Value {
        let fields: BTreeMap<_, _> = self
            .fields
            .iter()
            .filter(|(field, _)| include_secrets || !is_secret_field(field))
            .collect();
        let secret_fields: Vec<_> = self
            .fields
            .iter()
            .filter(|(field, value)| is_secret_field(field) && !value.is_empty())
            .map(|(field, _)| field)
            .collect();
        json!({
            "id": self.id,
            "kind": self.kind,
            "title": self.title,
            "tags": self.tags,
            "notes": self.notes,
            "fields": fields,
            "used": self.used,
            "createdAt": self.created_at,
            "updatedAt": self.updated_at,
            "secretFields": secret_fields,
        })
    }
}

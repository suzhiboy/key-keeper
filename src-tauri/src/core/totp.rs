use super::record::{Kind, Record};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use totp_rs::{Algorithm, Secret, TOTP};

const INVALID_TOTP: &str = "验证码配置无效";

#[derive(Serialize)]
pub(super) struct Code {
    pub(super) code: String,
    remaining: u64,
    period: u64,
}

pub(super) fn generate(record: &Record) -> Result<Code, String> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效".to_owned())?;
    generate_at(record, time.as_secs())
}

fn generate_at(record: &Record, now: u64) -> Result<Code, String> {
    let totp = configuration(record).ok_or_else(|| INVALID_TOTP.to_owned())?;
    Ok(Code {
        code: totp.generate(now),
        remaining: totp.step - now % totp.step,
        period: totp.step,
    })
}

fn configuration(record: &Record) -> Option<TOTP> {
    if !matches!(record.kind, Kind::Login | Kind::Totp) {
        return None;
    }
    let secret = record.field("totpSecret").trim();
    let tok = if secret.starts_with("otpauth://totp/") {
        TOTP::from_url_unchecked(secret).ok()?
    } else {
        let algorithm = match record
            .field("totpAlgorithm")
            .trim()
            .to_ascii_uppercase()
            .as_str()
        {
            "" | "SHA1" => Algorithm::SHA1,
            "SHA256" => Algorithm::SHA256,
            "SHA512" => Algorithm::SHA512,
            _ => return None,
        };
        let digits = parse_or_default(record.field("totpDigits"), 6)?;
        let period = parse_or_default(record.field("totpPeriod"), 30)?;
        let normalized: String = secret
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .map(|character| character.to_ascii_uppercase())
            .collect();
        let secret = Secret::Encoded(normalized).to_bytes().ok()?;
        TOTP::new_unchecked(algorithm, digits, 0, period, secret, None, String::new())
    };
    if tok.step == 0 || !(6..=8).contains(&tok.digits) || tok.secret.is_empty() {
        return None;
    }
    Some(tok)
}

fn parse_or_default<T: std::str::FromStr>(value: &str, default: T) -> Option<T> {
    let value = value.trim();
    if value.is_empty() {
        Some(default)
    } else {
        value.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rfc6238_vectors_for_all_algorithms() {
        let mut record = Record::default();
        record.fields.insert("totpDigits".into(), "8".into());
        for (algorithm, secret, expected) in [
            ("SHA1", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", "94287082"),
            ("SHA256", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA", "46119246"),
            ("SHA512", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA", "90693936"),
        ] {
            record.fields.insert("totpAlgorithm".into(), algorithm.into());
            record.fields.insert("totpSecret".into(), secret.into());
            let result = generate_at(&record, 59).unwrap();
            assert_eq!(result.code, expected);
            assert_eq!(result.period, 30);
            assert_eq!(result.remaining, 1);
        }
    }

    #[test]
    fn rejects_zero_step_and_malformed_otpauth_uri() {
        let mut record = Record::default();
        record.fields.insert(
            "totpSecret".into(),
            "otpauth://totp/mail?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&period=0".into(),
        );
        assert!(generate_at(&record, 59).is_err());
        record
            .fields
            .insert("totpSecret".into(), "otpauth:/mail".into());
        assert!(generate_at(&record, 59).is_err());
        record.fields.insert(
            "totpSecret".into(),
            "otpauth://totp/mail?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8".into(),
        );
        assert_eq!(generate_at(&record, 59).unwrap().code, "94287082");
    }
}

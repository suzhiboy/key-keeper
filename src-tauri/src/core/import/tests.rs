use super::*;
use serde_json::json;
use std::io::Write;

fn open(extension: &str, bytes: &[u8]) -> ImportDocument {
    let mut file = tempfile::Builder::new()
        .suffix(extension)
        .tempfile()
        .unwrap();
    file.write_all(bytes).unwrap();
    ImportDocument::open(file.path()).unwrap()
}

fn request(
    kind: &str,
    delimiter: &str,
    has_header: bool,
    mapping: serde_json::Value,
) -> PreviewRequest {
    serde_json::from_value(json!({
        "kind": kind,
        "encoding": "utf8",
        "delimiter": delimiter,
        "hasHeader": has_header,
        "mapping": mapping,
        "skipDuplicates": true
    }))
    .unwrap()
}

#[test]
fn txt_uses_literal_multicharacter_delimiter_and_keeps_password_whitespace() {
    let document = open(
        ".txt",
        "\u{feff}示例----  0012  \r\n\r\n第二项----00\n".as_bytes(),
    );
    let prepared = document
        .preview(
            &request("login", "----", false, json!({"title": 0, "password": 1})),
            &[],
        )
        .unwrap();

    assert!(prepared.ready);
    assert_eq!(prepared.records[0].title, "示例");
    assert_eq!(prepared.records[0].fields["password"], "  0012  ");
    assert_eq!(prepared.preview.valid_count, 2);
    let preview = serde_json::to_value(prepared.preview).unwrap();
    assert_eq!(preview["rows"][1]["index"], 3);
    assert_eq!(preview["sampleRows"][0][0], "示例");
    assert_ne!(preview["sampleRows"][0][1], "  0012  ");
}

#[test]
fn one_column_gb18030_retains_untrimmed_text_and_reports_empty_mapping() {
    let document = open(".txt", b" \xd6\xd0\xce\xc4 \n");
    let mut options = request("card", "", false, json!({}));
    options.encoding = Encoding::Gb18030;
    let initial = document.preview(&options, &[]).unwrap();
    assert!(!initial.ready);
    assert_eq!(initial.preview.columns.len(), 1);
    assert_eq!(initial.preview.sample_rows[0][0], " 中文 ");
    assert_eq!(initial.preview.valid_count, 0);

    options.mapping.insert("secretCode".into(), 0);
    let prepared = document.preview(&options, &[]).unwrap();
    assert_eq!(prepared.records[0].fields["secretCode"], " 中文 ");
}

#[test]
fn quoted_csv_keeps_delimiters_escaped_quotes_and_embedded_newlines() {
    let document = open(
        ".csv",
        "名称,密码,备注\r\n\"中文,标题\",\"a\"\"b\",\"第一行\r\n第二行\"\r\n".as_bytes(),
    );
    let prepared = document
        .preview(
            &request(
                "login",
                ",",
                true,
                json!({"title": 0, "password": 1, "notes": 2}),
            ),
            &[],
        )
        .unwrap();

    assert_eq!(prepared.preview.columns, ["名称", "密码", "备注"]);
    assert_eq!(prepared.records[0].title, "中文,标题");
    assert_eq!(prepared.records[0].fields["password"], "a\"b");
    assert_eq!(prepared.records[0].notes, "第一行\r\n第二行");
    let preview = serde_json::to_value(prepared.preview).unwrap();
    assert_eq!(preview["rows"][0]["index"], 2);
    assert!(!preview.to_string().contains("a\\\"b"));
}

#[test]
fn malformed_and_uneven_csv_rows_are_errors_without_losing_following_rows() {
    let document = open(".csv", b"title,password\r\nshort\r\nextra,111,ignored\r\nbad,a\"b\r\ngood,0012\r\nopen,\"later\r\n");
    let prepared = document
        .preview(
            &request("login", ",", true, json!({"title": 0, "password": 1})),
            &[],
        )
        .unwrap();

    assert_eq!(prepared.preview.error_count, 4);
    assert_eq!(prepared.preview.valid_count, 1);
    assert_eq!(prepared.records[0].title, "good");
    assert_eq!(prepared.records[0].fields["password"], "0012");
    let preview = serde_json::to_value(prepared.preview).unwrap();
    assert_eq!(preview["rows"][2]["status"], "error");
    assert_eq!(preview["rows"][3]["index"], 5);
}

#[test]
fn xlsx_selects_sheet_and_preserves_text_leading_zeroes() {
    let document = open(".xlsx", include_bytes!("fixtures/cards.xlsx"));
    assert_eq!(document.sheets(), ["Unused", "Cards"]);
    let mut options = request("card", "", true, json!({"title": 1, "secretCode": 2}));
    options.sheet = Some("Cards".into());
    let prepared = document.preview(&options, &[]).unwrap();

    assert_eq!(prepared.preview.columns, ["第 1 列", "Name", "Code"]);
    assert_eq!(prepared.records[0].title, "Sample card");
    assert_eq!(prepared.records[0].fields["secretCode"], "000123");
    assert_eq!(prepared.records[1].fields["secretCode"], " 000987 ");
    let preview = serde_json::to_value(prepared.preview).unwrap();
    assert_eq!(preview["rows"][0]["index"], 4);
    assert_eq!(preview["rows"][1]["index"], 6);
}

#[test]
fn duplicates_compare_complete_business_content_within_file_and_vault() {
    let document = open(
        ".txt",
        b"alice----original\nalice----different\nalice----different\n",
    );
    let mut options = request(
        "login",
        "----",
        false,
        json!({"username": 0, "password": 1}),
    );
    let existing_document = open(".txt", b"alice----original\n");
    let existing = existing_document.preview(&options, &[]).unwrap().records;
    let prepared = document.preview(&options, &existing).unwrap();

    assert_eq!(prepared.preview.duplicate_count, 2);
    assert_eq!(prepared.preview.error_count, 0);
    assert_eq!(prepared.records.len(), 1);
    assert_eq!(prepared.records[0].fields["password"], "different");

    options.skip_duplicates = false;
    assert_eq!(
        document.preview(&options, &existing).unwrap().records.len(),
        3
    );
}

#[test]
fn repeated_titleless_card_codes_are_duplicates() {
    let document = open(".txt", b"00000\n00000\n");
    let options = request("card", "", false, json!({"secretCode": 0}));
    let prepared = document.preview(&options, &[]).unwrap();

    assert_eq!(prepared.preview.duplicate_count, 1);
    assert_eq!(prepared.records.len(), 1);
    assert!(!serde_json::to_string(&prepared.preview)
        .unwrap()
        .contains("00000"));
}

#[test]
fn xls_preserves_text_codes_and_warns_for_numeric_cells() {
    let document = open(".xls", include_bytes!("fixtures/cards.xls"));
    let prepared = document
        .preview(
            &request("card", "", true, json!({"platform":0,"secretCode":1})),
            &[],
        )
        .unwrap();
    assert_eq!(prepared.records[0].fields["secretCode"], "000123");
    let preview = serde_json::to_value(prepared.preview).unwrap();
    assert!(preview["rows"][1]["message"]
        .as_str()
        .unwrap()
        .contains("数值"));
    assert_eq!(preview["validCount"], 2);
}

#[test]
fn card_usage_maps_boolean_values_and_reports_bad_rows() {
    let document = open(
        ".txt",
        "001----已使用\n002----false\n003----unknown\n".as_bytes(),
    );
    let prepared = document
        .preview(
            &request("card", "----", false, json!({"secretCode":0,"used":1})),
            &[],
        )
        .unwrap();
    assert_eq!(prepared.preview.error_count, 1);
    assert!(prepared.records[0].used);
    assert!(!prepared.records[1].used);
}

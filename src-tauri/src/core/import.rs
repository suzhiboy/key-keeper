use super::record::{is_secret_field, Kind, Record};
use calamine::{open_workbook_from_rs, Data, Reader, Sheets, Xls, Xlsx};
use encoding_rs::GB18030;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

#[cfg(test)]
#[path = "import/tests.rs"]
mod tests;

#[derive(Clone, Copy)]
enum Format {
    Txt,
    Csv,
    Xls,
    Xlsx,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Encoding {
    #[default]
    Utf8,
    Gb18030,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreviewRequest {
    kind: Kind,
    #[serde(default)]
    encoding: Encoding,
    #[serde(default)]
    delimiter: String,
    sheet: Option<String>,
    #[serde(default)]
    has_header: bool,
    #[serde(default)]
    mapping: BTreeMap<String, usize>,
    #[serde(default = "default_skip_duplicates")]
    skip_duplicates: bool,
}

fn default_skip_duplicates() -> bool {
    true
}

pub(super) struct ImportDocument {
    format: Format,
    file_name: String,
    sheets: Vec<String>,
    bytes: Zeroizing<Vec<u8>>,
}

pub(super) struct PreparedImport {
    pub(super) preview: ImportPreview,
    pub(super) ready: bool,
    pub(super) records: Vec<Record>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ImportPreview {
    columns: Vec<String>,
    sample_rows: Vec<Vec<String>>,
    rows: Vec<PreviewRow>,
    pub(super) valid_count: usize,
    pub(super) error_count: usize,
    pub(super) duplicate_count: usize,
}

#[derive(Serialize)]
struct PreviewRow {
    index: usize,
    title: String,
    status: RowStatus,
    message: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum RowStatus {
    Valid,
    Duplicate,
    Error,
}

struct SourceRow {
    index: usize,
    cells: Vec<String>,
    error: Option<&'static str>,
    numeric_columns: HashSet<usize>,
}

impl SourceRow {
    fn cell(&self, index: usize) -> &str {
        self.cells.get(index).map_or("", String::as_str)
    }
}

impl Drop for SourceRow {
    fn drop(&mut self) {
        self.cells.zeroize();
    }
}

impl ImportDocument {
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        let file_name = path
            .file_name()
            .ok_or("无法识别所选文件")?
            .to_string_lossy()
            .into_owned();
        let format = match path.extension().and_then(|name| name.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("txt") => Format::Txt,
            Some(extension) if extension.eq_ignore_ascii_case("csv") => Format::Csv,
            Some(extension) if extension.eq_ignore_ascii_case("xls") => Format::Xls,
            Some(extension) if extension.eq_ignore_ascii_case("xlsx") => Format::Xlsx,
            _ => return Err("仅支持 TXT、CSV、XLS 和 XLSX 文件".into()),
        };
        let bytes = Zeroizing::new(fs::read(path).map_err(|_| "无法读取所选文件")?);
        let sheets = match format {
            Format::Xls | Format::Xlsx => open_workbook(format, &bytes)?.sheet_names(),
            Format::Txt | Format::Csv => Vec::new(),
        };
        Ok(Self {
            format,
            file_name,
            sheets,
            bytes,
        })
    }

    pub(super) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(super) fn sheets(&self) -> &[String] {
        &self.sheets
    }

    pub(super) fn preview(
        &self,
        request: &PreviewRequest,
        existing: &[Record],
    ) -> Result<PreparedImport, String> {
        let mut source = self.read_rows(request)?;
        source.retain(|row| row.error.is_some() || row.cells.iter().any(|cell| !cell.is_empty()));
        let mut rows = source.iter();
        let header = if request.has_header {
            rows.next()
        } else {
            None
        };
        if header.is_some_and(|row| row.error.is_some()) {
            return Err("表头格式无效，请修改源文件或关闭首行表头".into());
        }

        let width = header
            .or_else(|| rows.clone().find(|row| row.error.is_none()))
            .map_or(0, |row| row.cells.len());
        let secret_columns = request.secret_columns(width)?;
        let mut preview = ImportPreview {
            columns: (0..width)
                .map(|column| {
                    let name = header.map_or("", |row| row.cell(column)).trim();
                    if name.is_empty() {
                        format!("第 {} 列", column + 1)
                    } else {
                        name.to_owned()
                    }
                })
                .collect(),
            sample_rows: rows
                .clone()
                .filter(|row| row.error.is_none() && row.cells.len() == width)
                .take(5)
                .map(|row| {
                    row.cells
                        .iter()
                        .enumerate()
                        .map(|(column, cell)| {
                            if !cell.is_empty() && secret_columns.contains(&column) {
                                "••••••".to_owned()
                            } else {
                                cell.clone()
                            }
                        })
                        .collect()
                })
                .collect(),
            ..ImportPreview::default()
        };
        let ready = !request.mapping.is_empty();
        let mut records = Vec::new();
        if ready {
            for row in rows {
                let error = row
                    .error
                    .or_else(|| (row.cells.len() != width).then_some("该行的列数与表格不一致"));
                if let Some(message) = error {
                    preview.error_count += 1;
                    preview.rows.push(PreviewRow {
                        index: row.index,
                        title: String::new(),
                        status: RowStatus::Error,
                        message,
                    });
                    continue;
                }
                if request
                    .mapping
                    .get("used")
                    .is_some_and(|&column| parse_used(row.cell(column)).is_none())
                {
                    preview.error_count += 1;
                    preview.rows.push(PreviewRow {
                        index: row.index,
                        title: String::new(),
                        status: RowStatus::Error,
                        message: "已使用状态无效，请填写已使用/未使用或 true/false",
                    });
                    continue;
                }
                let record = request.record_from(row)?;
                let duplicate = request.skip_duplicates
                    && existing
                        .iter()
                        .chain(&records)
                        .any(|other| record.same_content(other));
                let (status, message) = if duplicate {
                    preview.duplicate_count += 1;
                    (RowStatus::Duplicate, "已有相同记录")
                } else {
                    preview.valid_count += 1;
                    let message = if request
                        .mapping
                        .values()
                        .any(|column| row.numeric_columns.contains(column))
                    {
                        "含数值型单元格，请确认前导零和长数字精度"
                    } else {
                        ""
                    };
                    (RowStatus::Valid, message)
                };
                preview.rows.push(PreviewRow {
                    index: row.index,
                    title: record.title.clone(),
                    status,
                    message,
                });
                if !duplicate {
                    records.push(record);
                }
            }
        }
        Ok(PreparedImport {
            preview,
            ready,
            records,
        })
    }

    fn read_rows(&self, request: &PreviewRequest) -> Result<Vec<SourceRow>, String> {
        if matches!(self.format, Format::Xls | Format::Xlsx) {
            let name = request
                .sheet
                .as_ref()
                .or_else(|| self.sheets.first())
                .ok_or("文件没有工作表")?;
            if !self.sheets.contains(name) {
                return Err("找不到所选工作表".into());
            }
            return read_worksheet(self.format, &self.bytes, name);
        }

        let text = match request.encoding {
            Encoding::Utf8 => std::str::from_utf8(
                self.bytes
                    .strip_prefix(b"\xef\xbb\xbf")
                    .unwrap_or(&self.bytes),
            )
            .map(Cow::Borrowed)
            .map_err(|_| "文件不是有效的 UTF-8，请检查编码选项")?,
            Encoding::Gb18030 => GB18030
                .decode_without_bom_handling_and_without_replacement(&self.bytes)
                .ok_or("文件不是有效的 GB18030，请检查编码选项")?,
        };
        let decoded;
        let text = match text {
            Cow::Borrowed(text) => text,
            Cow::Owned(text) => {
                decoded = Zeroizing::new(text);
                &decoded
            }
        };
        if request.delimiter.contains(['\r', '\n']) {
            return Err("分隔符不能包含换行".into());
        }
        match self.format {
            Format::Txt => Ok(read_txt(text, &request.delimiter)),
            Format::Csv => {
                let delimiter = if request.delimiter.is_empty() {
                    ","
                } else {
                    &request.delimiter
                };
                if delimiter.contains('"') {
                    return Err("CSV 分隔符不能包含双引号".into());
                }
                Ok(read_csv(text, delimiter))
            }
            Format::Xls | Format::Xlsx => unreachable!(),
        }
    }
}

impl PreviewRequest {
    fn secret_columns(&self, width: usize) -> Result<HashSet<usize>, String> {
        let mut secret = HashSet::new();
        let mut public = HashSet::new();
        for (field, &column) in &self.mapping {
            if column >= width
                || (!matches!(field.as_str(), "title" | "tags" | "notes")
                    && !(field == "used" && self.kind == Kind::Card)
                    && !self.kind.field_allowed(field))
            {
                return Err("字段映射无效，请重新选择导入列".into());
            }
            if is_secret_field(field) {
                secret.insert(column);
            } else {
                public.insert(column);
            }
        }
        if !secret.is_disjoint(&public) {
            return Err("同一列不能同时映射到保密字段与普通字段".into());
        }
        Ok(secret)
    }

    fn record_from(&self, row: &SourceRow) -> Result<Record, String> {
        let mut record = Record::default();
        record.kind = self.kind;
        for (field, &column) in &self.mapping {
            let value = row.cell(column);
            match field.as_str() {
                "title" => record.title = value.to_owned(),
                "notes" => record.notes = value.to_owned(),
                "used" => record.used = parse_used(value).unwrap_or(false),
                "tags" => {
                    let mut seen = HashSet::new();
                    record.tags = value
                        .split([',', '，'])
                        .map(str::trim)
                        .filter(|tag| !tag.is_empty() && seen.insert(*tag))
                        .map(str::to_owned)
                        .collect();
                }
                _ if !value.is_empty() => {
                    record.fields.insert(field.clone(), value.to_owned());
                }
                _ => {}
            }
        }
        record.prepare_new()?;
        Ok(record)
    }
}

fn parse_used(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "是" | "已使用" => Some(true),
        "" | "false" | "0" | "否" | "未使用" => Some(false),
        _ => None,
    }
}

fn open_workbook(format: Format, bytes: &[u8]) -> Result<Sheets<Cursor<&[u8]>>, String> {
    let data = Cursor::new(bytes);
    let error = |_| "无法读取 Excel 文件".to_owned();
    match format {
        Format::Xls => open_workbook_from_rs::<Xls<_>, _>(data)
            .map(Sheets::Xls)
            .map_err(error),
        Format::Xlsx => open_workbook_from_rs::<Xlsx<_>, _>(data)
            .map(Sheets::Xlsx)
            .map_err(|_| "无法读取 Excel 文件".to_owned()),
        Format::Txt | Format::Csv => unreachable!(),
    }
}

fn read_worksheet(format: Format, bytes: &[u8], name: &str) -> Result<Vec<SourceRow>, String> {
    let range = open_workbook(format, bytes)?
        .worksheet_range(name)
        .map_err(|_| "无法读取所选工作表")?;
    let Some((first_row, first_column)) = range.start() else {
        return Ok(Vec::new());
    };
    Ok(range.rows().enumerate().map(|(offset, row)| {
        let mut cells = vec![String::new(); first_column as usize];
        cells.extend(row.iter().map(|cell| match cell {
            Data::DateTime(value) if !value.is_duration() => {
                let (year, month, day, hour, minute, second, millis) = value.to_ymd_hms_milli();
                if (hour, minute, second, millis) == (0, 0, 0, 0) {
                    format!("{year:04}-{month:02}-{day:02}")
                } else {
                    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}")
                }
            }
            _ => cell.to_string(),
        }));
        let error = row.iter().any(|cell| matches!(cell, Data::Error(_))).then_some("该行包含 Excel 错误单元格");
        let numeric_columns = row.iter().enumerate().filter_map(|(column, cell)| {
            matches!(cell, Data::Int(_) | Data::Float(_)).then_some(first_column as usize + column)
        }).collect();
        SourceRow { index: first_row as usize + offset + 1, cells, error, numeric_columns }
    }).collect())
}

fn read_txt(text: &str, delimiter: &str) -> Vec<SourceRow> {
    text.split_inclusive(['\r', '\n'])
        .scan((0, false), |(line, previous_cr), segment| {
            let followed_cr = *previous_cr;
            *previous_cr = segment.ends_with('\r');
            if followed_cr && segment == "\n" {
                return Some(None);
            }
            *line += 1;
            let text = segment.strip_suffix(['\r', '\n']).unwrap_or(segment);
            let cells = if delimiter.is_empty() {
                vec![text.to_owned()]
            } else {
                text.split(delimiter).map(str::to_owned).collect()
            };
            Some(Some(SourceRow {
                numeric_columns: HashSet::new(),
                index: *line,
                cells,
                error: None,
            }))
        })
        .flatten()
        .collect()
}

#[derive(PartialEq, Eq)]
enum CsvField {
    Start,
    Unquoted,
    Quoted,
    Closed,
}

fn read_csv(mut text: &str, delimiter: &str) -> Vec<SourceRow> {
    let mut rows = Vec::new();
    let mut cells = Vec::new();
    let mut field = String::new();
    let mut state = CsvField::Start;
    let mut line = 1;
    let mut index = 1;
    let mut error = None;
    while let Some(character) = text.chars().next() {
        let tail = &text[character.len_utf8()..];
        if character == '\n' || character == '\r' {
            let newline = if character == '\r' && tail.starts_with('\n') {
                "\r\n"
            } else {
                &text[..1]
            };
            text = &text[newline.len()..];
            line += 1;
            if state == CsvField::Quoted {
                field.push_str(newline);
                continue;
            }
            cells.push(std::mem::take(&mut field));
            rows.push(SourceRow {
                numeric_columns: HashSet::new(),
                index,
                cells: std::mem::take(&mut cells),
                error: error.take(),
            });
            index = line;
            state = CsvField::Start;
        } else if error.is_some() {
            text = tail;
        } else if state != CsvField::Quoted && text.starts_with(delimiter) {
            cells.push(std::mem::take(&mut field));
            text = &text[delimiter.len()..];
            state = CsvField::Start;
        } else if character == '"' && state == CsvField::Quoted {
            if let Some(rest) = tail.strip_prefix('"') {
                field.push('"');
                text = rest;
            } else {
                state = CsvField::Closed;
                text = tail;
            }
        } else {
            match (&state, character) {
                (CsvField::Start, '"') => state = CsvField::Quoted,
                (CsvField::Unquoted, '"') | (CsvField::Closed, _) => {
                    error = Some("CSV 引号格式无效")
                }
                (CsvField::Start | CsvField::Unquoted, _) => {
                    state = CsvField::Unquoted;
                    field.push(character);
                }
                (CsvField::Quoted, _) => field.push(character),
            }
            text = tail;
        }
    }
    if state == CsvField::Quoted {
        error = Some("CSV 字段缺少结束引号");
    }
    if !cells.is_empty() || state != CsvField::Start || error.is_some() {
        cells.push(field);
        rows.push(SourceRow {
            numeric_columns: HashSet::new(),
            index,
            cells,
            error,
        });
    }
    rows
}

// OOXML Viewer - Tauri backend
// 负责打开 OOXML (docx/xlsx/pptx) 文件、列出内部 zip 条目、读取/更新单个条目。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// 内存 zip 归档类型（读取整个文件到内存后解析）
type Archive = ZipArchive<Cursor<Vec<u8>>>;

#[derive(Serialize, Clone)]
struct ZipEntry {
    /// zip 内完整路径，如 "word/document.xml"
    path: String,
    /// 文件名（不含目录）
    name: String,
    /// 显示名（如 xlsx 工作表名称 "Sheet1"），无特殊显示名时为 None
    display_name: Option<String>,
    is_dir: bool,
    /// 解压后大小
    size: u64,
    /// 压缩后大小
    compressed_size: u64,
}

#[derive(Serialize)]
struct OpenResult {
    file_name: String,
    file_path: String,
    file_size: u64,
    /// docx | xlsx | pptx | ooxml
    kind: String,
    entries: Vec<ZipEntry>,
}

#[derive(Serialize)]
struct ReadEntryResult {
    /// "text" | "binary"
    kind: String,
    content: String,
    size: u64,
    /// 二进制文件是否仅预览了前 N 字节
    preview_truncated: bool,
}

#[derive(Serialize, Clone)]
struct BackupInfo {
    /// 备份文件绝对路径
    backup_path: String,
    /// 备份文件名（如 ".sample.docx.ooxml-backup-123.bak"）
    file_name: String,
    size: u64,
    /// 备份文件修改时间（毫秒时间戳）
    modified_ms: u64,
}

fn detect_kind(file_path: &str) -> String {
    let lower = file_path.to_lowercase();
    if lower.ends_with(".docx") || lower.ends_with(".docm") {
        "docx".into()
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") {
        "xlsx".into()
    } else if lower.ends_with(".pptx") || lower.ends_with(".pptm") {
        "pptx".into()
    } else {
        "ooxml".into()
    }
}

/// 判定 zip 内条目是否属于可编辑的文本文件
fn is_textual_entry(path: &str) -> bool {
    if path.ends_with('/') {
        return false;
    }
    let lower = path.to_lowercase();
    [
        ".xml",
        ".rels",
        ".json",
        ".txt",
        ".csv",
        ".tsv",
        ".js",
        ".css",
        ".html",
        ".htm",
        ".svg",
        ".md",
        ".properties",
        ".dtd",
        ".xsd",
        ".xsl",
        ".yml",
        ".yaml",
        ".rng",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// 判定 zip 内条目是否属于可预览的图片文件（svg 保留为文本编辑）
fn is_image_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".ico", ".tif", ".tiff",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn hex_dump(data: &[u8], max_bytes: usize) -> (String, bool) {
    let show = &data[..data.len().min(max_bytes)];
    let mut out = String::with_capacity(show.len() * 3 + 32);
    for (i, chunk) in show.chunks(16).enumerate() {
        out.push_str(&format!("{:08x}  ", i * 16));
        for b in chunk {
            out.push_str(&format!("{:02x} ", b));
        }
        for _ in chunk.len()..16 {
            out.push_str("   ");
        }
        out.push_str(" |");
        for b in chunk {
            let c = if b.is_ascii_graphic() || *b == b' ' {
                *b as char
            } else {
                '.'
            };
            out.push(c);
        }
        out.push_str("|\n");
    }
    (out, data.len() > max_bytes)
}

fn load_archive(file_path: &str) -> Result<(Vec<u8>, Archive), String> {
    let bytes = fs::read(file_path).map_err(|e| format!("Failed to read file: {e}"))?;
    let archive = ZipArchive::new(Cursor::new(bytes.clone()))
        .map_err(|e| format!("Not a valid OOXML/ZIP file: {e}"))?;
    Ok((bytes, archive))
}

fn read_utf8_str(archive: &mut Archive, name: &str) -> Result<String, String> {
    let mut f = archive.by_name(name).map_err(|e| e.to_string())?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}

/// 在字符串中提取 `name="value"` 形式的属性值
fn attr(xml: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let idx = xml.find(&pat)?;
    let rest = &xml[idx + pat.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// 解析 workbook.xml 中的 <sheet name="..." r:id="..."/> 列表
fn parse_sheets(wb: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(start) = wb[search..].find("<sheet") {
        let abs = search + start;
        let end = wb[abs..].find('>').unwrap_or(wb.len() - abs);
        let tag = &wb[abs..abs + end + 1];
        if let Some(name) = attr(tag, "name") {
            let rid = attr(tag, "r:id").unwrap_or_default();
            out.push((name, rid));
        }
        search = abs + end + 1;
    }
    out
}

/// 解析 .rels 文件中的 <Relationship Id="..." Target="..."/> 列表
fn parse_rels(rels: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut search = 0;
    while let Some(start) = rels[search..].find("<Relationship") {
        let abs = search + start;
        let end = rels[abs..].find('>').unwrap_or(rels.len() - abs);
        let tag = &rels[abs..abs + end + 1];
        let id = attr(tag, "Id").unwrap_or_default();
        let target = attr(tag, "Target").unwrap_or_default();
        if !id.is_empty() && !target.is_empty() {
            map.insert(id, target);
        }
        search = abs + end + 1;
    }
    map
}

/// 把 .rels 中的相对 Target 解析为包内完整路径
fn resolve_target(target: &str) -> String {
    let t = target.trim_start_matches('/');
    if t.starts_with("xl/") {
        t.to_string()
    } else {
        format!("xl/{t}")
    }
}

/// 提取 xlsx 工作表名称 → 工作表 XML 路径 的映射
/// （解析 xl/workbook.xml 与 xl/_rels/workbook.xml.rels；无法解析时回退按序号匹配）
fn extract_sheet_names(archive: &mut Archive) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let wb = match read_utf8_str(archive, "xl/workbook.xml") {
        Ok(s) => s,
        Err(_) => return map,
    };
    let sheets = parse_sheets(&wb);
    if sheets.is_empty() {
        return map;
    }

    let rid_target = read_utf8_str(archive, "xl/_rels/workbook.xml.rels")
        .map(|s| parse_rels(&s))
        .unwrap_or_default();

    let mut fallback_no = 1;
    for (name, rid) in sheets {
        let path = rid_target
            .get(&rid)
            .map(|t| resolve_target(t))
            .unwrap_or_else(|| {
                let p = format!("xl/worksheets/sheet{fallback_no}.xml");
                fallback_no += 1;
                p
            });
        map.insert(path, name);
    }
    map
}

/// 打开一个 OOXML 文件，返回内部全部条目清单。
#[tauri::command]
fn open_ooxml(file_path: String) -> Result<OpenResult, String> {
    let (_, mut archive) = load_archive(&file_path)?;

    // xlsx：建立 工作表 XML 路径 → 工作表名称 映射，用于列表中直接显示表名
    let sheet_names = extract_sheet_names(&mut archive);

    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let path = file.name().to_string();
        let name = Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        entries.push(ZipEntry {
            display_name: sheet_names.get(&path).cloned(),
            is_dir: file.is_dir(),
            size: file.size(),
            compressed_size: file.compressed_size(),
            path,
            name,
        });
    }

    let file_size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
    let file_name = Path::new(&file_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.clone());
    let kind = detect_kind(&file_path);

    Ok(OpenResult {
        file_name,
        file_path,
        file_size,
        kind,
        entries,
    })
}

/// 读取 zip 内指定条目。文本类条目返回原文；二进制条目返回十六进制预览。
#[tauri::command]
fn read_entry(file_path: String, entry_path: String) -> Result<ReadEntryResult, String> {
    let (_, mut archive) = load_archive(&file_path)?;

    let mut file = archive
        .by_name(&entry_path)
        .map_err(|e| format!("Cannot read entry [{entry_path}]: {e}"))?;
    if file.is_dir() {
        return Err("This is a directory entry".into());
    }
    let size = file.size();
    let mut data = Vec::with_capacity(size.min(64 * 1024 * 1024) as usize);
    file.read_to_end(&mut data)
        .map_err(|e| format!("Extraction failed: {e}"))?;
    drop(file);

    if is_textual_entry(&entry_path) {
        if let Ok(s) = String::from_utf8(data.clone()) {
            return Ok(ReadEntryResult {
                kind: "text".into(),
                content: s,
                size,
                preview_truncated: false,
            });
        }
        // 扩展名是文本类但内容不是合法 UTF-8，降级为二进制预览
    }

    let (content, truncated) = hex_dump(&data, 2048);
    Ok(ReadEntryResult {
        kind: "binary".into(),
        content,
        size,
        preview_truncated: truncated,
    })
}

/// 读取 zip 内图片条目，返回 base64 编码内容（前端以 data URL 预览）。
#[tauri::command]
fn read_image(file_path: String, entry_path: String) -> Result<String, String> {
    if !is_image_path(&entry_path) {
        return Err(format!("Format not supported for preview: {entry_path}"));
    }
    let (_, mut archive) = load_archive(&file_path)?;
    let mut file = archive
        .by_name(&entry_path)
        .map_err(|e| format!("Cannot read entry [{entry_path}]: {e}"))?;
    if file.is_dir() {
        return Err("This is a directory entry".into());
    }
    let size = file.size();
    let mut data = Vec::with_capacity(size.min(128 * 1024 * 1024) as usize);
    file.read_to_end(&mut data)
        .map_err(|e| format!("Extraction failed: {e}"))?;
    drop(file);
    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

/// 将 io 错误转为友好提示（macOS TCC 会拒绝未签名进程访问“桌面/文稿/下载”等受保护目录）
fn io_err(action: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        format!(
            "{action}: permission denied. macOS may block unsigned apps from accessing protected folders such as Desktop/Documents/Downloads. Please choose another folder, or build a signed .app."
        )
    } else {
        format!("{action}: {e}")
    }
}

/// 原子写入：先写同目录临时文件，再 rename 覆盖原文件
fn atomic_write(file_path: &str, out: &[u8]) -> Result<(), String> {
    let dir = Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let file_stem = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ooxml".into());
    let tmp_path = dir.join(format!(
        ".{}.ooxml-save-{}-{}.tmp",
        file_stem,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    if let Err(e) = fs::write(&tmp_path, out) {
        let _ = fs::remove_file(&tmp_path);
        return Err(io_err("Failed to write temporary file", &e));
    }
    if let Err(e) = fs::rename(&tmp_path, file_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(io_err("Failed to replace the original file", &e));
    }
    Ok(())
}

/// 备份文件前缀：.{原文件名}.ooxml-backup-
fn backup_prefix(file_path: &str) -> String {
    let name = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ooxml".into());
    format!(".{name}.ooxml-backup-")
}

/// 列出原文件同目录下的全部备份（按修改时间倒序）
fn list_backups_impl(file_path: &str) -> Result<Vec<BackupInfo>, String> {
    let dir = Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let prefix = backup_prefix(file_path);
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| io_err("Failed to read backup directory", &e))?;
    for ent in entries.flatten() {
        let p = ent.path();
        let Some(fname) = p.file_name().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        if !fname.starts_with(&prefix) || !fname.ends_with(".bak") {
            continue;
        }
        let meta = ent.metadata().ok();
        let modified_ms = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(BackupInfo {
            backup_path: p.to_string_lossy().into_owned(),
            file_name: fname,
            size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            modified_ms,
        });
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    Ok(out)
}

/// 覆盖写盘前创建原文件备份（仅保留最新一份）。
/// 保存后删除/修改错了，可通过“恢复上一次保存”回退。
fn backup_file(file_path: &str) -> Result<(), String> {
    let bytes =
        fs::read(file_path).map_err(|e| format!("Failed to read the original file: {e}"))?;
    let dir = Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let prefix = backup_prefix(file_path);
    let backup_path = dir.join(format!(
        "{prefix}{}-{}.bak",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&backup_path, &bytes).map_err(|e| io_err("Failed to create backup", &e))?;
    // 清理旧备份，仅保留最新一份
    for b in list_backups_impl(file_path)? {
        if b.backup_path != backup_path.to_string_lossy() {
            let _ = fs::remove_file(&b.backup_path);
        }
    }
    Ok(())
}

/// 列出当前文件的备份（用于前端“恢复”按钮的可用状态）。
#[tauri::command]
fn list_backups(file_path: String) -> Result<Vec<BackupInfo>, String> {
    list_backups_impl(&file_path)
}

/// 用指定备份覆盖当前文件（复制而非移动，备份保留可再次恢复）。
#[tauri::command]
fn restore_backup(file_path: String, backup_path: String) -> Result<u64, String> {
    // 安全校验：备份必须位于原文件同目录，且文件名符合本文件的备份规则
    let target_dir = Path::new(&file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let bp = Path::new(&backup_path);
    if bp.parent().unwrap_or_else(|| Path::new("")) != target_dir {
        return Err("Backup file is not in the same directory as the original file".into());
    }
    let fname = bp
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !fname.starts_with(&backup_prefix(&file_path)) || !fname.ends_with(".bak") {
        return Err("Invalid backup file".into());
    }

    let bytes = fs::read(bp).map_err(|e| format!("Failed to read backup file: {e}"))?;
    atomic_write(&file_path, &bytes)?;
    Ok(bytes.len() as u64)
}

/// 单条条目变更：path + 新内容（新增/更新共用）
#[derive(Deserialize)]
struct EntryChange {
    path: String,
    content: String,
}

/// 一次性应用增/删/改变更并原子写回原文件。
/// 未涉及的条目以原始压缩数据直接复制，保证 OOXML 结构最小变动。
#[tauri::command]
fn save_changes(
    file_path: String,
    adds: Vec<EntryChange>,
    updates: Vec<EntryChange>,
    deletes: Vec<String>,
) -> Result<u64, String> {
    for c in adds.iter().chain(updates.iter()) {
        if !is_textual_entry(&c.path) {
            return Err(format!(
                "Only text files can be written (.xml / .rels / .json, etc.): {}",
                c.path
            ));
        }
    }

    let (_, mut archive) = load_archive(&file_path)?;

    let mut add_map: HashMap<String, String> =
        adds.into_iter().map(|c| (c.path, c.content)).collect();
    let update_map: HashMap<String, String> =
        updates.into_iter().map(|c| (c.path, c.content)).collect();
    let delete_set: std::collections::HashSet<String> = deletes.into_iter().collect();

    let mut out: Vec<u8> = Vec::with_capacity(4096);
    let mut written = 0u64;

    {
        let mut writer = ZipWriter::new(Cursor::new(&mut out));
        for i in 0..archive.len() {
            let file = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = file.name().to_string();
            if delete_set.contains(&name) {
                continue;
            }
            // 已有条目被更新（或新增路径与已有条目同名，视为覆盖）
            if let Some(content) = add_map
                .remove(&name)
                .or_else(|| update_map.get(&name).cloned())
            {
                let opts = SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .last_modified_time(file.last_modified().unwrap_or_default());
                writer.start_file(name, opts).map_err(|e| e.to_string())?;
                writer
                    .write_all(content.as_bytes())
                    .map_err(|e| e.to_string())?;
                written += content.len() as u64;
                continue;
            }
            writer.raw_copy_file(file).map_err(|e| e.to_string())?;
        }
        // 追加剩余的新增条目（全新路径）
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (path, content) in add_map {
            writer.start_file(path, opts).map_err(|e| e.to_string())?;
            writer
                .write_all(content.as_bytes())
                .map_err(|e| e.to_string())?;
            written += content.len() as u64;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }

    // 先备份原文件（保存后可通过“恢复上一次保存”回退删除/修改），再原子覆盖写盘
    backup_file(&file_path)?;
    atomic_write(&file_path, &out)?;
    Ok(written)
}

/// 提取 zip 内指定条目到目标目录（保持相对路径，含路径穿越防护）。
#[tauri::command]
fn extract_entry(file_path: String, entry_path: String, dest_dir: String) -> Result<u64, String> {
    if entry_path.ends_with('/') {
        return Err("Cannot extract a directory entry".into());
    }

    // 安全校验：拒绝绝对路径与包含 ".." 的条目路径，防止路径穿越
    let rel = Path::new(&entry_path);
    if rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err("Invalid entry path".into());
    }
    let dest = Path::new(&dest_dir).join(rel);
    if !dest.starts_with(&dest_dir) {
        return Err("Destination path out of bounds".into());
    }

    // 读取条目数据（文本/二进制均可）
    let (_, mut archive) = load_archive(&file_path)?;
    let mut file = archive
        .by_name(&entry_path)
        .map_err(|e| format!("Cannot read entry [{entry_path}]: {e}"))?;
    if file.is_dir() {
        return Err("Cannot extract a directory entry".into());
    }
    let size = file.size();
    let mut data = Vec::with_capacity(size.min(256 * 1024 * 1024) as usize);
    file.read_to_end(&mut data)
        .map_err(|e| format!("Extraction failed: {e}"))?;
    drop(file);

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err("Failed to create directory", &e))?;
    }
    std::fs::write(&dest, &data).map_err(|e| io_err("Failed to write file", &e))?;
    Ok(data.len() as u64)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_ooxml,
            read_entry,
            read_image,
            save_changes,
            extract_entry,
            list_backups,
            restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_zip() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut w = ZipWriter::new(Cursor::new(&mut out));
            let opts = SimpleFileOptions::default();
            w.start_file("[Content_Types].xml", opts).unwrap();
            w.write_all(b"<Types/>").unwrap();
            w.start_file("word/document.xml", opts).unwrap();
            w.write_all(b"<w:document/>").unwrap();
            w.start_file("word/media/img1.png", opts).unwrap();
            w.write_all(&[0x89, 0x50, 0x4e, 0x47, 0x00, 0xff, 0xfe, 0xfd])
                .unwrap();
            w.finish().unwrap();
        }
        out
    }

    #[test]
    fn test_open_read_update_roundtrip() {
        let dir = std::env::temp_dir().join("ooxml_viewer_test_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.docx");
        std::fs::write(&path, make_sample_zip()).unwrap();

        // open
        let opened = open_ooxml(path.to_str().unwrap().to_string()).unwrap();
        assert_eq!(opened.kind, "docx");
        assert_eq!(opened.entries.len(), 3);

        // read text
        let r = read_entry(
            path.to_str().unwrap().to_string(),
            "word/document.xml".into(),
        )
        .unwrap();
        assert_eq!(r.kind, "text");
        assert_eq!(r.content, "<w:document/>");

        // read binary -> hex preview
        let b = read_entry(
            path.to_str().unwrap().to_string(),
            "word/media/img1.png".into(),
        )
        .unwrap();
        assert_eq!(b.kind, "binary");
        assert!(b.content.contains("89 50 4e 47"));

        // 一次性批量变更：更新 document.xml、新增 item1.xml、删除 [Content_Types].xml
        let written = save_changes(
            path.to_str().unwrap().to_string(),
            vec![EntryChange {
                path: "word/customXml/item1.xml".into(),
                content: "<item>ok</item>".into(),
            }],
            vec![EntryChange {
                path: "word/document.xml".into(),
                content: "<w:document><w:body/></w:document>".into(),
            }],
            vec!["[Content_Types].xml".into()],
        )
        .unwrap();
        assert!(written > 0);

        // re-open 验证结果
        let opened2 = open_ooxml(path.to_str().unwrap().to_string()).unwrap();
        assert_eq!(opened2.entries.len(), 3);

        let r2 = read_entry(
            path.to_str().unwrap().to_string(),
            "word/document.xml".into(),
        )
        .unwrap();
        assert_eq!(r2.content, "<w:document><w:body/></w:document>");

        let r3 = read_entry(
            path.to_str().unwrap().to_string(),
            "word/customXml/item1.xml".into(),
        )
        .unwrap();
        assert_eq!(r3.content, "<item>ok</item>");

        // 被删除的条目不存在
        assert!(read_entry(
            path.to_str().unwrap().to_string(),
            "[Content_Types].xml".into(),
        )
        .is_err());

        // 未修改条目保持字节不变
        let b2 = read_entry(
            path.to_str().unwrap().to_string(),
            "word/media/img1.png".into(),
        )
        .unwrap();
        assert_eq!(b2.kind, "binary");
        assert!(b2.content.contains("89 50 4e 47"));

        // 二进制条目禁止写入
        let err = save_changes(
            path.to_str().unwrap().to_string(),
            vec![],
            vec![EntryChange {
                path: "word/media/img1.png".into(),
                content: "hack".into(),
            }],
            vec![],
        )
        .unwrap_err();
        assert!(err.contains("Only text files can be written"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn make_sample_xlsx() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut w = ZipWriter::new(Cursor::new(&mut out));
            let opts = SimpleFileOptions::default();
            w.start_file("xl/workbook.xml", opts).unwrap();
            w.write_all(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/><sheet name="数据明细" sheetId="2" r:id="rId2"/></sheets>
</workbook>"#
                .as_bytes(),
            )
            .unwrap();
            w.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
            w.write_all(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#
                .as_bytes(),
            )
            .unwrap();
            w.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            w.write_all(b"<worksheet/>").unwrap();
            w.start_file("xl/worksheets/sheet2.xml", opts).unwrap();
            w.write_all(b"<worksheet/>").unwrap();
            w.finish().unwrap();
        }
        out
    }

    #[test]
    fn test_sheet_display_names() {
        let dir = std::env::temp_dir().join("ooxml_viewer_test_sheets");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.xlsx");
        std::fs::write(&path, make_sample_xlsx()).unwrap();

        let opened = open_ooxml(path.to_str().unwrap().to_string()).unwrap();
        assert_eq!(opened.kind, "xlsx");

        let sheet1 = opened
            .entries
            .iter()
            .find(|e| e.path == "xl/worksheets/sheet1.xml")
            .unwrap();
        assert_eq!(sheet1.display_name.as_deref(), Some("Sheet1"));

        let sheet2 = opened
            .entries
            .iter()
            .find(|e| e.path == "xl/worksheets/sheet2.xml")
            .unwrap();
        assert_eq!(sheet2.display_name.as_deref(), Some("数据明细"));

        // 无工作表的包（如 docx）display_name 应为 None
        let docx = dir.join("plain.docx");
        std::fs::write(&docx, make_sample_zip()).unwrap();
        let opened2 = open_ooxml(docx.to_str().unwrap().to_string()).unwrap();
        assert!(opened2.entries.iter().all(|e| e.display_name.is_none()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_extract_entry() {
        let dir = std::env::temp_dir().join("ooxml_viewer_test_extract");
        std::fs::create_dir_all(&dir).unwrap();
        let docx = dir.join("sample.docx");
        std::fs::write(&docx, make_sample_zip()).unwrap();
        let dest = dir.join("out");

        // 提取文本条目（保持相对路径）
        let n = extract_entry(
            docx.to_str().unwrap().to_string(),
            "word/document.xml".into(),
            dest.to_str().unwrap().to_string(),
        )
        .unwrap();
        assert_eq!(n, "<w:document/>".len() as u64);
        let written = std::fs::read_to_string(dest.join("word/document.xml")).unwrap();
        assert_eq!(written, "<w:document/>");

        // 提取二进制条目
        extract_entry(
            docx.to_str().unwrap().to_string(),
            "word/media/img1.png".into(),
            dest.to_str().unwrap().to_string(),
        )
        .unwrap();
        assert!(dest.join("word/media/img1.png").exists());

        // 路径穿越防护
        let err = extract_entry(
            docx.to_str().unwrap().to_string(),
            "../evil.xml".into(),
            dest.to_str().unwrap().to_string(),
        )
        .unwrap_err();
        assert!(err.contains("Invalid entry path"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_read_image() {
        let dir = std::env::temp_dir().join("ooxml_viewer_test_image");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.docx");
        std::fs::write(&path, make_sample_zip()).unwrap();
        let fp = path.to_str().unwrap().to_string();

        // PNG 魔数 89 50 4E 47（任意字节 00 FF FE FD）→ base64 "iVBORwD/v0="
        let b64 = read_image(fp.clone(), "word/media/img1.png".into()).unwrap();
        assert!(b64.starts_with("iVBORwD"));

        // 非图片条目被拒绝
        let err = read_image(fp.clone(), "word/document.xml".into()).unwrap_err();
        assert!(err.contains("not supported for preview"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_backup_restore() {
        let dir = std::env::temp_dir().join("ooxml_viewer_test_backup");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.docx");
        std::fs::write(&path, make_sample_zip()).unwrap();
        let fp = path.to_str().unwrap().to_string();

        // 保存前：无备份
        assert!(list_backups(fp.clone()).unwrap().is_empty());

        // 删除 word/document.xml 并保存 → 生成备份，且原文件已无该条目
        save_changes(fp.clone(), vec![], vec![], vec!["word/document.xml".into()]).unwrap();
        let backups = list_backups(fp.clone()).unwrap();
        assert_eq!(backups.len(), 1);
        assert!(read_entry(fp.clone(), "word/document.xml".into()).is_err());

        // 恢复 → 条目回来且内容正确
        let n = restore_backup(fp.clone(), backups[0].backup_path.clone()).unwrap();
        assert!(n > 0);
        let r = read_entry(fp.clone(), "word/document.xml".into()).unwrap();
        assert_eq!(r.content, "<w:document/>");

        // 再次保存：旧备份被清理，仍只保留最新一份
        save_changes(fp.clone(), vec![], vec![], vec!["word/document.xml".into()]).unwrap();
        let backups2 = list_backups(fp.clone()).unwrap();
        assert_eq!(backups2.len(), 1);

        // 非法备份路径（目录越界）被拒绝
        let evil = dir.join("..").join("evil.bak");
        let err = restore_backup(fp.clone(), evil.to_str().unwrap().to_string()).unwrap_err();
        assert!(err.contains("Backup"));

        // 同目录但文件名不符的备份被拒绝
        let wrong = dir.join("not-a-backup.bak");
        std::fs::write(&wrong, b"x").unwrap();
        let err2 = restore_backup(fp.clone(), wrong.to_str().unwrap().to_string()).unwrap_err();
        assert!(err2.contains("Invalid"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use encoding_rs::{GBK, UTF_16BE, UTF_16LE, UTF_8};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileVersion {
    size: u64,
    modified_ns: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkdownDocument {
    path: String,
    name: String,
    content: String,
    encoding: String,
    version: FileVersion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkdownWriteResult {
    path: String,
    name: String,
    encoding: String,
    version: FileVersion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkdownFileStatus {
    path: String,
    version: Option<FileVersion>,
}

fn detect_encoding(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        "utf-8-bom"
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        "utf-16le-bom"
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        "utf-16be-bom"
    } else if std::str::from_utf8(bytes).is_ok() {
        "utf-8"
    } else {
        "gbk"
    }
}

pub(crate) fn file_operation_error(action: &str, error: &std::io::Error) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::NotFound => "文件不存在或已被移动",
        std::io::ErrorKind::PermissionDenied => "没有访问权限，请检查文件权限",
        std::io::ErrorKind::InvalidInput => "文件路径无效",
        _ => "系统未能完成操作，请检查文件状态、权限和磁盘空间",
    };
    format!("{action}失败：{reason}")
}

fn file_version(path: &Path) -> Result<FileVersion, String> {
    let metadata =
        fs::metadata(path).map_err(|error| file_operation_error("检查文件状态", &error))?;
    let modified = metadata
        .modified()
        .map_err(|error| file_operation_error("读取文件修改时间", &error))?;
    let modified_ns = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();

    Ok(FileVersion {
        size: metadata.len(),
        modified_ns,
    })
}

fn file_version_conflict_error(
    current: Option<&FileVersion>,
    expected: Option<&FileVersion>,
) -> String {
    match (current, expected) {
        (None, Some(_)) => "文件已被删除或移动，请另存为".into(),
        (Some(_), None) => "目标文件已由其他程序创建，请重新选择保存位置".into(),
        _ => "文件已被其他程序修改，请重新载入或另存为".into(),
    }
}

pub(crate) fn decode_markdown(bytes: &[u8]) -> Result<String, String> {
    if let Some(text) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return std::str::from_utf8(text)
            .map(str::to_owned)
            .map_err(|_| "UTF-8 BOM 后包含无效字节".to_owned());
    }
    if let Some(text) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (decoded, had_errors) = UTF_16LE.decode_without_bom_handling(text);
        return if had_errors {
            Err("UTF-16LE 内容不完整或无效".into())
        } else {
            Ok(decoded.into_owned())
        };
    }
    if let Some(text) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let (decoded, had_errors) = UTF_16BE.decode_without_bom_handling(text);
        return if had_errors {
            Err("UTF-16BE 内容不完整或无效".into())
        } else {
            Ok(decoded.into_owned())
        };
    }

    let (utf8, had_errors) = UTF_8.decode_without_bom_handling(bytes);
    if !had_errors {
        return Ok(utf8.into_owned());
    }

    let (gbk, had_errors) = GBK.decode_without_bom_handling(bytes);
    if had_errors {
        Err("无法识别文件编码，支持 UTF-8、UTF-16 和 GBK".into())
    } else {
        Ok(gbk.into_owned())
    }
}

#[tauri::command]
fn read_markdown(path: String) -> Result<MarkdownDocument, String> {
    let file = Path::new(&path)
        .canonicalize()
        .map_err(|error| file_operation_error("打开文件", &error))?;
    let extension = file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "md" && extension != "markdown" {
        return Err("请选择 .md 或 .markdown 文件".into());
    }
    if file.is_dir() {
        return Err("所选路径是文件夹，请选择 Markdown 文件".into());
    }

    let before_read = file_version(&file)?;
    let bytes = fs::read(&file).map_err(|error| file_operation_error("读取文件", &error))?;
    let version = file_version(&file)?;
    if before_read != version {
        return Err("文件读取期间发生变化，请重新打开".into());
    }
    let name = file
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Markdown 文档")
        .to_owned();

    let encoding = detect_encoding(&bytes).to_owned();
    let content = decode_markdown(&bytes).map_err(|error| format!("无法解码文件内容：{error}"))?;
    Ok(MarkdownDocument {
        path: file.to_string_lossy().into_owned(),
        name,
        content,
        encoding,
        version,
    })
}

#[tauri::command]
fn get_markdown_version(path: String) -> Result<MarkdownFileStatus, String> {
    let requested_path = Path::new(&path);
    let parent = requested_path
        .parent()
        .ok_or_else(|| "无法定位文件夹".to_owned())?
        .canonicalize()
        .map_err(|error| file_operation_error("访问目标文件夹", &error))?;
    let file_name = requested_path
        .file_name()
        .ok_or_else(|| "文件名无效".to_owned())?;
    let requested_target = parent.join(file_name);
    validate_markdown_path(&requested_target)?;
    let (target, version) = match requested_target.canonicalize() {
        Ok(target) => {
            let version = file_version(&target)?;
            (target, Some(version))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (requested_target, None),
        Err(error) => return Err(file_operation_error("检查文件状态", &error)),
    };
    validate_markdown_path(&target)?;

    Ok(MarkdownFileStatus {
        path: target.to_string_lossy().into_owned(),
        version,
    })
}

fn encode_markdown(content: &str, encoding: &str) -> Result<Vec<u8>, String> {
    match encoding {
        "utf-8" => Ok(content.as_bytes().to_vec()),
        "utf-8-bom" => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(content.as_bytes());
            Ok(bytes)
        }
        "utf-16le-bom" | "utf-16be-bom" => {
            let little_endian = encoding == "utf-16le-bom";
            let mut bytes = if little_endian {
                vec![0xFF, 0xFE]
            } else {
                vec![0xFE, 0xFF]
            };
            for unit in content.encode_utf16() {
                let encoded_unit = if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend_from_slice(&encoded_unit);
            }
            Ok(bytes)
        }
        "gbk" => {
            let (encoded, _, had_errors) = GBK.encode(content);
            if had_errors {
                Err("文件编码无法表示新增字符，请改用 UTF-8 保存".into())
            } else {
                Ok(encoded.into_owned())
            }
        }
        _ => Err("不支持的文件编码".into()),
    }
}

fn validate_markdown_path(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "md" && extension != "markdown" {
        return Err("请选择 .md 或 .markdown 文件".into());
    }
    Ok(())
}

fn write_markdown_atomically(
    path: &Path,
    bytes: &[u8],
    expected_version: Option<&FileVersion>,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "无法定位文件夹".to_owned())?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "文件名无效".to_owned())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));

    let original_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| file_operation_error("创建保存临时文件", &error))?;
        file.write_all(bytes)
            .map_err(|error| file_operation_error("写入文件", &error))?;
        file.sync_all()
            .map_err(|error| file_operation_error("完成文件写入", &error))?;
        drop(file);

        if let Some(permissions) = original_permissions {
            fs::set_permissions(&temporary, permissions)
                .map_err(|error| file_operation_error("保留原文件权限", &error))?;
        }
        let latest_version = if path.exists() {
            Some(file_version(path)?)
        } else {
            None
        };
        if latest_version.as_ref() != expected_version {
            return Err(file_version_conflict_error(
                latest_version.as_ref(),
                expected_version,
            ));
        }
        fs::rename(&temporary, path).map_err(|error| file_operation_error("替换原文件", &error))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

#[tauri::command]
fn write_markdown(
    path: String,
    content: String,
    encoding: String,
    expected_version: Option<FileVersion>,
) -> Result<MarkdownWriteResult, String> {
    let requested_path = Path::new(&path);
    let parent = requested_path
        .parent()
        .ok_or_else(|| "无法定位文件夹".to_owned())?
        .canonicalize()
        .map_err(|error| file_operation_error("访问目标文件夹", &error))?;
    let file_name = requested_path
        .file_name()
        .ok_or_else(|| "文件名无效".to_owned())?;
    let requested_target = parent.join(file_name);
    validate_markdown_path(&requested_target)?;
    let target = requested_target.canonicalize().unwrap_or(requested_target);

    let current_version = if target.exists() {
        Some(file_version(&target)?)
    } else {
        None
    };
    if current_version != expected_version {
        return Err(file_version_conflict_error(
            current_version.as_ref(),
            expected_version.as_ref(),
        ));
    }

    let bytes = encode_markdown(&content, &encoding)?;
    write_markdown_atomically(&target, &bytes, expected_version.as_ref())?;
    let canonical_path = target
        .canonicalize()
        .map_err(|error| file_operation_error("定位已保存文件", &error))?;
    let name = canonical_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Markdown 文档")
        .to_owned();

    Ok(MarkdownWriteResult {
        path: canonical_path.to_string_lossy().into_owned(),
        name,
        encoding,
        version: file_version(&canonical_path)?,
    })
}

#[tauri::command]
fn write_export_html(path: String, content: String) -> Result<(), String> {
    let requested_path = Path::new(&path);
    let parent = requested_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|error| file_operation_error("访问导出文件夹", &error))?;
    let file_name = requested_path
        .file_name()
        .ok_or_else(|| "导出文件名无效".to_owned())?;
    let target = parent.join(file_name);
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "html" && extension != "htm" {
        return Err("导出文件必须使用 .html 或 .htm 扩展名".into());
    }

    let expected_version = match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("不能将 HTML 导出到符号链接".into());
        }
        Ok(metadata) if !metadata.is_file() => return Err("导出目标不是文件".into()),
        Ok(_) => Some(file_version(&target)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(file_operation_error("检查导出目标", &error)),
    };

    write_markdown_atomically(&target, content.as_bytes(), expected_version.as_ref())
}

#[tauri::command]
fn resolve_markdown_link(document_path: String, source: String) -> Result<String, String> {
    let link = Path::new(&source);
    if source.contains("://")
        || matches!(
            link.components().next(),
            Some(Component::Prefix(_)) | Some(Component::RootDir)
        )
    {
        return Err("仅支持相对 Markdown 文档链接".into());
    }

    let document = Path::new(&document_path)
        .canonicalize()
        .map_err(|error| file_operation_error("定位当前 Markdown 文档", &error))?;
    let parent = document
        .parent()
        .ok_or_else(|| "无法定位文档目录".to_owned())?;
    let target = parent
        .join(source.replace('/', std::path::MAIN_SEPARATOR_STR))
        .canonicalize()
        .map_err(|error| file_operation_error("打开链接文档", &error))?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "md" && extension != "markdown" {
        return Err("链接目标不是 Markdown 文档".into());
    }

    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
fn read_local_image(document_path: String, source: String) -> Result<String, String> {
    if source.contains("://") || Path::new(&source).is_absolute() {
        return Err("仅支持本地相对路径图片".into());
    }

    let document = Path::new(&document_path)
        .canonicalize()
        .map_err(|error| file_operation_error("定位 Markdown 文档", &error))?;
    let parent = document
        .parent()
        .ok_or_else(|| "无法定位文档目录".to_owned())?;
    let image = parent
        .join(source.replace('/', std::path::MAIN_SEPARATOR_STR))
        .canonicalize()
        .map_err(|error| file_operation_error("打开图片", &error))?;
    if !image.starts_with(parent) {
        return Err("图片路径不能超出 Markdown 所在文件夹".into());
    }
    let extension = image
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return Err("不支持的图片格式".into()),
    };

    const MAX_IMAGE_BYTES: u64 = 12 * 1024 * 1024;
    let file = fs::File::open(&image).map_err(|error| file_operation_error("读取图片", &error))?;
    let metadata = file
        .metadata()
        .map_err(|error| file_operation_error("读取图片状态", &error))?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err("图片超过 12 MB，已跳过加载".into());
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let mut limited_file = file.take(MAX_IMAGE_BYTES + 1);
    limited_file
        .read_to_end(&mut bytes)
        .map_err(|error| file_operation_error("读取图片", &error))?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("图片超过 12 MB，已跳过加载".into());
    }
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_markdown,
            get_markdown_version,
            write_markdown,
            write_export_html,
            resolve_markdown_link,
            read_local_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        decode_markdown, encode_markdown, read_local_image, read_markdown, resolve_markdown_link,
        write_markdown,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "markdown-reader-test-{}-{timestamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn decodes_utf8_with_bom() {
        assert_eq!(decode_markdown(b"\xEF\xBB\xBFhello").unwrap(), "hello");
    }

    #[test]
    fn decodes_utf8_without_bom() {
        assert_eq!(decode_markdown("你好".as_bytes()).unwrap(), "你好");
    }

    #[test]
    fn decodes_utf16_with_bom() {
        assert_eq!(decode_markdown(b"\xFF\xFE\x2D\x4E").unwrap(), "中");
        assert_eq!(decode_markdown(b"\xFE\xFF\x4E\x2D").unwrap(), "中");
    }

    #[test]
    fn falls_back_to_gbk_without_bom() {
        assert_eq!(decode_markdown(b"\xD6\xD0").unwrap(), "中");
    }

    #[test]
    fn rejects_malformed_encoding_instead_of_returning_replacement_text() {
        assert!(decode_markdown(b"\xEF\xBB\xBF\xFF").is_err());
        assert!(decode_markdown(b"\xFF\xFE\x2D").is_err());
        assert!(decode_markdown(b"\x81").is_err());
    }

    #[test]
    fn reads_markdown_file_and_returns_canonical_path_and_name() {
        let temporary = TestDirectory::new();
        let documents = temporary.path().join("documents");
        fs::create_dir_all(&documents).unwrap();
        let markdown = documents.join("Notes.MARKDOWN");
        fs::write(&markdown, b"\xEF\xBB\xBF# Hello").unwrap();

        let document = read_markdown(markdown.to_string_lossy().into_owned()).unwrap();

        assert_eq!(
            document.path,
            markdown.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(document.name, "Notes.MARKDOWN");
        assert_eq!(document.content, "# Hello");
    }

    #[test]
    fn saves_markdown_and_returns_the_saved_file_status() {
        let temporary = TestDirectory::new();
        let markdown = temporary.path().join("note.md");
        fs::write(&markdown, "# Before").unwrap();
        let document = read_markdown(markdown.to_string_lossy().into_owned()).unwrap();

        let saved = write_markdown(
            document.path.clone(),
            "# After".into(),
            document.encoding,
            Some(document.version),
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&markdown).unwrap(), "# After");
        assert_eq!(saved.name, "note.md");
        assert_eq!(saved.version.size, "# After".len() as u64);
    }

    #[test]
    fn preserves_markdown_encoding_when_saving() {
        let temporary = TestDirectory::new();
        let content = "# Before";
        let updated_content = "# After 中文";
        let cases = [
            ("utf8-bom.md", "utf-8-bom"),
            ("utf16le.md", "utf-16le-bom"),
            ("utf16be.md", "utf-16be-bom"),
            ("gbk.md", "gbk"),
        ];

        for (name, encoding) in cases {
            let path = temporary.path().join(name);
            let initial_content = if encoding == "gbk" {
                "# 原文"
            } else {
                content
            };
            fs::write(&path, encode_markdown(initial_content, encoding).unwrap()).unwrap();
            let document = read_markdown(path.to_string_lossy().into_owned()).unwrap();
            assert_eq!(document.encoding, encoding);

            let saved = write_markdown(
                document.path,
                updated_content.into(),
                document.encoding,
                Some(document.version),
            )
            .unwrap();

            let bytes = fs::read(path).unwrap();
            assert_eq!(saved.encoding, encoding);
            assert_eq!(decode_markdown(&bytes).unwrap(), updated_content);
            assert_eq!(bytes, encode_markdown(updated_content, encoding).unwrap());
        }
    }

    #[test]
    fn rejects_unrepresentable_gbk_content_without_changing_the_file() {
        let temporary = TestDirectory::new();
        let path = temporary.path().join("gbk.md");
        let original = encode_markdown("# 原文", "gbk").unwrap();
        fs::write(&path, &original).unwrap();
        let document = read_markdown(path.to_string_lossy().into_owned()).unwrap();

        let error = write_markdown(
            document.path,
            "# New 😀".into(),
            document.encoding,
            Some(document.version),
        )
        .err()
        .unwrap();

        assert_eq!(error, "文件编码无法表示新增字符，请改用 UTF-8 保存");
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn does_not_overwrite_a_markdown_file_changed_after_opening() {
        let temporary = TestDirectory::new();
        let markdown = temporary.path().join("note.md");
        fs::write(&markdown, "# Before").unwrap();
        let document = read_markdown(markdown.to_string_lossy().into_owned()).unwrap();
        fs::write(&markdown, "# Changed outside LeanReader").unwrap();

        let result = write_markdown(
            document.path,
            "# Editor content".into(),
            document.encoding,
            Some(document.version),
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&markdown).unwrap(),
            "# Changed outside LeanReader"
        );
    }

    #[test]
    fn detects_same_size_external_changes_before_saving() {
        let temporary = TestDirectory::new();
        let markdown = temporary.path().join("note.md");
        fs::write(&markdown, "# Before").unwrap();
        let document = read_markdown(markdown.to_string_lossy().into_owned()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        fs::write(&markdown, "# After!").unwrap();

        let result = write_markdown(
            document.path,
            "# Editor content".into(),
            document.encoding,
            Some(document.version),
        );

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(markdown).unwrap(), "# After!");
    }

    #[test]
    fn rejects_non_markdown_files() {
        let temporary = TestDirectory::new();
        let text_file = temporary.path().join("notes.txt");
        fs::write(&text_file, "not Markdown").unwrap();

        let error = read_markdown(text_file.to_string_lossy().into_owned())
            .err()
            .unwrap();

        assert_eq!(error, "请选择 .md 或 .markdown 文件");
    }

    #[test]
    fn reports_missing_markdown_files() {
        let temporary = TestDirectory::new();
        let missing = temporary.path().join("missing.md");

        let error = read_markdown(missing.to_string_lossy().into_owned())
            .err()
            .unwrap();

        assert_eq!(error, "打开文件失败：文件不存在或已被移动");
    }

    #[test]
    fn reads_only_images_inside_the_markdown_folder() {
        let temporary = TestDirectory::new();
        let documents = temporary.path().join("documents");
        fs::create_dir_all(&documents).unwrap();
        let markdown = documents.join("note.md");
        fs::write(&markdown, "# Note").unwrap();
        fs::write(documents.join("image.png"), [1, 2, 3]).unwrap();
        fs::write(temporary.path().join("private.png"), [4, 5, 6]).unwrap();

        let markdown_path = markdown.to_string_lossy().into_owned();
        let image = read_local_image(markdown_path.clone(), "image.png".into()).unwrap();
        assert!(image.starts_with("data:image/png;base64,"));
        assert!(read_local_image(markdown_path, "../private.png".into()).is_err());
    }

    #[test]
    fn rejects_absolute_unsupported_and_oversized_images() {
        let temporary = TestDirectory::new();
        let documents = temporary.path().join("documents");
        fs::create_dir_all(&documents).unwrap();
        let markdown = documents.join("note.md");
        let absolute_image = documents.join("image.png");
        let unsupported_image = documents.join("image.txt");
        let oversized_image = documents.join("large.png");
        fs::write(&markdown, "# Note").unwrap();
        fs::write(&absolute_image, [1, 2, 3]).unwrap();
        fs::write(&unsupported_image, [1, 2, 3]).unwrap();
        fs::File::create(&oversized_image)
            .unwrap()
            .set_len(12 * 1024 * 1024 + 1)
            .unwrap();

        let markdown_path = markdown.to_string_lossy().into_owned();
        let absolute_error = read_local_image(
            markdown_path.clone(),
            absolute_image.to_string_lossy().into_owned(),
        )
        .err()
        .unwrap();
        let unsupported_error = read_local_image(markdown_path.clone(), "image.txt".into())
            .err()
            .unwrap();
        let oversized_error = read_local_image(markdown_path, "large.png".into())
            .err()
            .unwrap();

        assert_eq!(absolute_error, "仅支持本地相对路径图片");
        assert_eq!(unsupported_error, "不支持的图片格式");
        assert_eq!(oversized_error, "图片超过 12 MB，已跳过加载");
    }

    #[test]
    fn resolves_relative_markdown_links_and_rejects_absolute_targets() {
        let temporary = TestDirectory::new();
        let current = temporary.path().join("current.md");
        let target = temporary.path().join("target.markdown");
        let unsupported = temporary.path().join("notes.txt");
        fs::write(&current, "# Current").unwrap();
        fs::write(&target, "# Target").unwrap();
        fs::write(&unsupported, "not Markdown").unwrap();

        let current_path = current.to_string_lossy().into_owned();
        let resolved =
            resolve_markdown_link(current_path.clone(), "target.markdown".into()).unwrap();
        assert_eq!(Path::new(&resolved), target.canonicalize().unwrap());
        assert!(resolve_markdown_link(current_path.clone(), resolved).is_err());
        assert!(resolve_markdown_link(current_path, "notes.txt".into()).is_err());
    }
}

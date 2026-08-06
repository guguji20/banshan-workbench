use crate::protocol::HostError;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const SERVICE_SETTLEMENT_TEMPLATE_SHA256: &str =
    "022D1399FD4CA8A2A04C006191C9865B79372B8B721B9776C38D0EFAF502FA56";

const DOCUMENT_PATH: &str = "word/document.xml";
const CORE_PROPERTIES_PATH: &str = "docProps/core.xml";
const SETTINGS_PATH: &str = "word/settings.xml";
const CONTENT_TYPES_PATH: &str = "[Content_Types].xml";
const ROOT_RELATIONSHIPS_PATH: &str = "_rels/.rels";
const MAX_PACKAGE_ENTRIES: usize = 2_048;
const MAX_DOCX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ITEMS: usize = 3;
const MAX_FIELD_BYTES: usize = 8 * 1024;

const HEADER_CELLS: [&str; 7] = [
    "序号",
    "视频名称",
    "使用时间",
    "服务说明",
    "服务是否按要求提供",
    "证明材料",
    "备注",
];

const SIGNOFF_LABELS: [&str; 3] = [
    "乙方验收人签字：",
    "甲方验收人签字：",
    "甲方验收监管人签字：",
];

const RESPONSIBILITY_TEXTS: [&str; 3] = [
    "乙方验收人：需根据实际情况提供相关执行完毕证明文件并签字盖章确认；",
    "甲方验收人：针对乙方提供的材料真实性进行复核并签字确认；",
    "甲方验收监管人：验收监管人需对验收流程及相关材料真实性、完整性、准确性进行监管。验收监管人不得与乙方人员，或与甲方验收人为同一人。",
];

static STAGED_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSettlementItem {
    pub service_name: String,
    pub period: String,
    pub description: String,
    pub provided_as_required: Option<bool>,
    pub evidence_label: String,
    pub remarks: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSettlementTemplateData {
    pub items: Vec<ServiceSettlementItem>,
}

#[derive(Debug)]
struct PackageEntry {
    name: String,
    options: SimpleFileOptions,
    is_dir: bool,
    contents: Vec<u8>,
}

#[derive(Debug)]
struct Package {
    entries: Vec<PackageEntry>,
    index_by_name: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct CellRange {
    events: Range<usize>,
    logical_columns: usize,
}

#[derive(Debug, Clone)]
struct RowRange {
    events: Range<usize>,
    cells: Vec<CellRange>,
}

#[derive(Debug, Clone)]
struct TableRange {
    rows: Vec<RowRange>,
    grid_columns: usize,
}

struct StagedOutput {
    path: Option<PathBuf>,
}

impl StagedOutput {
    fn create(destination: &Path) -> Result<(Self, File), HostError> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file_name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| HostError::validation("服务结算清单输出文件名无效"))?;
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| HostError::internal(format!("读取系统时间失败: {error}")))?
            .as_nanos();

        for _ in 0..32 {
            let counter = STAGED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(
                ".{file_name}.{}.{}.{}.tmp",
                std::process::id(),
                epoch_nanos,
                counter
            ));
            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => {
                    return Ok((
                        Self {
                            path: Some(candidate),
                        },
                        file,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(HostError::internal(format!(
                        "创建服务结算清单临时文件失败: {error}"
                    )));
                }
            }
        }

        Err(HostError::internal("无法分配服务结算清单临时文件"))
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("staged output path must exist before publish")
    }

    fn publish(self, destination: &Path) -> Result<(), HostError> {
        self.publish_with(destination, |source, destination| {
            fs::hard_link(source, destination)
        })
    }

    fn publish_with<F>(mut self, destination: &Path, hard_link: F) -> Result<(), HostError>
    where
        F: FnOnce(&Path, &Path) -> std::io::Result<()>,
    {
        match hard_link(self.path(), destination) {
            Ok(()) => {
                if let Some(path) = self.path.take() {
                    fs::remove_file(path).map_err(|error| {
                        HostError::internal(format!("清理服务结算清单临时文件失败: {error}"))
                    })?;
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(HostError::validation("服务结算清单输出文件已存在"))
            }
            Err(_) => {
                atomic_rename_no_replace(self.path(), destination).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        HostError::validation("服务结算清单输出文件已存在")
                    } else {
                        HostError::internal(format!(
                            "原子发布服务结算清单失败（硬链接与无覆盖移动均不可用）: {error}"
                        ))
                    }
                })?;
                self.path.take();
                Ok(())
            }
        }
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(windows)]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renameat2(
            olddirfd: std::ffi::c_int,
            oldpath: *const std::ffi::c_char,
            newdirfd: std::ffi::c_int,
            newpath: *const std::ffi::c_char,
            flags: std::ffi::c_uint,
        ) -> std::ffi::c_int;
    }

    const AT_FDCWD: std::ffi::c_int = -100;
    const RENAME_NOREPLACE: std::ffi::c_uint = 1;
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "source path contains a NUL byte",
        )
    })?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination path contains a NUL byte",
        )
    })?;
    let moved = unsafe {
        renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if moved == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: std::ffi::c_uint,
        ) -> std::ffi::c_int;
    }

    const RENAME_EXCL: std::ffi::c_uint = 0x0000_0004;
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "source path contains a NUL byte",
        )
    })?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination path contains a NUL byte",
        )
    })?;
    let moved = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) };
    if moved == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn atomic_rename_no_replace(_source: &Path, _destination: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace rename is unavailable on this platform",
    ))
}

pub(crate) fn clone_service_settlement_template(
    source: &Path,
    expected_sha256: &str,
    destination: &Path,
    data: &ServiceSettlementTemplateData,
) -> Result<(), HostError> {
    validate_paths(source, destination)?;
    let source_bytes = read_file_limited(source, MAX_DOCX_SOURCE_BYTES)?;
    clone_service_settlement_template_from_bytes(&source_bytes, expected_sha256, destination, data)
}

pub(crate) fn clone_service_settlement_template_from_bytes(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &ServiceSettlementTemplateData,
) -> Result<(), HostError> {
    if !expected_sha256.eq_ignore_ascii_case(SERVICE_SETTLEMENT_TEMPLATE_SHA256) {
        return Err(HostError::validation(format!(
            "白鹅潭服务结算清单必须绑定登记模板 SHA-256: {SERVICE_SETTLEMENT_TEMPLATE_SHA256}"
        )));
    }
    clone_service_settlement_template_from_bytes_impl(source, expected_sha256, destination, data)
}

fn clone_service_settlement_template_from_bytes_impl(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &ServiceSettlementTemplateData,
) -> Result<(), HostError> {
    if source.len() as u64 > MAX_DOCX_SOURCE_BYTES {
        return Err(HostError::validation(format!(
            "服务结算清单模板不能超过 {MAX_DOCX_SOURCE_BYTES} 字节"
        )));
    }
    validate_template_data(data)?;
    validate_destination(destination)?;
    validate_expected_hash(expected_sha256)?;

    let actual_sha256 = sha256_bytes(source);
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(HostError::validation(format!(
            "服务结算清单模板 SHA-256 不匹配，期望 {expected_sha256}，实际 {actual_sha256}"
        )));
    }

    let mut package = load_package_from_bytes(source)?;
    validate_standard_docx(&package)?;

    let document = package_entry(&package, DOCUMENT_PATH)?;
    let transformed_document = transform_document(document, data)?;
    replace_package_entry(&mut package, DOCUMENT_PATH, transformed_document)?;

    if package.index_by_name.contains_key(CORE_PROPERTIES_PATH) {
        let core = package_entry(&package, CORE_PROPERTIES_PATH)?;
        let sanitized = remove_named_elements(core, &[b"creator", b"lastModifiedBy"])?;
        replace_package_entry(&mut package, CORE_PROPERTIES_PATH, sanitized)?;
    }

    if package.index_by_name.contains_key(SETTINGS_PATH) {
        let settings = package_entry(&package, SETTINGS_PATH)?;
        let sanitized = remove_named_elements(settings, &[b"attachedTemplate"])?;
        replace_package_entry(&mut package, SETTINGS_PATH, sanitized)?;
    }

    let relationship_paths = package
        .entries
        .iter()
        .filter(|entry| entry.name.ends_with(".rels"))
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    for path in relationship_paths {
        let relationships = package_entry(&package, &path)?;
        let sanitized = sanitize_relationships(relationships)?;
        replace_package_entry(&mut package, &path, sanitized)?;
    }

    let (staged, staged_file) = StagedOutput::create(destination)?;
    write_package(staged_file, &package)?;
    verify_output(staged.path(), data)?;
    staged.publish(destination)
}

fn validate_template_data(data: &ServiceSettlementTemplateData) -> Result<(), HostError> {
    if data.items.is_empty() {
        return Err(HostError::validation("服务结算清单至少需要一条服务明细"));
    }
    if data.items.len() > MAX_ITEMS {
        return Err(HostError::validation(format!(
            "服务结算清单真实分页尚未完成，当前最多可生成 {MAX_ITEMS} 条服务明细"
        )));
    }

    for (index, item) in data.items.iter().enumerate() {
        let row = index + 1;
        if item.provided_as_required.is_none() {
            return Err(HostError::validation(format!(
                "服务结算清单第 {row} 行尚未确认是否按要求提供"
            )));
        }
        validate_field(row, "视频名称", &item.service_name)?;
        validate_field(row, "使用时间", &item.period)?;
        validate_field(row, "服务说明", &item.description)?;
        validate_field(row, "证明材料", &item.evidence_label)?;
        validate_field(row, "备注", &item.remarks)?;
    }
    Ok(())
}

fn validate_field(row: usize, label: &str, value: &str) -> Result<(), HostError> {
    if value.len() > MAX_FIELD_BYTES {
        return Err(HostError::validation(format!(
            "服务结算清单第 {row} 行{label}超过 {MAX_FIELD_BYTES} 字节"
        )));
    }
    if value
        .chars()
        .any(|character| !is_valid_xml_character(character))
    {
        return Err(HostError::validation(format!(
            "服务结算清单第 {row} 行{label}包含非法 XML 字符"
        )));
    }
    Ok(())
}

fn is_valid_xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || matches!(character as u32, 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

fn validate_paths(source: &Path, destination: &Path) -> Result<(), HostError> {
    let source = fs::canonicalize(source)
        .map_err(|error| HostError::internal(format!("读取服务结算清单模板失败: {error}")))?;
    if !source.is_file() {
        return Err(HostError::validation("服务结算清单模板不是文件"));
    }

    let destination_parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = fs::canonicalize(destination_parent)
        .map_err(|error| HostError::internal(format!("读取服务结算清单输出目录失败: {error}")))?;
    let destination_name = destination
        .file_name()
        .ok_or_else(|| HostError::validation("服务结算清单输出路径无效"))?;
    let destination_absolute = destination_parent.join(destination_name);

    if paths_equal(&source, &destination_absolute) {
        return Err(HostError::validation("服务结算清单模板和输出路径不能相同"));
    }
    validate_destination(&destination_absolute)
}

fn validate_destination(destination: &Path) -> Result<(), HostError> {
    let destination_parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = fs::canonicalize(destination_parent)
        .map_err(|error| HostError::internal(format!("读取服务结算清单输出目录失败: {error}")))?;
    let destination_name = destination
        .file_name()
        .ok_or_else(|| HostError::validation("服务结算清单输出路径无效"))?;
    let destination_absolute = destination_parent.join(destination_name);

    if destination_absolute.exists() {
        return Err(HostError::validation("服务结算清单输出文件已存在"));
    }
    if destination_absolute
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("docx"))
    {
        return Err(HostError::validation(
            "服务结算清单输出文件必须使用 .docx 扩展名",
        ));
    }
    Ok(())
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn validate_expected_hash(expected_sha256: &str) -> Result<(), HostError> {
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(HostError::validation(
            "服务结算清单模板 SHA-256 必须是 64 位十六进制字符串",
        ));
    }
    Ok(())
}

fn read_file_limited(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, HostError> {
    let file = File::open(path)
        .map_err(|error| HostError::internal(format!("打开服务结算清单模板失败: {error}")))?;
    let mut bytes = Vec::new();
    file.take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| HostError::internal(format!("读取服务结算清单模板失败: {error}")))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(HostError::validation(format!(
            "服务结算清单模板不能超过 {maximum_bytes} 字节"
        )));
    }
    Ok(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String, HostError> {
    Ok(sha256_bytes(&read_file_limited(
        path,
        MAX_DOCX_SOURCE_BYTES,
    )?))
}

fn load_package_from_bytes(source: &[u8]) -> Result<Package, HostError> {
    let mut archive = ZipArchive::new(Cursor::new(source))
        .map_err(|error| HostError::validation(format!("DOCX 不是有效 ZIP 包: {error}")))?;
    if archive.is_empty() || archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(HostError::validation(format!(
            "DOCX 包条目数必须在 1 到 {MAX_PACKAGE_ENTRIES} 之间"
        )));
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut index_by_name = HashMap::with_capacity(archive.len());
    let mut case_folded_names = HashSet::with_capacity(archive.len());
    let mut total_size = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| HostError::validation(format!("读取 DOCX 条目失败: {error}")))?;
        let name = entry.name().to_owned();
        validate_zip_entry_name(&name)?;
        if entry.encrypted() {
            return Err(HostError::validation(format!("DOCX 条目不能加密: {name}")));
        }
        if entry.is_symlink() {
            return Err(HostError::validation(format!(
                "DOCX 条目不能是符号链接: {name}"
            )));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(HostError::validation(format!(
                "DOCX 条目使用了不支持的压缩算法: {name}"
            )));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(HostError::validation(format!("DOCX 条目过大: {name}")));
        }
        total_size = total_size
            .checked_add(entry.size())
            .ok_or_else(|| HostError::validation("DOCX 包解压大小溢出"))?;
        if total_size > MAX_PACKAGE_BYTES {
            return Err(HostError::validation("DOCX 包解压后过大"));
        }

        let folded = name.to_ascii_lowercase();
        if index_by_name.contains_key(&name) || !case_folded_names.insert(folded) {
            return Err(HostError::validation(format!(
                "DOCX 包包含重复条目: {name}"
            )));
        }

        let options = entry.options();
        let is_dir = entry.is_dir();
        let mut contents = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut contents).map_err(|error| {
            HostError::validation(format!("解压 DOCX 条目失败 {name}: {error}"))
        })?;
        index_by_name.insert(name.clone(), entries.len());
        entries.push(PackageEntry {
            name,
            options,
            is_dir,
            contents,
        });
    }

    Ok(Package {
        entries,
        index_by_name,
    })
}

fn validate_zip_entry_name(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.starts_with('/')
        || name.starts_with("//")
        || has_windows_drive_prefix(name)
    {
        return Err(HostError::validation(format!(
            "DOCX 包含不安全条目路径: {name}"
        )));
    }

    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty()
        || trimmed
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(HostError::validation(format!(
            "DOCX 包含路径逃逸条目: {name}"
        )));
    }
    Ok(())
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn validate_standard_docx(package: &Package) -> Result<(), HostError> {
    for required in [CONTENT_TYPES_PATH, ROOT_RELATIONSHIPS_PATH, DOCUMENT_PATH] {
        if !package.index_by_name.contains_key(required) {
            return Err(HostError::validation(format!(
                "DOCX 缺少标准条目: {required}"
            )));
        }
    }

    for entry in &package.entries {
        let lower_name = entry.name.to_ascii_lowercase();
        if lower_name.ends_with("vbaproject.bin")
            || lower_name.contains("/macros/")
            || lower_name.contains("/activex/")
            || lower_name.contains("/embeddings/")
        {
            return Err(HostError::validation(format!(
                "DOCX 包含宏或活动内容: {}",
                entry.name
            )));
        }
    }

    validate_content_types(package_entry(package, CONTENT_TYPES_PATH)?)?;
    for entry in package
        .entries
        .iter()
        .filter(|entry| entry.name.ends_with(".rels"))
    {
        validate_relationships(&entry.contents)?;
    }
    for entry in package
        .entries
        .iter()
        .filter(|entry| entry.name.starts_with("word/") && entry.name.ends_with(".xml"))
    {
        validate_no_active_word_content(&entry.contents, &entry.name)?;
    }
    Ok(())
}

fn validate_content_types(xml: &[u8]) -> Result<(), HostError> {
    let events = parse_xml_events(xml, CONTENT_TYPES_PATH)?;
    let mut has_document_type = false;
    for event in &events {
        let start = match event {
            Event::Start(start) | Event::Empty(start) => start,
            _ => continue,
        };
        if !matches!(local_name(start), b"Default" | b"Override") {
            continue;
        }
        let content_type = attribute_value(start, b"ContentType")?.unwrap_or_default();
        let lower = content_type.to_ascii_lowercase();
        if lower.contains("macroenabled")
            || lower.contains("vbaproject")
            || lower.contains("activex")
            || lower.contains("oleobject")
        {
            return Err(HostError::validation("DOCX 内容类型包含宏或活动内容"));
        }
        if local_name(start) == b"Override"
            && attribute_value(start, b"PartName")?.as_deref() == Some("/word/document.xml")
        {
            if content_type
                != "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
            {
                return Err(HostError::validation(
                    "DOCX 主文档不是标准无宏 Word 文档类型",
                ));
            }
            has_document_type = true;
        }
    }
    if !has_document_type {
        return Err(HostError::validation(
            "DOCX 内容类型未声明标准 word/document.xml",
        ));
    }
    Ok(())
}

fn validate_relationships(xml: &[u8]) -> Result<(), HostError> {
    let events = parse_xml_events(xml, "DOCX relationships")?;
    for event in &events {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"Relationship" => {
                start
            }
            _ => continue,
        };
        validate_relationship(start)?;
    }
    Ok(())
}

fn validate_relationship(start: &BytesStart<'_>) -> Result<(), HostError> {
    let relationship_type = attribute_value(start, b"Type")?.unwrap_or_default();
    let target = attribute_value(start, b"Target")?.unwrap_or_default();
    let target_mode = attribute_value(start, b"TargetMode")?.unwrap_or_default();
    let lower_type = relationship_type.to_ascii_lowercase();

    if target_mode.eq_ignore_ascii_case("external") {
        return Err(HostError::validation("DOCX 包含外部关系"));
    }
    if lower_type.contains("vbaproject")
        || lower_type.contains("macro")
        || lower_type.ends_with("/oleobject")
        || lower_type.ends_with("/package")
    {
        return Err(HostError::validation("DOCX 关系包含宏、OLE 或嵌入包"));
    }
    if relationship_target_is_absolute(&target) {
        return Err(HostError::validation(format!(
            "DOCX 关系包含绝对路径或外部目标: {target}"
        )));
    }
    Ok(())
}

fn validate_no_active_word_content(xml: &[u8], label: &str) -> Result<(), HostError> {
    let events = parse_xml_events(xml, label)?;
    let mut instruction_depth = 0_usize;
    let mut instruction_text = String::new();
    for event in &events {
        match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"object" => {
                return Err(HostError::validation(format!(
                    "DOCX 包含 w:object 活动内容: {label}"
                )));
            }
            Event::Start(start) if local_name(start) == b"instrText" => {
                instruction_depth += 1;
            }
            Event::End(end) if end.local_name().as_ref() == b"instrText" => {
                instruction_depth = instruction_depth.saturating_sub(1);
                instruction_text.push(' ');
            }
            Event::Text(text) if instruction_depth > 0 => {
                instruction_text.push_str(&decode_text(text)?);
            }
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"fldSimple" => {
                if attribute_value(start, b"instr")?
                    .as_deref()
                    .is_some_and(contains_dde_instruction)
                {
                    return Err(HostError::validation(format!(
                        "DOCX 包含 DDE 字段代码: {label}"
                    )));
                }
            }
            _ => {}
        }
    }
    if contains_dde_instruction(&instruction_text) {
        return Err(HostError::validation(format!(
            "DOCX 包含 DDE 字段代码: {label}"
        )));
    }
    Ok(())
}

fn contains_dde_instruction(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| token.eq_ignore_ascii_case("dde") || token.eq_ignore_ascii_case("ddeauto"))
}

fn relationship_target_is_absolute(target: &str) -> bool {
    let trimmed = target.trim();
    if trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.starts_with("//")
        || has_windows_drive_prefix(trimmed)
    {
        return true;
    }
    let bytes = trimmed.as_bytes();
    if let Some(colon) = bytes.iter().position(|byte| *byte == b':') {
        return colon > 0
            && bytes[..colon]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'));
    }
    false
}

fn package_entry<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], HostError> {
    let index = package
        .index_by_name
        .get(name)
        .copied()
        .ok_or_else(|| HostError::validation(format!("DOCX 缺少条目: {name}")))?;
    Ok(&package.entries[index].contents)
}

fn replace_package_entry(
    package: &mut Package,
    name: &str,
    contents: Vec<u8>,
) -> Result<(), HostError> {
    let index = package
        .index_by_name
        .get(name)
        .copied()
        .ok_or_else(|| HostError::validation(format!("DOCX 缺少条目: {name}")))?;
    package.entries[index].contents = contents;
    Ok(())
}

fn transform_document(
    xml: &[u8],
    data: &ServiceSettlementTemplateData,
) -> Result<Vec<u8>, HostError> {
    let events = parse_xml_events(xml, DOCUMENT_PATH)?;
    assert_responsibility_texts(&events)?;
    let table = locate_service_table(&events, Some(1))?;
    let header = table.rows[0].clone();
    let prototype = table.rows[1].clone();
    let signoff = table.rows[2].clone();

    let mut output = Vec::with_capacity(events.len() + data.items.len() * 64);
    output.extend(events[..header.events.start].iter().cloned());
    output.extend(add_row_properties(
        &events,
        header.events.clone(),
        &["w:tblHeader", "w:cantSplit"],
    )?);
    output.extend(
        events[header.events.end..prototype.events.start]
            .iter()
            .cloned(),
    );
    for (index, item) in data.items.iter().enumerate() {
        let values = [
            (index + 1).to_string(),
            item.service_name.clone(),
            item.period.clone(),
            item.description.clone(),
            String::new(),
            item.evidence_label.clone(),
            item.remarks.clone(),
        ];
        output.extend(build_data_row(
            &events,
            &prototype,
            &values,
            item.provided_as_required == Some(true),
        )?);
    }
    output.extend(
        events[prototype.events.end..signoff.events.start]
            .iter()
            .cloned(),
    );
    output.extend(sanitize_signoff_row(
        &events[signoff.events.start..signoff.events.end],
    )?);
    output.extend(events[signoff.events.end..].iter().cloned());
    write_xml_events(output)
}

fn build_data_row(
    events: &[Event<'static>],
    prototype: &RowRange,
    values: &[String; 7],
    provided_as_required: bool,
) -> Result<Vec<Event<'static>>, HostError> {
    if prototype.cells.len() != 7 {
        return Err(HostError::validation("服务结算清单样式原型行不是 7 列"));
    }
    let mut output = Vec::new();
    output.extend(
        events[prototype.events.start..prototype.cells[0].events.start]
            .iter()
            .cloned(),
    );
    for (index, (cell, value)) in prototype.cells.iter().zip(values).enumerate() {
        if index == 4 {
            output.extend(toggle_status_cell(events, cell, provided_as_required)?);
        } else {
            output.extend(replace_cell_text(events, cell, value)?);
        }
    }
    output.extend(
        events[prototype.cells[6].events.end..prototype.events.end]
            .iter()
            .cloned(),
    );
    let output_len = output.len();
    let output = add_row_properties(&output, 0..output_len, &["w:cantSplit"])?;
    strip_cloned_paragraph_ids(output)
}

fn add_row_properties(
    events: &[Event<'static>],
    row: Range<usize>,
    required_properties: &[&str],
) -> Result<Vec<Event<'static>>, HostError> {
    if !matches!(events.get(row.start), Some(Event::Start(start)) if local_name(start) == b"tr")
        || !matches!(events.get(row.end.saturating_sub(1)), Some(Event::End(end)) if end.local_name().as_ref() == b"tr")
    {
        return Err(HostError::validation("服务结算清单表格行结构无效"));
    }

    let existing = find_first_element(events, row.clone(), b"trPr")?
        .filter(|properties| properties.start > row.start && properties.end < row.end);
    let has_property = |name: &str| {
        existing.as_ref().is_some_and(|properties| {
            events[properties.clone()].iter().any(|event| match event {
                Event::Start(start) | Event::Empty(start) => {
                    start.name().as_ref() == name.as_bytes()
                }
                _ => false,
            })
        })
    };
    let missing = required_properties
        .iter()
        .copied()
        .filter(|name| !has_property(name))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(events[row].to_vec());
    }

    let mut output = Vec::with_capacity(row.len() + missing.len() + 2);
    if let Some(properties) = existing {
        output.extend(events[row.start..properties.end - 1].iter().cloned());
        output.extend(
            missing
                .iter()
                .map(|name| Event::Empty(BytesStart::new((*name).to_owned()))),
        );
        output.extend(events[properties.end - 1..row.end].iter().cloned());
    } else {
        output.push(events[row.start].clone());
        output.push(Event::Start(BytesStart::new("w:trPr")));
        output.extend(
            missing
                .iter()
                .map(|name| Event::Empty(BytesStart::new((*name).to_owned()))),
        );
        output.push(Event::End(BytesEnd::new("w:trPr")));
        output.extend(events[row.start + 1..row.end].iter().cloned());
    }
    Ok(output)
}

fn toggle_status_cell(
    events: &[Event<'static>],
    cell: &CellRange,
    provided_as_required: bool,
) -> Result<Vec<Event<'static>>, HostError> {
    let paragraphs = collect_element_ranges(events, cell.events.clone(), b"p")?;
    if paragraphs.len() != 2 {
        return Err(HostError::validation(
            "服务结算清单状态列必须保留“是/否”两个复选框段落",
        ));
    }
    let paragraph_texts = paragraphs
        .iter()
        .map(|paragraph| text_in_range(events, paragraph.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    if !paragraph_texts[0].contains('是') || !paragraph_texts[1].contains('否') {
        return Err(HostError::validation(
            "服务结算清单状态列复选框段落顺序无效",
        ));
    }

    let mut checkbox_counts = [0_usize; 2];
    let mut output = Vec::with_capacity(cell.events.len());
    for (index, event) in events[cell.events.clone()].iter().enumerate() {
        let absolute_index = cell.events.start + index;
        let paragraph_index = paragraphs
            .iter()
            .position(|paragraph| paragraph.contains(&absolute_index));
        if let (Event::Text(text), Some(paragraph_index)) = (event, paragraph_index) {
            let selected = if paragraph_index == 0 {
                provided_as_required
            } else {
                !provided_as_required
            };
            let value = decode_text(text)?;
            let mut changed = false;
            let toggled = value
                .chars()
                .map(|character| {
                    if matches!(character, '☑' | '☒' | '☐' | '□') {
                        changed = true;
                        checkbox_counts[paragraph_index] += 1;
                        if selected {
                            '☑'
                        } else {
                            '□'
                        }
                    } else {
                        character
                    }
                })
                .collect::<String>();
            if changed {
                output.push(Event::Text(BytesText::new(&toggled).into_owned()));
                continue;
            }
        }
        output.push(event.clone());
    }
    if checkbox_counts != [1, 1] {
        return Err(HostError::validation(
            "服务结算清单状态列必须每个段落各包含一个复选框",
        ));
    }
    Ok(output)
}

fn collect_element_ranges(
    events: &[Event<'static>],
    range: Range<usize>,
    name: &[u8],
) -> Result<Vec<Range<usize>>, HostError> {
    let mut ranges = Vec::new();
    let mut cursor = range.start;
    while cursor < range.end {
        let Some(found) = find_first_element(events, cursor..range.end, name)? else {
            break;
        };
        cursor = found.end;
        ranges.push(found);
    }
    Ok(ranges)
}

fn strip_cloned_paragraph_ids(
    events: Vec<Event<'static>>,
) -> Result<Vec<Event<'static>>, HostError> {
    events
        .into_iter()
        .map(|event| match event {
            Event::Start(start) if local_name(&start) == b"p" => {
                Ok(Event::Start(strip_paragraph_ids(&start)?))
            }
            Event::Empty(start) if local_name(&start) == b"p" => {
                Ok(Event::Empty(strip_paragraph_ids(&start)?))
            }
            event => Ok(event),
        })
        .collect()
}

fn strip_paragraph_ids(start: &BytesStart<'_>) -> Result<BytesStart<'static>, HostError> {
    let mut cleaned = start.clone().into_owned();
    cleaned.clear_attributes();
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute
            .map_err(|error| HostError::validation(format!("DOCX XML 属性无效: {error}")))?;
        if !matches!(attribute.key.local_name().as_ref(), b"paraId" | b"textId") {
            cleaned.push_attribute(attribute);
        }
    }
    Ok(cleaned)
}

fn replace_cell_text(
    events: &[Event<'static>],
    cell: &CellRange,
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    let paragraph = find_first_element(events, cell.events.clone(), b"p")?
        .ok_or_else(|| HostError::validation("服务结算清单样式单元格缺少段落"))?;
    let paragraph_properties = find_first_element(events, paragraph.clone(), b"pPr")?
        .filter(|range| range.start > paragraph.start && range.end < paragraph.end);
    let insert_at = paragraph_properties
        .as_ref()
        .map_or(paragraph.start + 1, |range| range.end);
    let run_properties = find_first_element(events, paragraph.clone(), b"rPr")?;

    let mut output = Vec::new();
    output.extend(events[cell.events.start..insert_at].iter().cloned());
    output.push(Event::Start(BytesStart::new("w:r")));
    if let Some(properties) = run_properties {
        output.extend(events[properties].iter().cloned());
    }
    let mut text_start = BytesStart::new("w:t");
    if value.starts_with(char::is_whitespace) || value.ends_with(char::is_whitespace) {
        text_start.push_attribute(("xml:space", "preserve"));
    }
    output.push(Event::Start(text_start));
    output.push(Event::Text(BytesText::new(value).into_owned()));
    output.push(Event::End(BytesEnd::new("w:t")));
    output.push(Event::End(BytesEnd::new("w:r")));
    output.push(events[paragraph.end - 1].clone());
    output.push(events[cell.events.end - 1].clone());
    Ok(output)
}

fn sanitize_signoff_row(events: &[Event<'static>]) -> Result<Vec<Event<'static>>, HostError> {
    let mut output = Vec::with_capacity(events.len());
    let mut inside_word_text = false;
    for event in events {
        match event {
            Event::Start(start) if local_name(start) == b"t" => {
                inside_word_text = true;
                output.push(event.clone());
            }
            Event::End(end) if end.local_name().as_ref() == b"t" => {
                inside_word_text = false;
                output.push(event.clone());
            }
            Event::Text(text) if inside_word_text => {
                let value = decode_text(text)?;
                let sanitized = value
                    .chars()
                    .filter(|character| !character.is_ascii_digit())
                    .collect::<String>();
                output.push(Event::Text(BytesText::new(&sanitized).into_owned()));
            }
            _ => output.push(event.clone()),
        }
    }
    Ok(output)
}

fn locate_service_table(
    events: &[Event<'static>],
    expected_data_rows: Option<usize>,
) -> Result<TableRange, HostError> {
    let tables = collect_tables(events)?;
    let mut candidates = Vec::new();
    for table in tables {
        if table.grid_columns != 7 || table.rows.len() < 3 {
            continue;
        }
        if expected_data_rows.is_some_and(|count| table.rows.len() != count + 2) {
            continue;
        }
        let header = &table.rows[0];
        if header.cells.len() != 7
            || header
                .cells
                .iter()
                .map(|cell| normalized_cell_text(events, cell))
                .collect::<Result<Vec<_>, _>>()?
                != HEADER_CELLS
        {
            continue;
        }
        if table.rows[1..table.rows.len() - 1].iter().any(|row| {
            row.cells.len() != 7 || row.cells.iter().any(|cell| cell.logical_columns != 1)
        }) {
            continue;
        }
        let signoff = table.rows.last().expect("table has at least three rows");
        if signoff
            .cells
            .iter()
            .map(|cell| cell.logical_columns)
            .sum::<usize>()
            != 7
        {
            continue;
        }
        let signoff_text = row_text(events, signoff)?;
        if !SIGNOFF_LABELS
            .iter()
            .all(|label| signoff_text.contains(label))
        {
            continue;
        }
        candidates.push(table);
    }

    if candidates.len() != 1 {
        return Err(HostError::validation(format!(
            "服务结算清单必须包含唯一的语义主表，实际匹配 {} 个",
            candidates.len()
        )));
    }
    Ok(candidates.remove(0))
}

fn collect_tables(events: &[Event<'static>]) -> Result<Vec<TableRange>, HostError> {
    let mut tables = Vec::new();
    let mut depth = 0_usize;
    let mut table_start = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            Event::Start(start) if local_name(start) == b"tbl" => {
                if depth == 0 {
                    table_start = Some(index);
                }
                depth += 1;
            }
            Event::End(end) if end.local_name().as_ref() == b"tbl" => {
                if depth == 0 {
                    return Err(HostError::validation("DOCX 表格结束标签不平衡"));
                }
                depth -= 1;
                if depth == 0 {
                    let start = table_start
                        .take()
                        .ok_or_else(|| HostError::validation("DOCX 表格结构无效"))?;
                    tables.push(analyze_table(events, start..index + 1)?);
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(HostError::validation("DOCX 表格开始标签不平衡"));
    }
    Ok(tables)
}

fn analyze_table(events: &[Event<'static>], table: Range<usize>) -> Result<TableRange, HostError> {
    let mut table_depth = 0_usize;
    let mut row_start = None;
    let mut rows = Vec::new();
    let mut grid_columns = 0_usize;

    for index in table.clone() {
        match &events[index] {
            Event::Start(start) if local_name(start) == b"tbl" => table_depth += 1,
            Event::End(end) if end.local_name().as_ref() == b"tbl" => {
                table_depth = table_depth.saturating_sub(1)
            }
            Event::Start(start) if local_name(start) == b"tr" && table_depth == 1 => {
                row_start = Some(index)
            }
            Event::End(end) if end.local_name().as_ref() == b"tr" && table_depth == 1 => {
                let start = row_start
                    .take()
                    .ok_or_else(|| HostError::validation("DOCX 表格行结构无效"))?;
                rows.push(analyze_row(events, start..index + 1)?);
            }
            Event::Start(start) | Event::Empty(start)
                if local_name(start) == b"gridCol" && table_depth == 1 =>
            {
                grid_columns += 1;
            }
            _ => {}
        }
    }
    Ok(TableRange { rows, grid_columns })
}

fn analyze_row(events: &[Event<'static>], row: Range<usize>) -> Result<RowRange, HostError> {
    let mut nested_table_depth = 0_usize;
    let mut cell_start = None;
    let mut cells = Vec::new();
    for index in row.clone() {
        match &events[index] {
            Event::Start(start) if local_name(start) == b"tbl" => nested_table_depth += 1,
            Event::End(end) if end.local_name().as_ref() == b"tbl" => {
                nested_table_depth = nested_table_depth.saturating_sub(1)
            }
            Event::Start(start) if local_name(start) == b"tc" && nested_table_depth == 0 => {
                cell_start = Some(index)
            }
            Event::End(end) if end.local_name().as_ref() == b"tc" && nested_table_depth == 0 => {
                let start = cell_start
                    .take()
                    .ok_or_else(|| HostError::validation("DOCX 单元格结构无效"))?;
                let range = start..index + 1;
                cells.push(CellRange {
                    logical_columns: cell_logical_columns(events, range.clone())?,
                    events: range,
                });
            }
            _ => {}
        }
    }
    Ok(RowRange { events: row, cells })
}

fn cell_logical_columns(events: &[Event<'static>], cell: Range<usize>) -> Result<usize, HostError> {
    for event in &events[cell] {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"gridSpan" => start,
            _ => continue,
        };
        let value = attribute_value(start, b"val")?
            .ok_or_else(|| HostError::validation("DOCX gridSpan 缺少 val"))?;
        return value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| HostError::validation("DOCX gridSpan 无效"));
    }
    Ok(1)
}

fn normalized_cell_text(events: &[Event<'static>], cell: &CellRange) -> Result<String, HostError> {
    Ok(cell_text(events, cell)?
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect())
}

fn cell_text(events: &[Event<'static>], cell: &CellRange) -> Result<String, HostError> {
    text_in_range(events, cell.events.clone())
}

fn row_text(events: &[Event<'static>], row: &RowRange) -> Result<String, HostError> {
    text_in_range(events, row.events.clone())
}

fn text_in_range(events: &[Event<'static>], range: Range<usize>) -> Result<String, HostError> {
    let mut output = String::new();
    let mut inside_word_text = false;
    for event in &events[range] {
        match event {
            Event::Start(start) if local_name(start) == b"t" => inside_word_text = true,
            Event::End(end) if end.local_name().as_ref() == b"t" => inside_word_text = false,
            Event::Text(text) if inside_word_text => output.push_str(&decode_text(text)?),
            Event::GeneralRef(reference) if inside_word_text => {
                let name = reference.decode().map_err(|error| {
                    HostError::validation(format!("DOCX 实体解码失败: {error}"))
                })?;
                output.push_str(decode_predefined_entity(name.as_ref())?);
            }
            Event::CData(text) if inside_word_text => {
                output.push_str(&text.decode().map_err(|error| {
                    HostError::validation(format!("DOCX 文本解码失败: {error}"))
                })?)
            }
            _ => {}
        }
    }
    Ok(output)
}

fn decode_text(text: &BytesText<'_>) -> Result<String, HostError> {
    let decoded = text
        .decode()
        .map_err(|error| HostError::validation(format!("DOCX 文本解码失败: {error}")))?;
    Ok(unescape(&decoded)
        .map_err(|error| HostError::validation(format!("DOCX 文本转义无效: {error}")))?
        .into_owned())
}

fn assert_responsibility_texts(events: &[Event<'static>]) -> Result<(), HostError> {
    let document_text = text_in_range(events, 0..events.len())?;
    if !RESPONSIBILITY_TEXTS
        .iter()
        .all(|expected| document_text.contains(expected))
    {
        return Err(HostError::validation("服务结算清单缺少固定验收职责说明"));
    }
    Ok(())
}

fn find_first_element(
    events: &[Event<'static>],
    search: Range<usize>,
    name: &[u8],
) -> Result<Option<Range<usize>>, HostError> {
    let mut start_index = None;
    let mut depth = 0_usize;
    for index in search {
        match &events[index] {
            Event::Start(start) if local_name(start) == name => {
                if start_index.is_none() {
                    start_index = Some(index);
                }
                depth += 1;
            }
            Event::Empty(start) if local_name(start) == name && start_index.is_none() => {
                return Ok(Some(index..index + 1));
            }
            Event::End(end) if end.local_name().as_ref() == name && start_index.is_some() => {
                if depth == 0 {
                    return Err(HostError::validation("DOCX XML 元素结构无效"));
                }
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(start_index.expect("start index exists")..index + 1));
                }
            }
            _ => {}
        }
    }
    if start_index.is_some() {
        return Err(HostError::validation("DOCX XML 元素未闭合"));
    }
    Ok(None)
}

fn remove_named_elements(xml: &[u8], names: &[&[u8]]) -> Result<Vec<u8>, HostError> {
    let events = parse_xml_events(xml, "DOCX XML")?;
    let mut output = Vec::with_capacity(events.len());
    let mut skip_depth = 0_usize;
    for event in events {
        if skip_depth > 0 {
            match &event {
                Event::Start(_) => skip_depth += 1,
                Event::End(_) => skip_depth -= 1,
                _ => {}
            }
            continue;
        }
        match &event {
            Event::Start(start) if names.contains(&local_name(start)) => skip_depth = 1,
            Event::Empty(start) if names.contains(&local_name(start)) => {}
            _ => output.push(event),
        }
    }
    if skip_depth != 0 {
        return Err(HostError::validation("DOCX XML 清理时元素未闭合"));
    }
    write_xml_events(output)
}

fn sanitize_relationships(xml: &[u8]) -> Result<Vec<u8>, HostError> {
    let events = parse_xml_events(xml, "DOCX relationships")?;
    let mut output = Vec::with_capacity(events.len());
    let mut skip_depth = 0_usize;
    let mut changed = false;
    for event in events {
        if skip_depth > 0 {
            match &event {
                Event::Start(_) => skip_depth += 1,
                Event::End(_) => skip_depth -= 1,
                _ => {}
            }
            continue;
        }
        let relationship = match &event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"Relationship" => {
                Some(start)
            }
            _ => None,
        };
        if let Some(start) = relationship {
            validate_relationship(start)?;
            let relationship_type = attribute_value(start, b"Type")?.unwrap_or_default();
            if relationship_type
                .to_ascii_lowercase()
                .ends_with("/attachedtemplate")
            {
                changed = true;
                if matches!(event, Event::Start(_)) {
                    skip_depth = 1;
                }
                continue;
            }
        }
        output.push(event);
    }
    if skip_depth != 0 {
        return Err(HostError::validation("DOCX relationships 清理时元素未闭合"));
    }
    if changed {
        write_xml_events(output)
    } else {
        Ok(xml.to_vec())
    }
}

fn parse_xml_events(xml: &[u8], label: &str) -> Result<Vec<Event<'static>>, HostError> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().check_end_names = true;
    reader.config_mut().trim_text(false);
    let mut events = Vec::new();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Eof) => break,
            Ok(Event::DocType(_)) => {
                return Err(HostError::validation(format!("{label} 不能包含 DTD")));
            }
            Ok(Event::GeneralRef(reference)) => {
                let name = reference.decode().map_err(|error| {
                    HostError::validation(format!("{label} 实体解码失败: {error}"))
                })?;
                decode_predefined_entity(name.as_ref())?;
                events.push(Event::GeneralRef(reference.into_owned()));
            }
            Ok(event) => events.push(event.into_owned()),
            Err(error) => {
                return Err(HostError::validation(format!(
                    "{label} XML 结构无效: {error}"
                )));
            }
        }
        buffer.clear();
    }
    Ok(events)
}

fn decode_predefined_entity(name: &str) -> Result<&'static str, HostError> {
    match name {
        "amp" => Ok("&"),
        "lt" => Ok("<"),
        "gt" => Ok(">"),
        "quot" => Ok("\""),
        "apos" => Ok("'"),
        _ => Err(HostError::validation(format!(
            "DOCX 包含非标准实体引用: &{name};"
        ))),
    }
}

fn write_xml_events(events: Vec<Event<'static>>) -> Result<Vec<u8>, HostError> {
    let mut writer = Writer::new(Vec::new());
    for event in events {
        writer
            .write_event(event)
            .map_err(|error| HostError::internal(format!("写入 DOCX XML 失败: {error}")))?;
    }
    Ok(writer.into_inner())
}

fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a [u8] {
    start.local_name().into_inner()
}

fn attribute_value(start: &BytesStart<'_>, name: &[u8]) -> Result<Option<String>, HostError> {
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute
            .map_err(|error| HostError::validation(format!("DOCX XML 属性无效: {error}")))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .decode_and_unescape_value(start.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|error| HostError::validation(format!("DOCX XML 属性解码失败: {error}")));
        }
    }
    Ok(None)
}

fn write_package(mut file: File, package: &Package) -> Result<(), HostError> {
    {
        let mut writer = ZipWriter::new(&mut file);
        for entry in &package.entries {
            if entry.is_dir {
                writer
                    .add_directory(&entry.name, entry.options)
                    .map_err(|error| HostError::internal(format!("写入 DOCX 目录失败: {error}")))?;
            } else {
                writer
                    .start_file(&entry.name, entry.options)
                    .map_err(|error| HostError::internal(format!("写入 DOCX 条目失败: {error}")))?;
                writer
                    .write_all(&entry.contents)
                    .map_err(|error| HostError::internal(format!("写入 DOCX 内容失败: {error}")))?;
            }
        }
        writer
            .finish()
            .map_err(|error| HostError::internal(format!("完成 DOCX 包失败: {error}")))?;
    }
    file.sync_all()
        .map_err(|error| HostError::internal(format!("同步 DOCX 临时文件失败: {error}")))
}

fn verify_output(path: &Path, data: &ServiceSettlementTemplateData) -> Result<(), HostError> {
    let output = read_file_limited(path, MAX_DOCX_SOURCE_BYTES)?;
    let package = load_package_from_bytes(&output)?;
    validate_standard_docx(&package)?;
    verify_core_properties(&package)?;
    verify_no_attached_template(&package)?;
    verify_no_sensitive_or_absolute_path_leaks(&package)?;

    let document = package_entry(&package, DOCUMENT_PATH)?;
    let events = parse_xml_events(document, DOCUMENT_PATH)?;
    assert_responsibility_texts(&events)?;
    let table = locate_service_table(&events, Some(data.items.len()))?;
    if table.rows.len() != data.items.len() + 2 {
        return Err(HostError::validation("服务结算清单输出行数校验失败"));
    }
    if !row_has_property(&events, &table.rows[0], b"tblHeader")?
        || !row_has_property(&events, &table.rows[0], b"cantSplit")?
    {
        return Err(HostError::validation(
            "服务结算清单表头必须重复且禁止跨页拆分",
        ));
    }
    for row in &table.rows[1..table.rows.len() - 1] {
        if !row_has_property(&events, row, b"cantSplit")? {
            return Err(HostError::validation("服务结算清单数据行必须禁止跨页拆分"));
        }
    }
    verify_unique_paragraph_ids(&events)?;

    for (index, (row, item)) in table.rows[1..table.rows.len() - 1]
        .iter()
        .zip(&data.items)
        .enumerate()
    {
        let actual = row
            .cells
            .iter()
            .map(|cell| cell_text(&events, cell))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = vec![
            (index + 1).to_string(),
            item.service_name.clone(),
            item.period.clone(),
            item.description.clone(),
            if item.provided_as_required == Some(true) {
                "☑是    □否".to_owned()
            } else {
                "□是    ☑否".to_owned()
            },
            item.evidence_label.clone(),
            item.remarks.clone(),
        ];
        if actual != expected {
            return Err(HostError::validation(format!(
                "服务结算清单第 {} 行输出复验失败",
                index + 1
            )));
        }
    }

    let signoff = table.rows.last().expect("verified table has signoff row");
    let signoff_text = row_text(&events, signoff)?;
    if signoff_text
        .chars()
        .any(|character| character.is_ascii_digit())
    {
        return Err(HostError::validation("服务结算清单签字区残留预填日期"));
    }
    Ok(())
}

fn row_has_property(
    events: &[Event<'static>],
    row: &RowRange,
    property: &[u8],
) -> Result<bool, HostError> {
    let Some(properties) = find_first_element(events, row.events.clone(), b"trPr")? else {
        return Ok(false);
    };
    Ok(events[properties].iter().any(|event| match event {
        Event::Start(start) | Event::Empty(start) => local_name(start) == property,
        _ => false,
    }))
}

fn verify_unique_paragraph_ids(events: &[Event<'static>]) -> Result<(), HostError> {
    let mut ids = HashSet::new();
    for event in events {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"p" => start,
            _ => continue,
        };
        for name in [b"paraId".as_slice(), b"textId".as_slice()] {
            let Some(value) = attribute_value(start, name)? else {
                continue;
            };
            if name == b"textId" && value == "77777777" {
                continue;
            }
            if !ids.insert((name, value)) {
                return Err(HostError::validation(
                    "服务结算清单输出包含重复 w14:paraId/w14:textId",
                ));
            }
        }
    }
    Ok(())
}

fn verify_core_properties(package: &Package) -> Result<(), HostError> {
    let Some(index) = package.index_by_name.get(CORE_PROPERTIES_PATH).copied() else {
        return Ok(());
    };
    let events = parse_xml_events(&package.entries[index].contents, CORE_PROPERTIES_PATH)?;
    if events.iter().any(|event| match event {
        Event::Start(start) | Event::Empty(start) => {
            matches!(local_name(start), b"creator" | b"lastModifiedBy")
        }
        _ => false,
    }) {
        return Err(HostError::validation("服务结算清单输出仍包含作者元数据"));
    }
    Ok(())
}

fn verify_no_attached_template(package: &Package) -> Result<(), HostError> {
    for entry in &package.entries {
        if entry.name == SETTINGS_PATH {
            let events = parse_xml_events(&entry.contents, SETTINGS_PATH)?;
            if events.iter().any(|event| match event {
                Event::Start(start) | Event::Empty(start) => {
                    local_name(start) == b"attachedTemplate"
                }
                _ => false,
            }) {
                return Err(HostError::validation(
                    "服务结算清单输出仍包含 attachedTemplate",
                ));
            }
        }
        if entry.name.ends_with(".rels") {
            let events = parse_xml_events(&entry.contents, &entry.name)?;
            for event in &events {
                let start = match event {
                    Event::Start(start) | Event::Empty(start)
                        if local_name(start) == b"Relationship" =>
                    {
                        start
                    }
                    _ => continue,
                };
                let relationship_type = attribute_value(start, b"Type")?.unwrap_or_default();
                if relationship_type
                    .to_ascii_lowercase()
                    .ends_with("/attachedtemplate")
                {
                    return Err(HostError::validation("服务结算清单输出仍包含模板关系"));
                }
            }
        }
    }
    Ok(())
}

fn verify_no_sensitive_or_absolute_path_leaks(package: &Package) -> Result<(), HostError> {
    for entry in package
        .entries
        .iter()
        .filter(|entry| entry.name.ends_with(".xml") || entry.name.ends_with(".rels"))
    {
        let text = std::str::from_utf8(&entry.contents).map_err(|error| {
            HostError::validation(format!("DOCX 文本条目不是 UTF-8 {}: {error}", entry.name))
        })?;
        if text.contains("rabbit") || text.contains("芷珺 何") {
            return Err(HostError::validation(format!(
                "DOCX 输出包含模板个人信息: {}",
                entry.name
            )));
        }
        if contains_absolute_path(text) {
            return Err(HostError::validation(format!(
                "DOCX 输出包含绝对路径: {}",
                entry.name
            )));
        }
    }
    Ok(())
}

fn contains_absolute_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("file:/") || lower.contains("/users/") || lower.contains("/home/") {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.windows(3).enumerate().any(|(index, window)| {
        let has_path_boundary = index == 0
            || matches!(
                bytes[index - 1],
                b'\'' | b'"' | b'=' | b'(' | b'[' | b'{' | b' ' | b'\t' | b'\r' | b'\n'
            );
        has_path_boundary
            && window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && matches!(window[2], b'\\' | b'/')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::escape::escape;
    use tempfile::TempDir;

    #[derive(Default)]
    struct FixtureOptions {
        grid_columns: usize,
        external_relationship: bool,
        attached_template: bool,
        macro_entry: bool,
        embedding_entry: bool,
        ole_relationship: bool,
        package_relationship: bool,
        object_element: bool,
        dde_field: bool,
        malformed_document: bool,
        traversal_entry: bool,
    }

    fn item(index: usize, provided: Option<bool>) -> ServiceSettlementItem {
        ServiceSettlementItem {
            service_name: format!("视频服务{index}"),
            period: format!("2026-07-{index:02}"),
            description: format!("第{index}组服务说明"),
            provided_as_required: provided,
            evidence_label: format!("证据{index}"),
            remarks: format!("备注{index}"),
        }
    }

    fn fixture_cell(text: &str, span: Option<usize>) -> String {
        let span = span
            .map(|value| format!(r#"<w:gridSpan w:val="{value}"/>"#))
            .unwrap_or_default();
        format!(
            r#"<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/>{span}</w:tcPr><w:p><w:pPr><w:jc w:val="center"/><w:rPr><w:rFonts w:eastAsia="仿宋"/><w:sz w:val="24"/></w:rPr></w:pPr><w:r><w:rPr><w:rFonts w:eastAsia="仿宋"/><w:sz w:val="24"/></w:rPr><w:t>{}</w:t></w:r></w:p></w:tc>"#,
            escape(text)
        )
    }

    fn fixture_status_cell() -> String {
        r#"<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p w14:paraId="7FF164E6" w14:textId="77777777"><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Segoe UI Symbol" w:hAnsi="Segoe UI Symbol"/></w:rPr><w:t>☑</w:t></w:r><w:r><w:rPr><w:rFonts w:eastAsia="仿宋"/></w:rPr><w:t>是</w:t></w:r><w:r><w:t xml:space="preserve">    </w:t></w:r></w:p><w:p w14:paraId="221374A6" w14:textId="6AB8C1A4"><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Segoe UI Symbol" w:hAnsi="Segoe UI Symbol"/></w:rPr><w:t>□</w:t></w:r><w:r><w:rPr><w:rFonts w:eastAsia="仿宋"/></w:rPr><w:t>否</w:t></w:r></w:p></w:tc>"#.to_owned()
    }

    fn fixture_document(grid_columns: usize, object_element: bool, dde_field: bool) -> String {
        let grid = (0..grid_columns)
            .map(|_| r#"<w:gridCol w:w="1000"/>"#)
            .collect::<String>();
        let header = HEADER_CELLS
            .iter()
            .map(|value| fixture_cell(value, None))
            .collect::<String>();
        let prototype = ["1", "", "", ""]
            .iter()
            .map(|value| fixture_cell(value, None))
            .chain(std::iter::once(fixture_status_cell()))
            .chain(
                ["盖章版文件", "已完成"]
                    .iter()
                    .map(|value| fixture_cell(value, None)),
            )
            .collect::<String>();
        let signoff = fixture_cell(
            "乙方验收人签字： 日期：2026 年 月 日 甲方验收人签字： 日期：2026 年 月 日 甲方验收监管人签字： 日期：2026 年 月 日",
            Some(7),
        );
        let active_content = format!(
            "{}{}",
            if object_element {
                r#"<w:p><w:r><w:object/></w:r></w:p>"#
            } else {
                ""
            },
            if dde_field {
                r#"<w:p><w:r><w:instrText>DDEAUTO c:\\windows\\system32\\cmd.exe</w:instrText></w:r></w:p>"#
            } else {
                ""
            }
        );
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>服务结算清单</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="7000" w:type="dxa"/></w:tblPr><w:tblGrid>{grid}</w:tblGrid><w:tr>{header}</w:tr><w:tr>{prototype}</w:tr><w:tr>{signoff}</w:tr></w:tbl><w:p><w:r><w:t>验收职责：</w:t></w:r><w:r><w:t>{}</w:t></w:r><w:r><w:t>{}</w:t></w:r><w:r><w:t>{}</w:t></w:r></w:p>{active_content}<w:sectPr/></w:body></w:document>"#,
            RESPONSIBILITY_TEXTS[0], RESPONSIBILITY_TEXTS[1], RESPONSIBILITY_TEXTS[2]
        )
    }

    fn write_fixture(directory: &TempDir, options: FixtureOptions) -> PathBuf {
        let source = directory.path().join("source.docx");
        let file = File::create(&source).unwrap();
        let mut writer = ZipWriter::new(file);
        let file_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        let main_content_type = if options.macro_entry {
            "application/vnd.ms-word.document.macroEnabled.main+xml"
        } else {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
        };
        let content_types = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="{main_content_type}"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#
        );
        write_fixture_entry(
            &mut writer,
            CONTENT_TYPES_PATH,
            &content_types,
            file_options,
        );
        write_fixture_entry(
            &mut writer,
            ROOT_RELATIONSHIPS_PATH,
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
            file_options,
        );
        let document = if options.malformed_document {
            "<w:document><w:body>"
        } else {
            &fixture_document(
                if options.grid_columns == 0 {
                    7
                } else {
                    options.grid_columns
                },
                options.object_element,
                options.dde_field,
            )
        };
        write_fixture_entry(&mut writer, DOCUMENT_PATH, document, file_options);

        let mut relationships = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#,
        );
        if options.external_relationship {
            relationships.push_str(r#"<Relationship Id="rIdExternal" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid/secret" TargetMode="External"/>"#);
        }
        if options.attached_template {
            relationships.push_str(r#"<Relationship Id="rIdAttached" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate" Target="template.dotx"/>"#);
        }
        if options.ole_relationship {
            relationships.push_str(r#"<Relationship Id="rIdOle" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject" Target="embeddings/oleObject1.bin"/>"#);
        }
        if options.package_relationship {
            relationships.push_str(r#"<Relationship Id="rIdPackage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/package" Target="embeddings/package1.bin"/>"#);
        }
        relationships.push_str("</Relationships>");
        write_fixture_entry(
            &mut writer,
            "word/_rels/document.xml.rels",
            &relationships,
            file_options,
        );

        let settings = if options.attached_template {
            r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:attachedTemplate r:id="rIdAttached"/><w:zoom w:percent="100"/></w:settings>"#
        } else {
            r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:zoom w:percent="100"/></w:settings>"#
        };
        write_fixture_entry(&mut writer, SETTINGS_PATH, settings, file_options);
        write_fixture_entry(
            &mut writer,
            CORE_PROPERTIES_PATH,
            r#"<?xml version="1.0" encoding="UTF-8"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>rabbit</dc:creator><cp:lastModifiedBy>芷珺 何</cp:lastModifiedBy><cp:revision>14</cp:revision></cp:coreProperties>"#,
            file_options,
        );
        if options.macro_entry {
            writer
                .start_file("word/vbaProject.bin", file_options)
                .unwrap();
            writer.write_all(b"not-a-real-macro").unwrap();
        }
        if options.embedding_entry {
            writer
                .start_file("word/embeddings/oleObject1.bin", file_options)
                .unwrap();
            writer.write_all(b"not-a-real-ole-object").unwrap();
        }
        if options.traversal_entry {
            writer.start_file("../escape.xml", file_options).unwrap();
            writer.write_all(b"<escape/>").unwrap();
        }
        writer.finish().unwrap();
        source
    }

    fn write_fixture_entry(
        writer: &mut ZipWriter<File>,
        name: &str,
        contents: &str,
        options: SimpleFileOptions,
    ) {
        writer.start_file(name, options).unwrap();
        writer.write_all(contents.as_bytes()).unwrap();
    }

    fn clone_fixture(
        directory: &TempDir,
        source: &Path,
        data: &ServiceSettlementTemplateData,
    ) -> Result<PathBuf, HostError> {
        let destination = directory.path().join("output.docx");
        let hash = sha256_file(source).unwrap();
        clone_service_settlement_template_impl(source, &hash, &destination, data)?;
        Ok(destination)
    }

    fn clone_service_settlement_template_impl(
        source: &Path,
        expected_sha256: &str,
        destination: &Path,
        data: &ServiceSettlementTemplateData,
    ) -> Result<(), HostError> {
        validate_paths(source, destination)?;
        let source = read_file_limited(source, MAX_DOCX_SOURCE_BYTES)?;
        clone_service_settlement_template_from_bytes_impl(
            &source,
            expected_sha256,
            destination,
            data,
        )
    }

    fn load_package(path: &Path) -> Result<Package, HostError> {
        let source = read_file_limited(path, MAX_DOCX_SOURCE_BYTES)?;
        load_package_from_bytes(&source)
    }

    fn output_rows(path: &Path, count: usize) -> (Vec<Event<'static>>, TableRange) {
        let package = load_package(path).unwrap();
        let events = parse_xml_events(
            package_entry(&package, DOCUMENT_PATH).unwrap(),
            DOCUMENT_PATH,
        )
        .unwrap();
        let table = locate_service_table(&events, Some(count)).unwrap();
        (events, table)
    }

    #[test]
    fn clones_one_item_with_chinese_and_xml_special_characters() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let data = ServiceSettlementTemplateData {
            items: vec![ServiceSettlementItem {
                service_name: "中文<&>\"'服务".to_owned(),
                period: "2026年7月".to_owned(),
                description: "A & B < C > D".to_owned(),
                provided_as_required: Some(true),
                evidence_label: "证明<&>".to_owned(),
                remarks: "人工确认已完成".to_owned(),
            }],
        };
        let destination = clone_fixture(&directory, &source, &data).unwrap();
        let (events, table) = output_rows(&destination, 1);
        let values = table.rows[1]
            .cells
            .iter()
            .map(|cell| cell_text(&events, cell).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values[1], "中文<&>\"'服务");
        assert_eq!(values[3], "A & B < C > D");
        assert_eq!(values[5], "证明<&>");
        assert_eq!(table.rows.len(), 3);
        assert!(destination.exists());
    }

    #[test]
    fn clones_from_the_same_bytes_used_for_hash_validation() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let source = read_file_limited(&source, MAX_DOCX_SOURCE_BYTES).unwrap();
        let destination = directory.path().join("from-bytes.docx");
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };

        clone_service_settlement_template_from_bytes_impl(
            &source,
            &sha256_bytes(&source),
            &destination,
            &data,
        )
        .unwrap();

        verify_output(&destination, &data).unwrap();
    }

    #[test]
    fn expands_to_three_rows_and_preserves_fixed_sections() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let data = ServiceSettlementTemplateData {
            items: (1..=3).map(|index| item(index, Some(true))).collect(),
        };
        let destination = clone_fixture(&directory, &source, &data).unwrap();
        let package = load_package(&destination).unwrap();
        let events = parse_xml_events(
            package_entry(&package, DOCUMENT_PATH).unwrap(),
            DOCUMENT_PATH,
        )
        .unwrap();
        let table = locate_service_table(&events, Some(3)).unwrap();
        assert_eq!(table.rows.len(), 5);
        assert!(row_has_property(&events, &table.rows[0], b"tblHeader").unwrap());
        assert!(row_has_property(&events, &table.rows[0], b"cantSplit").unwrap());
        assert!(table.rows[1..4]
            .iter()
            .all(|row| row_has_property(&events, row, b"cantSplit").unwrap()));
        verify_unique_paragraph_ids(&events).unwrap();
        for row in &table.rows[1..4] {
            for event in &events[row.events.clone()] {
                let start = match event {
                    Event::Start(start) | Event::Empty(start) if local_name(start) == b"p" => start,
                    _ => continue,
                };
                assert!(attribute_value(start, b"paraId").unwrap().is_none());
                assert!(attribute_value(start, b"textId").unwrap().is_none());
            }
        }
        assert_responsibility_texts(&events).unwrap();
        let signoff = row_text(&events, table.rows.last().unwrap()).unwrap();
        assert!(SIGNOFF_LABELS.iter().all(|label| signoff.contains(label)));
        assert!(!signoff.contains("2026"));
    }

    #[test]
    fn rejects_more_than_three_rows_until_real_pagination_is_implemented() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let data = ServiceSettlementTemplateData {
            items: (1..=4).map(|index| item(index, Some(true))).collect(),
        };
        let error = clone_fixture(&directory, &source, &data).unwrap_err();
        assert!(error.message.contains("真实分页尚未完成"));
        assert!(error.message.contains("最多可生成 3 条"));
    }

    #[test]
    fn rejects_unconfirmed_status_without_creating_output() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let destination = directory.path().join("output.docx");
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, None)],
        };
        let hash = sha256_file(&source).unwrap();
        assert!(clone_service_settlement_template(&source, &hash, &destination, &data).is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn false_status_writes_only_no() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(false))],
        };
        let destination = clone_fixture(&directory, &source, &data).unwrap();
        let (events, table) = output_rows(&destination, 1);
        let status = cell_text(&events, &table.rows[1].cells[4]).unwrap();
        assert_eq!(status, "□是    ☑否");
        let paragraphs =
            collect_element_ranges(&events, table.rows[1].cells[4].events.clone(), b"p").unwrap();
        assert_eq!(paragraphs.len(), 2);
        let symbol_fonts = events[table.rows[1].cells[4].events.clone()]
            .iter()
            .filter_map(|event| match event {
                Event::Start(start) | Event::Empty(start) if local_name(start) == b"rFonts" => {
                    attribute_value(start, b"ascii").unwrap()
                }
                _ => None,
            })
            .filter(|font| font == "Segoe UI Symbol")
            .count();
        assert_eq!(symbol_fonts, 2);
    }

    #[test]
    fn true_status_preserves_two_checkbox_paragraphs_and_symbol_font() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        let destination = clone_fixture(&directory, &source, &data).unwrap();
        let (events, table) = output_rows(&destination, 1);
        let cell = &table.rows[1].cells[4];
        assert_eq!(cell_text(&events, cell).unwrap(), "☑是    □否");
        assert_eq!(
            collect_element_ranges(&events, cell.events.clone(), b"p")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn rejects_hash_mismatch_and_preserves_existing_destination() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let destination = directory.path().join("output.docx");
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        let mismatch = "0".repeat(64);
        assert!(
            clone_service_settlement_template_impl(&source, &mismatch, &destination, &data)
                .is_err()
        );
        assert!(!destination.exists());

        fs::write(&destination, b"existing").unwrap();
        let hash = sha256_file(&source).unwrap();
        assert!(
            clone_service_settlement_template_impl(&source, &hash, &destination, &data).is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
    }

    #[test]
    fn rejects_wrong_table_structure_and_removes_atomic_staging_file() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(
            &directory,
            FixtureOptions {
                grid_columns: 6,
                ..FixtureOptions::default()
            },
        );
        let destination = directory.path().join("output.docx");
        let hash = sha256_file(&source).unwrap();
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        assert!(clone_service_settlement_template(&source, &hash, &destination, &data).is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn rejects_external_relationship_macro_and_path_escape() {
        for options in [
            FixtureOptions {
                external_relationship: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                macro_entry: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                traversal_entry: true,
                ..FixtureOptions::default()
            },
        ] {
            let directory = TempDir::new().unwrap();
            let source = write_fixture(&directory, options);
            let destination = directory.path().join("output.docx");
            let hash = sha256_file(&source).unwrap();
            let data = ServiceSettlementTemplateData {
                items: vec![item(1, Some(true))],
            };
            assert!(
                clone_service_settlement_template(&source, &hash, &destination, &data).is_err()
            );
            assert!(!destination.exists());
        }
    }

    #[test]
    fn rejects_embeddings_ole_package_object_and_dde_active_content() {
        for options in [
            FixtureOptions {
                embedding_entry: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                ole_relationship: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                package_relationship: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                object_element: true,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                dde_field: true,
                ..FixtureOptions::default()
            },
        ] {
            let directory = TempDir::new().unwrap();
            let source = write_fixture(&directory, options);
            let destination = directory.path().join("output.docx");
            let hash = sha256_file(&source).unwrap();
            let data = ServiceSettlementTemplateData {
                items: vec![item(1, Some(true))],
            };
            assert!(
                clone_service_settlement_template_impl(&source, &hash, &destination, &data)
                    .is_err()
            );
            assert!(!destination.exists());
        }
    }

    #[test]
    fn public_white_goose_pond_entry_rejects_unregistered_template_hash() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(&directory, FixtureOptions::default());
        let destination = directory.path().join("output.docx");
        let hash = sha256_file(&source).unwrap();
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        let error =
            clone_service_settlement_template(&source, &hash, &destination, &data).unwrap_err();
        assert!(error.message.contains("必须绑定登记模板 SHA-256"));
        assert!(!destination.exists());
    }

    #[test]
    fn atomic_publish_falls_back_without_overwriting_existing_destination() {
        let directory = TempDir::new().unwrap();
        let destination = directory.path().join("output.docx");
        let (staged, mut file) = StagedOutput::create(&destination).unwrap();
        file.write_all(b"new-output").unwrap();
        file.sync_all().unwrap();
        drop(file);
        staged
            .publish_with(&destination, |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "simulated hard-link rejection",
                ))
            })
            .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new-output");

        let (staged, mut file) = StagedOutput::create(&destination).unwrap();
        file.write_all(b"must-not-overwrite").unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(staged
            .publish_with(&destination, |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "simulated hard-link rejection",
                ))
            })
            .is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"new-output");
    }

    #[test]
    fn removes_attached_template_and_personal_metadata() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(
            &directory,
            FixtureOptions {
                attached_template: true,
                ..FixtureOptions::default()
            },
        );
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        let destination = clone_fixture(&directory, &source, &data).unwrap();
        let package = load_package(&destination).unwrap();
        verify_core_properties(&package).unwrap();
        verify_no_attached_template(&package).unwrap();
        let core =
            std::str::from_utf8(package_entry(&package, CORE_PROPERTIES_PATH).unwrap()).unwrap();
        assert!(!core.contains("rabbit"));
        assert!(!core.contains("芷珺 何"));
    }

    #[test]
    fn malformed_document_failure_leaves_no_partial_output() {
        let directory = TempDir::new().unwrap();
        let source = write_fixture(
            &directory,
            FixtureOptions {
                malformed_document: true,
                ..FixtureOptions::default()
            },
        );
        let destination = directory.path().join("output.docx");
        let hash = sha256_file(&source).unwrap();
        let data = ServiceSettlementTemplateData {
            items: vec![item(1, Some(true))],
        };
        assert!(clone_service_settlement_template(&source, &hash, &destination, &data).is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    #[ignore = "requires the read-only White Goose Pond source template"]
    fn real_template_read_only_clone() {
        let source = std::env::var_os("BSAIGC_SERVICE_SETTLEMENT_TEMPLATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("tests/fixtures/synthetic/business-v1/templates/synthetic-service-settlement.docx")
            });
        assert!(
            source.is_file(),
            "真实模板不存在: {}。请将 BSAIGC_SERVICE_SETTLEMENT_TEMPLATE 设为可读 DOCX 路径；显式配置后不会静默跳过。",
            source.display()
        );
        assert_eq!(
            sha256_file(&source).unwrap(),
            SERVICE_SETTLEMENT_TEMPLATE_SHA256
        );
        let directory = TempDir::new().unwrap();
        let destination = directory.path().join("服务结算清单.docx");
        let data = ServiceSettlementTemplateData {
            items: (1..=3).map(|index| item(index, Some(true))).collect(),
        };
        let source = read_file_limited(&source, MAX_DOCX_SOURCE_BYTES).unwrap();
        clone_service_settlement_template_from_bytes(
            &source,
            SERVICE_SETTLEMENT_TEMPLATE_SHA256,
            &destination,
            &data,
        )
        .unwrap();
        verify_output(&destination, &data).unwrap();
    }
}

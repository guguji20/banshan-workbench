use crate::protocol::HostError;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY: &str =
    "project.baietan.acceptance.video-completion-acceptance.v1";
pub const VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256: &str =
    "CF9E21CEC8C5458F709410A17350B58D066EA98F3E6F15194598EFCFAA38B5FB";

const DOCUMENT_PATH: &str = "word/document.xml";
const DOCUMENT_RELS_PATH: &str = "word/_rels/document.xml.rels";
const CONTENT_TYPES_PATH: &str = "[Content_Types].xml";
const ROOT_RELS_PATH: &str = "_rels/.rels";
const SETTINGS_PATH: &str = "word/settings.xml";
const CORE_PROPERTIES_PATH: &str = "docProps/core.xml";
const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 2_048;
const MAX_FIELD_BYTES: usize = 16 * 1024;
const MAX_GROUPS: usize = 32;
const MAX_VIDEOS: usize = 128;
const MAX_SCREENSHOTS_PER_VIDEO: usize = 8;
const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMAGE_WIDTH: u32 = 16_000;
const MAX_IMAGE_HEIGHT: u32 = 16_000;

static STAGED_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoAssetReference {
    pub asset_id: String,
    pub file_name: String,
    pub sha256: String,
    pub external_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoScreenshot {
    pub asset_id: String,
    pub sha256: String,
    pub caption: String,
    pub mime_type: String,
    pub image_bytes: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoBlock {
    pub title: String,
    pub video_type: String,
    pub content: String,
    pub duration: String,
    pub asset_reference: VideoAssetReference,
    pub screenshots: Vec<VideoScreenshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDeliveryGroup {
    pub name: String,
    pub service_description: String,
    pub videos: Vec<VideoBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCompletionAcceptanceTemplateData {
    pub contract_title: String,
    pub project_title: String,
    pub completion_date: String,
    pub delivery_groups: Vec<VideoDeliveryGroup>,
    pub acceptance_conclusion: String,
    pub manually_confirmed: bool,
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
    events: Range<usize>,
    rows: Vec<RowRange>,
    grid_columns: usize,
}

#[derive(Debug, Clone)]
struct ImagePlan {
    drawing_id: u32,
    rel_id: String,
    path: String,
    content_type: String,
    bytes: Vec<u8>,
}

pub(crate) fn validate_video_completion_acceptance_template_source_from_bytes(
    source: &[u8],
    expected_sha256: &str,
) -> Result<(), HostError> {
    validate_expected_template_hash(expected_sha256)?;
    validate_source_package(source, expected_sha256)
}

pub(crate) fn render_video_completion_acceptance_template(
    source: &Path,
    expected_sha256: &str,
    destination: &Path,
    data: &VideoCompletionAcceptanceTemplateData,
) -> Result<(), HostError> {
    validate_paths(source, destination)?;
    let source_bytes = read_file_limited(source, MAX_SOURCE_BYTES)?;
    render_video_completion_acceptance_template_from_bytes(
        &source_bytes,
        expected_sha256,
        destination,
        data,
    )
}

pub(crate) fn render_video_completion_acceptance_template_from_bytes(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &VideoCompletionAcceptanceTemplateData,
) -> Result<(), HostError> {
    validate_expected_template_hash(expected_sha256)?;
    validate_data(data)?;
    validate_destination(destination)?;
    validate_source_package(source, expected_sha256)?;

    let mut package = load_package_from_bytes(source)?;
    let mut image_plans = Vec::new();
    let mut image_index = 0_usize;
    for group in &data.delivery_groups {
        for video in &group.videos {
            for screenshot in &video.screenshots {
                let (extension, content_type) = image_format(screenshot)?;
                image_plans.push(ImagePlan {
                    drawing_id: u32::try_from(image_index + 1)
                        .map_err(|_| validation("截图数量超过 DrawingML 标识范围"))?,
                    rel_id: format!("rIdVideoAcceptance{image_index}"),
                    path: format!("word/media/video-acceptance-{image_index}.{extension}"),
                    content_type: content_type.to_owned(),
                    bytes: screenshot.image_bytes.clone(),
                });
                image_index += 1;
            }
        }
    }

    let document = package_entry(&package, DOCUMENT_PATH)?.to_vec();
    let transformed = transform_document(&document, data, &image_plans)?;
    replace_package_entry(&mut package, DOCUMENT_PATH, transformed)?;
    add_image_relationships(&mut package, &image_plans)?;
    add_image_content_types(&mut package, &image_plans)?;
    for plan in &image_plans {
        package.entries.push(PackageEntry {
            name: plan.path.clone(),
            options: SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o644),
            is_dir: false,
            contents: plan.bytes.clone(),
        });
        package
            .index_by_name
            .insert(plan.path.clone(), package.entries.len() - 1);
    }
    sanitize_package(&mut package)?;
    let output = write_package(&package)?;
    publish_no_replace(destination, &output)?;

    let rendered = read_file_limited(destination, MAX_SOURCE_BYTES)?;
    verify_rendered_document(&rendered, data, &image_plans)
}

fn validate_source_package(source: &[u8], expected_sha256: &str) -> Result<(), HostError> {
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(validation(format!(
            "视频成片验收模板不能超过 {MAX_SOURCE_BYTES} 字节"
        )));
    }
    let actual = sha256_bytes(source);
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        return Err(validation(format!(
            "视频成片验收模板 SHA-256 不匹配，期望 {expected_sha256}，实际 {actual}"
        )));
    }
    let package = load_package_from_bytes(source)?;
    validate_safe_docx(&package)?;
    let events = parse_xml_events(package_entry(&package, DOCUMENT_PATH)?, DOCUMENT_PATH)?;
    validate_source_structure(&events)
}

fn validate_source_structure(events: &[Event<'static>]) -> Result<(), HostError> {
    let table = locate_table(events)?;
    if table.grid_columns != 4 || table.rows.len() != 6 {
        return Err(validation("视频成片验收模板必须保持登记的 4 列 6 行源结构"));
    }
    let expected = [
        (0, "项目名称", "完成时间"),
        (1, "视频主题", "时长"),
        (2, "视频截图", ""),
        (3, "视频主题", "时长"),
        (4, "验收结论", ""),
        (5, "甲方", "乙方"),
    ];
    for (index, left, right) in expected {
        let text = row_text(events, &table.rows[index])?;
        if !text.contains(left) || (!right.is_empty() && !text.contains(right)) {
            return Err(validation(format!(
                "视频成片验收模板第 {} 行语义锚点不匹配",
                index + 1
            )));
        }
    }
    Ok(())
}

fn validate_data(data: &VideoCompletionAcceptanceTemplateData) -> Result<(), HostError> {
    validate_field("contractTitle", &data.contract_title, true)?;
    validate_field("projectTitle", &data.project_title, true)?;
    validate_field("completionDate", &data.completion_date, true)?;
    validate_field("acceptanceConclusion", &data.acceptance_conclusion, true)?;
    if !data.manually_confirmed {
        return Err(validation("视频成片验收正式版必须经过人工确认"));
    }
    if data.delivery_groups.is_empty() || data.delivery_groups.len() > MAX_GROUPS {
        return Err(validation(format!(
            "deliveryGroups 数量必须在 1 到 {MAX_GROUPS} 之间"
        )));
    }
    let mut video_count = 0_usize;
    for (group_index, group) in data.delivery_groups.iter().enumerate() {
        validate_field(
            &format!("deliveryGroups[{group_index}].name"),
            &group.name,
            true,
        )?;
        validate_field(
            &format!("deliveryGroups[{group_index}].serviceDescription"),
            &group.service_description,
            true,
        )?;
        if group.videos.is_empty() {
            return Err(validation(format!(
                "deliveryGroups[{group_index}] 不能没有视频"
            )));
        }
        for (video_index, video) in group.videos.iter().enumerate() {
            video_count += 1;
            if video_count > MAX_VIDEOS {
                return Err(validation(format!("视频数量不能超过 {MAX_VIDEOS}")));
            }
            for (field, value) in [
                ("title", &video.title),
                ("type", &video.video_type),
                ("content", &video.content),
                ("duration", &video.duration),
            ] {
                validate_field(
                    &format!("deliveryGroups[{group_index}].videos[{video_index}].{field}"),
                    value,
                    true,
                )?;
            }
            validate_asset_reference(
                &format!("deliveryGroups[{group_index}].videos[{video_index}].assetReference"),
                &video.asset_reference,
            )?;
            if video.screenshots.is_empty() || video.screenshots.len() > MAX_SCREENSHOTS_PER_VIDEO {
                return Err(validation(format!(
                    "deliveryGroups[{group_index}].videos[{video_index}].screenshots 数量必须在 1 到 {MAX_SCREENSHOTS_PER_VIDEO} 之间"
                )));
            }
            for (screenshot_index, screenshot) in video.screenshots.iter().enumerate() {
                validate_screenshot(
                    &format!(
                        "deliveryGroups[{group_index}].videos[{video_index}].screenshots[{screenshot_index}]"
                    ),
                    screenshot,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_asset_reference(label: &str, value: &VideoAssetReference) -> Result<(), HostError> {
    validate_identifier(&format!("{label}.assetId"), &value.asset_id)?;
    validate_field(&format!("{label}.fileName"), &value.file_name, true)?;
    validate_sha256(&format!("{label}.sha256"), &value.sha256)?;
    if let Some(link) = &value.external_link {
        validate_field(&format!("{label}.externalLink"), link, true)?;
        if !(link.starts_with("https://") || link.starts_with("http://")) {
            return Err(validation(format!(
                "{label}.externalLink 只允许 HTTP(S) 证据链接"
            )));
        }
    }
    Ok(())
}

fn validate_screenshot(label: &str, value: &VideoScreenshot) -> Result<(), HostError> {
    validate_identifier(&format!("{label}.assetId"), &value.asset_id)?;
    validate_sha256(&format!("{label}.sha256"), &value.sha256)?;
    validate_field(&format!("{label}.caption"), &value.caption, false)?;
    if value.image_bytes.is_empty() || value.image_bytes.len() > MAX_IMAGE_BYTES {
        return Err(validation(format!("{label}.imageBytes 大小无效")));
    }
    if value.width_px == 0
        || value.height_px == 0
        || value.width_px > MAX_IMAGE_WIDTH
        || value.height_px > MAX_IMAGE_HEIGHT
    {
        return Err(validation(format!("{label} 图片尺寸无效")));
    }
    if !sha256_bytes(&value.image_bytes).eq_ignore_ascii_case(&value.sha256) {
        return Err(validation(format!("{label} 图片 SHA-256 与字节不一致")));
    }
    image_format(value)?;
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), HostError> {
    validate_field(label, value, true)?;
    if value.chars().any(|character| character.is_whitespace()) {
        return Err(validation(format!("{label} 不能包含空白字符")));
    }
    Ok(())
}

fn validate_field(label: &str, value: &str, required: bool) -> Result<(), HostError> {
    if required && value.trim().is_empty() {
        return Err(validation(format!("{label} 不能为空")));
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(validation(format!("{label} 超过 {MAX_FIELD_BYTES} 字节")));
    }
    if value
        .chars()
        .any(|character| !is_valid_xml_character(character))
    {
        return Err(validation(format!("{label} 包含非法 XML 字符")));
    }
    Ok(())
}

fn validate_sha256(label: &str, value: &str) -> Result<(), HostError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(validation(format!("{label} 必须是 64 位十六进制 SHA-256")));
    }
    Ok(())
}

fn validate_expected_template_hash(value: &str) -> Result<(), HostError> {
    if !value.eq_ignore_ascii_case(VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256) {
        return Err(validation(format!(
            "模板键 {VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_KEY} 必须绑定登记 SHA-256 {VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256}"
        )));
    }
    validate_sha256("templateSha256", value)
}

fn image_format(screenshot: &VideoScreenshot) -> Result<(&'static str, &'static str), HostError> {
    let png = screenshot.image_bytes.starts_with(b"\x89PNG\r\n\x1a\n");
    let jpeg = screenshot.image_bytes.starts_with(&[0xff, 0xd8, 0xff]);
    let mime = screenshot.mime_type.trim().to_ascii_lowercase();
    if png && mime == "image/png" {
        return Ok(("png", "image/png"));
    }
    if jpeg && mime == "image/jpeg" {
        return Ok(("jpeg", "image/jpeg"));
    }
    Err(validation("截图只允许 MIME 与内容一致的 PNG 或 JPEG"))
}

fn transform_document(
    xml: &[u8],
    data: &VideoCompletionAcceptanceTemplateData,
    image_plans: &[ImagePlan],
) -> Result<Vec<u8>, HostError> {
    let mut events = parse_xml_events(xml, DOCUMENT_PATH)?;
    add_picture_namespaces(&mut events)?;
    replace_contract_paragraph(&mut events, &data.contract_title)?;
    let table = locate_table(&events)?;
    if table.rows.len() != 6 || table.grid_columns != 4 {
        return Err(validation("视频成片验收源表格结构不符合登记版本"));
    }

    let project_row = build_row_values(
        &events,
        &table.rows[0],
        &[
            "项目名称：",
            data.project_title.as_str(),
            "完成时间：",
            &data.completion_date,
        ],
    )?;
    let conclusion_row = build_row_values(
        &events,
        &table.rows[4],
        &["验收结论：", &data.acceptance_conclusion],
    )?;
    let conclusion_row = add_row_properties(&conclusion_row, &["w:cantSplit"])?;
    let conclusion_row = add_keep_next(&add_page_break_before(&conclusion_row)?)?;
    let signoff_row = build_signoff_row(&events, &table.rows[5])?;
    let signoff_row = add_row_properties(&signoff_row, &["w:cantSplit"])?;

    let mut dynamic_rows = Vec::new();
    let mut screenshot_index = 0_usize;
    for group in &data.delivery_groups {
        let group_row = build_row_values(
            &events,
            &table.rows[1],
            &[
                "交付组：",
                group.name.as_str(),
                "组说明：",
                group.service_description.as_str(),
            ],
        )?;
        dynamic_rows.push(add_keep_next(&add_row_properties(
            &group_row,
            &["w:cantSplit"],
        )?)?);
        for video in &group.videos {
            let metadata = build_row_values(
                &events,
                &table.rows[1],
                &[
                    "视频主题：",
                    video.title.as_str(),
                    "时长：",
                    video.duration.as_str(),
                ],
            )?;
            dynamic_rows.push(add_keep_next(&add_row_properties(
                &metadata,
                &["w:cantSplit"],
            )?)?);
            let content = build_content_row(
                &events,
                &table.rows[2],
                video,
                image_plans,
                &mut screenshot_index,
            )?;
            dynamic_rows.push(add_row_properties(&content, &["w:cantSplit"])?);
        }
    }

    let mut output = events[..table.rows[0].events.start].to_vec();
    output.extend(project_row);
    output.extend(
        events[table.rows[0].events.end..table.rows[1].events.start]
            .iter()
            .cloned(),
    );
    for row in &dynamic_rows {
        output.extend(row.iter().cloned());
    }
    output.extend(conclusion_row);
    output.extend(signoff_row);
    output.extend(events[table.rows[5].events.end..].iter().cloned());
    write_xml_events(output)
}

fn build_content_row(
    events: &[Event<'static>],
    prototype: &RowRange,
    video: &VideoBlock,
    image_plans: &[ImagePlan],
    screenshot_index: &mut usize,
) -> Result<Vec<Event<'static>>, HostError> {
    if prototype.cells.len() != 2 || prototype.cells[0].logical_columns != 1 {
        return Err(validation("视频截图原型行必须是左标签加三列合并内容区"));
    }
    let mut output = events[prototype.events.start..prototype.cells[0].events.start].to_vec();
    output.extend(events[prototype.cells[0].events.clone()].iter().cloned());
    output.extend(build_content_cell(
        events,
        &prototype.cells[1],
        video,
        image_plans,
        screenshot_index,
    )?);
    output.extend(
        events[prototype.cells[1].events.end..prototype.events.end]
            .iter()
            .cloned(),
    );
    Ok(output)
}

fn build_content_cell(
    events: &[Event<'static>],
    cell: &CellRange,
    video: &VideoBlock,
    image_plans: &[ImagePlan],
    screenshot_index: &mut usize,
) -> Result<Vec<Event<'static>>, HostError> {
    let properties = first_element_range(events, cell.events.clone(), b"tcPr")?
        .ok_or_else(|| validation("视频截图内容单元格缺少 tcPr"))?;
    let mut output = events[cell.events.start..properties.end].to_vec();
    output.extend(text_paragraph("视频说明：", &video.content, true)?);
    output.extend(text_paragraph(
        "视频资产：",
        &format_asset_reference(&video.asset_reference),
        true,
    )?);
    if let Some(link) = &video.asset_reference.external_link {
        output.extend(text_paragraph("证据链接：", link, true)?);
    }
    for screenshot in &video.screenshots {
        let plan = image_plans
            .get(*screenshot_index)
            .ok_or_else(|| validation("截图关系数量与 DOCX media 计划不一致"))?;
        output.extend(text_paragraph(
            "截图来源：",
            &format!("{} / SHA-256 {}", screenshot.asset_id, screenshot.sha256),
            true,
        )?);
        output.extend(image_paragraph(
            plan,
            screenshot,
            !screenshot.caption.trim().is_empty(),
        )?);
        if !screenshot.caption.trim().is_empty() {
            output.extend(text_paragraph("截图说明：", &screenshot.caption, false)?);
        }
        *screenshot_index += 1;
    }
    output.push(Event::End(BytesEnd::new("w:tc")));
    Ok(output)
}

fn build_row_values(
    events: &[Event<'static>],
    row: &RowRange,
    values: &[&str],
) -> Result<Vec<Event<'static>>, HostError> {
    if row.cells.len() != values.len() {
        return Err(validation("视频成片验收模板行的物理列数不符合映射"));
    }
    let mut output = events[row.events.start..row.cells[0].events.start].to_vec();
    for (cell, value) in row.cells.iter().zip(values) {
        output.extend(replace_cell_text(events, cell, value)?);
    }
    output.extend(
        events[row.cells.last().expect("row has cells").events.end..row.events.end]
            .iter()
            .cloned(),
    );
    Ok(output)
}

fn build_signoff_row(
    events: &[Event<'static>],
    row: &RowRange,
) -> Result<Vec<Event<'static>>, HostError> {
    if row.cells.len() != 2 {
        return Err(validation("视频成片验收签章行必须是双栏"));
    }
    let customer = "甲方：\n代表签字：\n日期：    年    月    日";
    let supplier = "乙方：\n代表签字：\n日期：    年    月    日";
    let mut output = events[row.events.start..row.cells[0].events.start].to_vec();
    output.extend(replace_cell_text(events, &row.cells[0], customer)?);
    output.extend(replace_cell_text(events, &row.cells[1], supplier)?);
    output.extend(
        events[row.cells[1].events.end..row.events.end]
            .iter()
            .cloned(),
    );
    Ok(output)
}

fn replace_cell_text(
    events: &[Event<'static>],
    cell: &CellRange,
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    replace_cell_body(events, cell, text_paragraphs(value, false)?)
}

fn replace_cell_body(
    events: &[Event<'static>],
    cell: &CellRange,
    body: Vec<Event<'static>>,
) -> Result<Vec<Event<'static>>, HostError> {
    let properties = first_element_range(events, cell.events.clone(), b"tcPr")?
        .ok_or_else(|| validation("DOCX 单元格缺少 tcPr"))?;
    let mut output = events[cell.events.start..properties.end].to_vec();
    output.extend(body);
    output.push(Event::End(BytesEnd::new("w:tc")));
    Ok(output)
}

fn text_paragraphs(value: &str, keep_next: bool) -> Result<Vec<Event<'static>>, HostError> {
    let mut output = Vec::new();
    output.push(Event::Start(BytesStart::new("w:p")));
    output.push(Event::Start(BytesStart::new("w:pPr")));
    if keep_next {
        output.push(Event::Empty(BytesStart::new("w:keepNext")));
    }
    output.push(Event::End(BytesEnd::new("w:pPr")));
    for (index, line) in value.split('\n').enumerate() {
        output.push(Event::Start(BytesStart::new("w:r")));
        if index > 0 {
            output.push(Event::Empty(BytesStart::new("w:br")));
        }
        output.extend(text_element(line));
        output.push(Event::End(BytesEnd::new("w:r")));
    }
    output.push(Event::End(BytesEnd::new("w:p")));
    Ok(output)
}

fn text_paragraph(
    prefix: &str,
    value: &str,
    keep_next: bool,
) -> Result<Vec<Event<'static>>, HostError> {
    let mut output = Vec::new();
    output.push(Event::Start(BytesStart::new("w:p")));
    output.push(Event::Start(BytesStart::new("w:pPr")));
    if keep_next {
        output.push(Event::Empty(BytesStart::new("w:keepNext")));
    }
    output.push(Event::End(BytesEnd::new("w:pPr")));
    output.push(Event::Start(BytesStart::new("w:r")));
    output.extend(text_element(&format!("{prefix}{value}")));
    output.push(Event::End(BytesEnd::new("w:r")));
    output.push(Event::End(BytesEnd::new("w:p")));
    Ok(output)
}

fn image_paragraph(
    plan: &ImagePlan,
    screenshot: &VideoScreenshot,
    keep_next: bool,
) -> Result<Vec<Event<'static>>, HostError> {
    let (cx, cy) = fit_image(screenshot.width_px, screenshot.height_px);
    let mut output = Vec::new();
    output.extend([
        Event::Start(BytesStart::new("w:p")),
        Event::Start(BytesStart::new("w:pPr")),
    ]);
    if keep_next {
        output.push(Event::Empty(BytesStart::new("w:keepNext")));
    }
    output.extend([
        Event::End(BytesEnd::new("w:pPr")),
        Event::Start(BytesStart::new("w:r")),
        Event::Start(BytesStart::new("w:drawing")),
    ]);
    let mut inline = BytesStart::new("wp:inline");
    inline.push_attribute(("distT", "0"));
    inline.push_attribute(("distB", "0"));
    inline.push_attribute(("distL", "0"));
    inline.push_attribute(("distR", "0"));
    output.push(Event::Start(inline));
    let cx_value = cx.to_string();
    let cy_value = cy.to_string();
    let mut extent = BytesStart::new("wp:extent");
    extent.push_attribute(("cx", cx_value.as_str()));
    extent.push_attribute(("cy", cy_value.as_str()));
    output.push(Event::Empty(extent));
    let mut effect_extent = BytesStart::new("wp:effectExtent");
    effect_extent.push_attribute(("l", "0"));
    effect_extent.push_attribute(("t", "0"));
    effect_extent.push_attribute(("r", "0"));
    effect_extent.push_attribute(("b", "0"));
    output.push(Event::Empty(effect_extent));
    let mut doc_pr = BytesStart::new("wp:docPr");
    let drawing_id = plan.drawing_id.to_string();
    let picture_name = format!("视频截图 {}", plan.drawing_id);
    doc_pr.push_attribute(("id", drawing_id.as_str()));
    doc_pr.push_attribute(("name", picture_name.as_str()));
    output.extend([
        Event::Empty(doc_pr),
        Event::Start(BytesStart::new("a:graphic")),
    ]);
    let mut graphic_data = BytesStart::new("a:graphicData");
    graphic_data.push_attribute((
        "uri",
        "http://schemas.openxmlformats.org/drawingml/2006/picture",
    ));
    output.push(Event::Start(graphic_data));
    output.extend([
        Event::Start(BytesStart::new("pic:pic")),
        Event::Start(BytesStart::new("pic:nvPicPr")),
    ]);
    let mut non_visual_properties = BytesStart::new("pic:cNvPr");
    non_visual_properties.push_attribute(("id", drawing_id.as_str()));
    non_visual_properties.push_attribute(("name", picture_name.as_str()));
    output.extend([
        Event::Empty(non_visual_properties),
        Event::Empty(BytesStart::new("pic:cNvPicPr")),
        Event::End(BytesEnd::new("pic:nvPicPr")),
        Event::Start(BytesStart::new("pic:blipFill")),
    ]);
    let mut blip = BytesStart::new("a:blip");
    blip.push_attribute(("r:embed", plan.rel_id.as_str()));
    output.push(Event::Empty(blip));
    output.extend([
        Event::Start(BytesStart::new("a:stretch")),
        Event::Empty(BytesStart::new("a:fillRect")),
        Event::End(BytesEnd::new("a:stretch")),
        Event::End(BytesEnd::new("pic:blipFill")),
        Event::Start(BytesStart::new("pic:spPr")),
        Event::Start(BytesStart::new("a:xfrm")),
    ]);
    let mut offset = BytesStart::new("a:off");
    offset.push_attribute(("x", "0"));
    offset.push_attribute(("y", "0"));
    output.push(Event::Empty(offset));
    let mut shape_extent = BytesStart::new("a:ext");
    shape_extent.push_attribute(("cx", cx_value.as_str()));
    shape_extent.push_attribute(("cy", cy_value.as_str()));
    output.extend([
        Event::Empty(shape_extent),
        Event::End(BytesEnd::new("a:xfrm")),
    ]);
    let mut preset_geometry = BytesStart::new("a:prstGeom");
    preset_geometry.push_attribute(("prst", "rect"));
    output.extend([
        Event::Start(preset_geometry),
        Event::Empty(BytesStart::new("a:avLst")),
        Event::End(BytesEnd::new("a:prstGeom")),
        Event::End(BytesEnd::new("pic:spPr")),
        Event::End(BytesEnd::new("pic:pic")),
        Event::End(BytesEnd::new("a:graphicData")),
        Event::End(BytesEnd::new("a:graphic")),
        Event::End(BytesEnd::new("wp:inline")),
        Event::End(BytesEnd::new("w:drawing")),
        Event::End(BytesEnd::new("w:r")),
        Event::End(BytesEnd::new("w:p")),
    ]);
    Ok(output)
}

fn fit_image(width: u32, height: u32) -> (u64, u64) {
    const EMU_PER_PIXEL_AT_96_DPI: u64 = 9_525;
    const MAX_CX: u64 = 5_800_000;
    const MAX_CY: u64 = 2_000_000;
    let width = width as u64;
    let height = height as u64;
    let divisor = greatest_common_divisor(width, height);
    let ratio_width = width / divisor;
    let ratio_height = height / divisor;
    let native_multiplier = divisor.saturating_mul(EMU_PER_PIXEL_AT_96_DPI);
    let multiplier = native_multiplier
        .min(MAX_CX / ratio_width)
        .min(MAX_CY / ratio_height)
        .max(1);
    (ratio_width * multiplier, ratio_height * multiplier)
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn format_asset_reference(reference: &VideoAssetReference) -> String {
    format!(
        "{} | {} | SHA-256 {}",
        reference.asset_id, reference.file_name, reference.sha256
    )
}

fn add_picture_namespaces(events: &mut [Event<'static>]) -> Result<(), HostError> {
    let root_index = events
        .iter()
        .position(|event| matches!(event, Event::Start(start) if local_name(start) == b"document"))
        .ok_or_else(|| validation("DOCX 缺少 document 根节点"))?;
    let start = match &events[root_index] {
        Event::Start(start) => start,
        _ => return Err(validation("DOCX document 根节点无效")),
    };
    let mut root = start.to_owned();
    if !has_attribute(&root, b"xmlns:a")? {
        root.push_attribute((
            "xmlns:a",
            "http://schemas.openxmlformats.org/drawingml/2006/main",
        ));
    }
    if !has_attribute(&root, b"xmlns:pic")? {
        root.push_attribute((
            "xmlns:pic",
            "http://schemas.openxmlformats.org/drawingml/2006/picture",
        ));
    }
    events[root_index] = Event::Start(root);
    Ok(())
}

fn replace_contract_paragraph(
    events: &mut Vec<Event<'static>>,
    title: &str,
) -> Result<(), HostError> {
    let paragraphs = collect_element_ranges(events, 0..events.len(), b"p")?;
    for paragraph in paragraphs {
        if text_in_range(events, paragraph.clone())?.contains("合同名称：") {
            let replacement =
                replace_paragraph_text(events, paragraph.clone(), &format!("合同名称：{title}"))?;
            events.splice(paragraph, replacement);
            return Ok(());
        }
    }
    Err(validation("DOCX 缺少合同名称语义段落"))
}

fn replace_paragraph_text(
    events: &[Event<'static>],
    paragraph: Range<usize>,
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    let properties = first_element_range(events, paragraph.clone(), b"pPr")?;
    let body_start = properties
        .as_ref()
        .map_or(paragraph.start + 1, |range| range.end);
    let mut output = events[paragraph.start..body_start].to_vec();
    output.extend(text_paragraph_body(value));
    output.push(Event::End(BytesEnd::new("w:p")));
    Ok(output)
}

fn text_paragraph_body(value: &str) -> Vec<Event<'static>> {
    let mut output = vec![Event::Start(BytesStart::new("w:r"))];
    output.extend(text_element(value));
    output.push(Event::End(BytesEnd::new("w:r")));
    output
}

fn text_element(value: &str) -> Vec<Event<'static>> {
    let mut element = BytesStart::new("w:t");
    element.push_attribute(("xml:space", "preserve"));
    vec![
        Event::Start(element),
        Event::Text(BytesText::new(value).into_owned()),
        Event::End(BytesEnd::new("w:t")),
    ]
}

fn add_row_properties(
    events: &[Event<'static>],
    properties: &[&str],
) -> Result<Vec<Event<'static>>, HostError> {
    if !matches!(events.first(), Some(Event::Start(start)) if local_name(start) == b"tr") {
        return Err(validation("DOCX 行结构无效"));
    }
    let mut output = events.to_vec();
    let insert_at = output
        .iter()
        .position(|event| matches!(event, Event::Start(start) if local_name(start) == b"trPr"))
        .map(|index| {
            let ranges =
                collect_element_ranges(&output, 0..output.len(), b"trPr").unwrap_or_default();
            ranges.first().map_or(index + 1, |range| range.end - 1)
        });
    let missing = properties
        .iter()
        .filter(|property| !contains_local_element(&output, property.as_bytes()))
        .map(|property| Event::Empty(BytesStart::new((*property).to_owned())))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(output);
    }
    if let Some(index) = insert_at {
        output.splice(index..index, missing);
    } else {
        output.splice(
            1..1,
            std::iter::once(Event::Start(BytesStart::new("w:trPr")))
                .chain(missing)
                .chain(std::iter::once(Event::End(BytesEnd::new("w:trPr")))),
        );
    }
    Ok(output)
}

fn add_keep_next(events: &[Event<'static>]) -> Result<Vec<Event<'static>>, HostError> {
    let paragraphs = collect_element_ranges(events, 0..events.len(), b"p")?;
    if paragraphs.is_empty() {
        return Ok(events.to_vec());
    }
    let mut output = events.to_vec();
    for paragraph in paragraphs.into_iter().rev() {
        let ppr = first_element_range(&output, paragraph.clone(), b"pPr")?;
        if let Some(range) = ppr {
            output.splice(
                range.end - 1..range.end - 1,
                [Event::Empty(BytesStart::new("w:keepNext"))],
            );
        } else {
            output.splice(
                paragraph.start + 1..paragraph.start + 1,
                [
                    Event::Start(BytesStart::new("w:pPr")),
                    Event::Empty(BytesStart::new("w:keepNext")),
                    Event::End(BytesEnd::new("w:pPr")),
                ],
            );
        }
    }
    Ok(output)
}

fn add_page_break_before(events: &[Event<'static>]) -> Result<Vec<Event<'static>>, HostError> {
    let paragraphs = collect_element_ranges(events, 0..events.len(), b"p")?;
    let Some(paragraph) = paragraphs.first() else {
        return Err(validation("验收结论行缺少段落"));
    };
    let mut output = events.to_vec();
    if let Some(ppr) = first_element_range(&output, paragraph.clone(), b"pPr")? {
        output.splice(
            ppr.end - 1..ppr.end - 1,
            [Event::Empty(BytesStart::new("w:pageBreakBefore"))],
        );
    } else {
        output.splice(
            paragraph.start + 1..paragraph.start + 1,
            [
                Event::Start(BytesStart::new("w:pPr")),
                Event::Empty(BytesStart::new("w:pageBreakBefore")),
                Event::End(BytesEnd::new("w:pPr")),
            ],
        );
    }
    Ok(output)
}

fn add_image_relationships(package: &mut Package, plans: &[ImagePlan]) -> Result<(), HostError> {
    let xml = package_entry(package, DOCUMENT_RELS_PATH)?.to_vec();
    let mut events = parse_xml_events(&xml, DOCUMENT_RELS_PATH)?;
    let end = events
        .iter()
        .rposition(|event| matches!(event, Event::End(end) if end.local_name().as_ref() == b"Relationships"))
        .ok_or_else(|| validation("DOCX document relationships 缺少根节点"))?;
    let mut additions = Vec::new();
    for plan in plans {
        let mut relationship = BytesStart::new("Relationship");
        relationship.push_attribute(("Id", plan.rel_id.as_str()));
        relationship.push_attribute((
            "Type",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
        ));
        relationship.push_attribute((
            "Target",
            plan.path
                .strip_prefix("word/")
                .unwrap_or(plan.path.as_str()),
        ));
        additions.push(Event::Empty(relationship));
    }
    events.splice(end..end, additions);
    replace_package_entry(package, DOCUMENT_RELS_PATH, write_xml_events(events)?)
}

fn add_image_content_types(package: &mut Package, plans: &[ImagePlan]) -> Result<(), HostError> {
    if plans.is_empty() {
        return Ok(());
    }
    let xml = package_entry(package, CONTENT_TYPES_PATH)?.to_vec();
    let mut events = parse_xml_events(&xml, CONTENT_TYPES_PATH)?;
    let mut types = HashSet::new();
    for event in &events {
        if let Event::Start(start) | Event::Empty(start) = event {
            if local_name(start) == b"Default" {
                if let Some(extension) = attribute_value(start, b"Extension")? {
                    types.insert(extension.to_ascii_lowercase());
                }
            }
        }
    }
    let end = events
        .iter()
        .rposition(
            |event| matches!(event, Event::End(end) if end.local_name().as_ref() == b"Types"),
        )
        .ok_or_else(|| validation("DOCX content types 缺少根节点"))?;
    let mut additions = Vec::new();
    for plan in plans {
        let extension = plan
            .path
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if types.insert(extension.clone()) {
            let mut default = BytesStart::new("Default");
            default.push_attribute(("Extension", extension.as_str()));
            default.push_attribute(("ContentType", plan.content_type.as_str()));
            additions.push(Event::Empty(default));
        }
    }
    events.splice(end..end, additions);
    replace_package_entry(package, CONTENT_TYPES_PATH, write_xml_events(events)?)
}

fn verify_rendered_document(
    output: &[u8],
    data: &VideoCompletionAcceptanceTemplateData,
    image_plans: &[ImagePlan],
) -> Result<(), HostError> {
    let package = load_package_from_bytes(output)?;
    validate_safe_docx(&package)?;
    let events = parse_xml_events(package_entry(&package, DOCUMENT_PATH)?, DOCUMENT_PATH)?;
    let table = locate_table(&events)?;
    let video_count = data
        .delivery_groups
        .iter()
        .map(|group| group.videos.len())
        .sum::<usize>();
    let expected_rows = 1 + data.delivery_groups.len() + video_count * 2 + 2;
    if table.rows.len() != expected_rows {
        return Err(validation("视频成片验收输出动态行数量校验失败"));
    }
    let first_row = row_text(&events, &table.rows[0])?;
    if !first_row.contains(&data.project_title) || !first_row.contains(&data.completion_date) {
        return Err(validation("视频成片验收输出项目或完成时间校验失败"));
    }
    let conclusion = row_text(&events, &table.rows[table.rows.len() - 2])?;
    if !conclusion.contains(&data.acceptance_conclusion) {
        return Err(validation("视频成片验收输出结论校验失败"));
    }
    let signoff = row_text(&events, &table.rows[table.rows.len() - 1])?;
    if signoff.chars().any(|character| character.is_ascii_digit()) {
        return Err(validation("视频成片验收输出签章区残留日期或历史值"));
    }
    for row in &table.rows[1..table.rows.len() - 2] {
        if !row_has_property(&events, row, b"cantSplit")? {
            return Err(validation("视频成片验收输出动态行未设置不可拆分页"));
        }
    }
    let mut row_index = 1_usize;
    for group in &data.delivery_groups {
        if !row_has_paragraph_property(&events, &table.rows[row_index], b"keepNext")? {
            return Err(validation("视频成片验收输出交付组标题未与首个视频保持同页"));
        }
        row_index += 1;
        for _ in &group.videos {
            if !row_has_paragraph_property(&events, &table.rows[row_index], b"keepNext")? {
                return Err(validation("视频成片验收输出视频标题未与内容保持同页"));
            }
            row_index += 2;
        }
    }
    if !row_has_paragraph_property(&events, &table.rows[table.rows.len() - 2], b"keepNext")? {
        return Err(validation("视频成片验收输出结论未与签章区保持同页"));
    }
    let text = package_text(package_entry(&package, DOCUMENT_PATH)?)?;
    let document_events = parse_xml_events(package_entry(&package, DOCUMENT_PATH)?, DOCUMENT_PATH)?;
    let relationship_events = parse_xml_events(
        package_entry(&package, DOCUMENT_RELS_PATH)?,
        DOCUMENT_RELS_PATH,
    )?;
    for stale in [
        "示例客户项目",
        "合成品牌片 A、合成品牌片 B",
        "32s/37s",
        "{{",
        "}}",
    ] {
        if text.contains(stale) {
            return Err(validation(format!(
                "视频成片验收输出残留模板示例或占位符: {stale}"
            )));
        }
    }
    for plan in image_plans {
        if !package.index_by_name.contains_key(&plan.path) {
            return Err(validation(format!(
                "视频成片验收输出缺少截图条目: {}",
                plan.path
            )));
        }
        let referenced = document_events.iter().any(|event| match event {
            Event::Start(start) | Event::Empty(start) => attribute_value(start, b"embed")
                .ok()
                .flatten()
                .is_some_and(|value| value == plan.rel_id),
            _ => false,
        });
        if !referenced {
            return Err(validation(format!(
                "视频成片验收输出缺少截图关系: {}",
                plan.rel_id
            )));
        }
        let expected_target = plan
            .path
            .strip_prefix("word/")
            .unwrap_or(plan.path.as_str());
        let relationship_valid = relationship_events.iter().any(|event| {
            let (Event::Start(start) | Event::Empty(start)) = event else {
                return false;
            };
            if local_name(start) != b"Relationship" {
                return false;
            }
            let id = attribute_value(start, b"Id").ok().flatten();
            let relationship_type = attribute_value(start, b"Type").ok().flatten();
            let target = attribute_value(start, b"Target").ok().flatten();
            id.as_deref() == Some(plan.rel_id.as_str())
                && relationship_type.as_deref()
                    == Some(
                        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
                    )
                && target.as_deref() == Some(expected_target)
        });
        if !relationship_valid {
            return Err(validation(format!(
                "视频成片验收输出截图关系目标或类型无效: {}",
                plan.rel_id
            )));
        }
    }
    verify_no_sensitive_metadata(&package)
}

fn verify_no_sensitive_metadata(package: &Package) -> Result<(), HostError> {
    if let Some(index) = package.index_by_name.get(CORE_PROPERTIES_PATH) {
        let text = package_text(&package.entries[*index].contents)?;
        if text.contains("<dc:creator") || text.contains("lastModifiedBy") {
            return Err(validation("视频成片验收输出仍包含作者元数据"));
        }
    }
    if let Some(index) = package.index_by_name.get(SETTINGS_PATH) {
        if package_text(&package.entries[*index].contents)?.contains("attachedTemplate") {
            return Err(validation("视频成片验收输出仍包含 attachedTemplate"));
        }
    }
    Ok(())
}

fn sanitize_package(package: &mut Package) -> Result<(), HostError> {
    if package.index_by_name.contains_key(CORE_PROPERTIES_PATH) {
        let xml = package_entry(package, CORE_PROPERTIES_PATH)?.to_vec();
        replace_package_entry(
            package,
            CORE_PROPERTIES_PATH,
            remove_named_elements(&xml, &[b"creator", b"lastModifiedBy"])?,
        )?;
    }
    if package.index_by_name.contains_key(SETTINGS_PATH) {
        let xml = package_entry(package, SETTINGS_PATH)?.to_vec();
        replace_package_entry(
            package,
            SETTINGS_PATH,
            remove_named_elements(&xml, &[b"attachedTemplate"])?,
        )?;
    }
    Ok(())
}

fn validate_paths(source: &Path, destination: &Path) -> Result<(), HostError> {
    let source = fs::canonicalize(source)
        .map_err(|error| internal(format!("读取视频成片验收模板失败: {error}")))?;
    if !source.is_file() {
        return Err(validation("视频成片验收模板不是文件"));
    }
    let destination_parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = fs::canonicalize(destination_parent)
        .map_err(|error| internal(format!("读取输出目录失败: {error}")))?;
    let name = destination
        .file_name()
        .ok_or_else(|| validation("视频成片验收输出文件名无效"))?;
    let destination = destination_parent.join(name);
    if paths_equal(&source, &destination) {
        return Err(validation("视频成片验收模板和输出路径不能相同"));
    }
    validate_destination(&destination)
}

fn validate_destination(destination: &Path) -> Result<(), HostError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent =
        fs::canonicalize(parent).map_err(|error| internal(format!("读取输出目录失败: {error}")))?;
    let absolute = parent.join(
        destination
            .file_name()
            .ok_or_else(|| validation("视频成片验收输出文件名无效"))?,
    );
    if absolute.exists() {
        return Err(validation("视频成片验收输出文件已存在"));
    }
    if absolute
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("docx"))
    {
        return Err(validation("视频成片验收输出必须使用 .docx 扩展名"));
    }
    Ok(())
}

fn publish_no_replace(destination: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .ok_or_else(|| validation("视频成片验收输出文件名无效"))?
        .to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| internal(format!("读取系统时间失败: {error}")))?
        .as_nanos();
    let mut staged = None;
    for _ in 0..32 {
        let counter = STAGED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{name}.{}.{}.{}.tmp",
            std::process::id(),
            stamp,
            counter
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|error| internal(format!("写入临时 DOCX 失败: {error}")))?;
                file.sync_all()
                    .map_err(|error| internal(format!("同步临时 DOCX 失败: {error}")))?;
                staged = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(internal(format!("创建临时 DOCX 失败: {error}"))),
        }
    }
    let staged = staged.ok_or_else(|| internal("无法分配视频成片验收临时输出"))?;
    let result = fs::hard_link(&staged, destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            validation("视频成片验收输出文件已存在")
        } else {
            internal(format!("原子发布视频成片验收 DOCX 失败: {error}"))
        }
    });
    let cleanup = fs::remove_file(&staged);
    result.and(cleanup.map_err(|error| internal(format!("清理临时 DOCX 失败: {error}"))))
}

fn load_package_from_bytes(source: &[u8]) -> Result<Package, HostError> {
    let mut archive = ZipArchive::new(Cursor::new(source))
        .map_err(|error| validation(format!("DOCX 不是有效 ZIP: {error}")))?;
    if archive.is_empty() || archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(validation("DOCX 条目数量超出限制"));
    }
    let mut entries = Vec::with_capacity(archive.len());
    let mut index_by_name = HashMap::with_capacity(archive.len());
    let mut names = HashSet::with_capacity(archive.len());
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| validation(format!("读取 DOCX 条目失败: {error}")))?;
        let name = entry.name().to_owned();
        validate_zip_name(&name)?;
        if entry.encrypted() || entry.is_symlink() {
            return Err(validation(format!("DOCX 条目不允许加密或符号链接: {name}")));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(validation(format!("DOCX 条目使用不支持的压缩方法: {name}")));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(validation(format!("DOCX 条目过大: {name}")));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| validation("DOCX 解压大小溢出"))?;
        if total > MAX_PACKAGE_BYTES {
            return Err(validation("DOCX 解压后超过大小限制"));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(validation(format!("DOCX 包含重复条目: {name}")));
        }
        let options = entry.options();
        let is_dir = entry.is_dir();
        let mut contents = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut contents)
            .map_err(|error| validation(format!("解压 DOCX 条目失败: {error}")))?;
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

fn validate_safe_docx(package: &Package) -> Result<(), HostError> {
    for required in [CONTENT_TYPES_PATH, ROOT_RELS_PATH, DOCUMENT_PATH] {
        if !package.index_by_name.contains_key(required) {
            return Err(validation(format!("DOCX 缺少标准条目: {required}")));
        }
    }
    for entry in &package.entries {
        let lower = entry.name.to_ascii_lowercase();
        if lower.ends_with("vbaproject.bin")
            || lower.contains("/macros/")
            || lower.contains("/activex/")
            || lower.contains("/embeddings/")
        {
            return Err(validation(format!("DOCX 包含宏或活动内容: {}", entry.name)));
        }
        if entry.name.ends_with(".rels") {
            validate_relationships(&entry.contents)?;
        }
        if entry.name.starts_with("word/") && entry.name.ends_with(".xml") {
            validate_word_xml(&entry.contents, &entry.name)?;
        }
    }
    validate_content_types(package_entry(package, CONTENT_TYPES_PATH)?)
}

fn validate_content_types(xml: &[u8]) -> Result<(), HostError> {
    let events = parse_xml_events(xml, CONTENT_TYPES_PATH)?;
    let mut document = false;
    for event in &events {
        let start = match event {
            Event::Start(start) | Event::Empty(start) => start,
            _ => continue,
        };
        if !matches!(local_name(start), b"Default" | b"Override") {
            continue;
        }
        let content_type = attribute_value(start, b"ContentType")?
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.contains("macroenabled")
            || content_type.contains("vbaproject")
            || content_type.contains("activex")
            || content_type.contains("oleobject")
        {
            return Err(validation("DOCX 内容类型包含宏或活动内容"));
        }
        if local_name(start) == b"Override"
            && attribute_value(start, b"PartName")?.as_deref() == Some("/word/document.xml")
        {
            document = content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
        }
    }
    if !document {
        return Err(validation("DOCX 未声明标准 word/document.xml 内容类型"));
    }
    Ok(())
}

fn validate_relationships(xml: &[u8]) -> Result<(), HostError> {
    for event in parse_xml_events(xml, "DOCX relationships")? {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(&start) == b"Relationship" => {
                start
            }
            _ => continue,
        };
        let relation_type = attribute_value(&start, b"Type")?
            .unwrap_or_default()
            .to_ascii_lowercase();
        let target = attribute_value(&start, b"Target")?.unwrap_or_default();
        let mode = attribute_value(&start, b"TargetMode")?.unwrap_or_default();
        if mode.eq_ignore_ascii_case("external")
            || relation_type.contains("vbaproject")
            || relation_type.contains("macro")
            || relation_type.ends_with("/oleobject")
            || relation_type.ends_with("/package")
            || relationship_target_is_absolute(&target)
        {
            return Err(validation(
                "DOCX relationships 包含外部、宏、OLE 或绝对路径关系",
            ));
        }
    }
    Ok(())
}

fn validate_word_xml(xml: &[u8], label: &str) -> Result<(), HostError> {
    let events = parse_xml_events(xml, label)?;
    let mut instruction = false;
    let mut instruction_text = String::new();
    for event in events {
        match event {
            Event::Start(start) | Event::Empty(start) if local_name(&start) == b"object" => {
                return Err(validation(format!("DOCX 包含 object: {label}")))
            }
            Event::Start(start) if local_name(&start) == b"instrText" => instruction = true,
            Event::Text(text) if instruction => instruction_text.push_str(
                &unescape(
                    std::str::from_utf8(text.as_ref())
                        .map_err(|_| validation("DOCX 指令文本无效"))?,
                )
                .map_err(|_| validation("DOCX 指令文本转义无效"))?,
            ),
            Event::End(end) if end.local_name().as_ref() == b"instrText" => instruction = false,
            _ => {}
        }
    }
    if instruction_text
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("dde") || token.eq_ignore_ascii_case("ddeauto"))
    {
        return Err(validation(format!("DOCX 包含 DDE 指令: {label}")));
    }
    Ok(())
}

fn write_package(package: &Package) -> Result<Vec<u8>, HostError> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        for entry in &package.entries {
            if entry.is_dir {
                writer
                    .add_directory(&entry.name, entry.options)
                    .map_err(|error| internal(format!("写入 DOCX 目录失败: {error}")))?;
            } else {
                writer
                    .start_file(&entry.name, entry.options)
                    .map_err(|error| internal(format!("写入 DOCX 条目失败: {error}")))?;
                writer
                    .write_all(&entry.contents)
                    .map_err(|error| internal(format!("写入 DOCX 条目失败: {error}")))?;
            }
        }
        writer
            .finish()
            .map_err(|error| internal(format!("完成 DOCX 包失败: {error}")))?;
    }
    Ok(output.into_inner())
}

fn package_entry<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], HostError> {
    let index = package
        .index_by_name
        .get(name)
        .copied()
        .ok_or_else(|| validation(format!("DOCX 缺少条目: {name}")))?;
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
        .ok_or_else(|| validation(format!("DOCX 缺少条目: {name}")))?;
    package.entries[index].contents = contents;
    Ok(())
}

fn remove_named_elements(xml: &[u8], names: &[&[u8]]) -> Result<Vec<u8>, HostError> {
    let events = parse_xml_events(xml, "DOCX XML")?;
    let mut output = Vec::new();
    let mut skip = 0_usize;
    for event in events {
        match &event {
            Event::Start(start) if names.iter().any(|name| local_name(start) == *name) => skip += 1,
            Event::End(end) if skip > 0 => {
                skip -= 1;
                continue;
            }
            Event::Empty(start) if names.iter().any(|name| local_name(start) == *name) => continue,
            _ if skip > 0 => continue,
            _ => {}
        }
        if skip == 0 {
            output.push(event);
        }
    }
    if skip != 0 {
        return Err(validation("DOCX XML 清理时元素未闭合"));
    }
    write_xml_events(output)
}

fn locate_table(events: &[Event<'static>]) -> Result<TableRange, HostError> {
    let tables = collect_element_ranges(events, 0..events.len(), b"tbl")?;
    if tables.len() != 1 {
        return Err(validation("视频成片验收 DOCX 必须只有一个主表格"));
    }
    let table_range = tables[0].clone();
    let rows = collect_element_ranges(events, table_range.clone(), b"tr")?
        .into_iter()
        .map(|range| {
            let cells = collect_element_ranges(events, range.clone(), b"tc")?
                .into_iter()
                .map(|cell| {
                    let span_range = if let Some(properties) =
                        first_element_range(events, cell.clone(), b"tcPr")?
                    {
                        first_element_range(events, properties, b"gridSpan")?
                    } else {
                        None
                    };
                    let span = span_range.map_or(1, |span| {
                        attribute_value(
                            match &events[span.start] {
                                Event::Empty(start) | Event::Start(start) => start,
                                _ => unreachable!(),
                            },
                            b"val",
                        )
                        .ok()
                        .flatten()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1)
                    });
                    Ok(CellRange {
                        events: cell,
                        logical_columns: span,
                    })
                })
                .collect::<Result<Vec<_>, HostError>>()?;
            Ok(RowRange {
                events: range,
                cells,
            })
        })
        .collect::<Result<Vec<_>, HostError>>()?;
    let grid_columns = collect_element_ranges(events, table_range.clone(), b"gridCol")?.len();
    Ok(TableRange {
        events: table_range,
        rows,
        grid_columns,
    })
}

fn collect_element_ranges(
    events: &[Event<'static>],
    scope: Range<usize>,
    name: &[u8],
) -> Result<Vec<Range<usize>>, HostError> {
    let mut stack = Vec::<(Vec<u8>, usize)>::new();
    let mut output = Vec::new();
    for index in scope.clone() {
        match &events[index] {
            Event::Start(start) => stack.push((local_name(start).to_vec(), index)),
            Event::Empty(start) if local_name(start) == name => output.push(index..index + 1),
            Event::End(end) => {
                let (element, start) = stack
                    .pop()
                    .ok_or_else(|| validation("DOCX XML 元素未闭合"))?;
                if element != end.local_name().as_ref() {
                    return Err(validation("DOCX XML 元素嵌套无效"));
                }
                if element == name {
                    output.push(start..index + 1);
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(validation("DOCX XML 元素未闭合"));
    }
    output.sort_by_key(|range| range.start);
    Ok(output)
}

fn first_element_range(
    events: &[Event<'static>],
    scope: Range<usize>,
    name: &[u8],
) -> Result<Option<Range<usize>>, HostError> {
    Ok(collect_element_ranges(events, scope, name)?
        .into_iter()
        .next())
}

fn row_text(events: &[Event<'static>], row: &RowRange) -> Result<String, HostError> {
    text_in_range(events, row.events.clone())
}

fn text_in_range(events: &[Event<'static>], range: Range<usize>) -> Result<String, HostError> {
    let mut output = String::new();
    let mut text_depth = 0_usize;
    for event in &events[range] {
        match event {
            Event::Start(start) if local_name(start) == b"t" => text_depth += 1,
            Event::End(end) if end.local_name().as_ref() == b"t" => {
                text_depth = text_depth.saturating_sub(1);
            }
            Event::Text(text) if text_depth > 0 => {
                output.push_str(
                    &unescape(
                        std::str::from_utf8(text.as_ref())
                            .map_err(|_| validation("DOCX 文本不是 UTF-8"))?,
                    )
                    .map_err(|_| validation("DOCX 文本转义无效"))?,
                );
            }
            _ => {}
        }
    }
    Ok(output)
}

fn row_has_property(
    events: &[Event<'static>],
    row: &RowRange,
    property: &[u8],
) -> Result<bool, HostError> {
    let Some(properties) = first_element_range(events, row.events.clone(), b"trPr")? else {
        return Ok(false);
    };
    Ok(contains_local_element(&events[properties], property))
}

fn row_has_paragraph_property(
    events: &[Event<'static>],
    row: &RowRange,
    property: &[u8],
) -> Result<bool, HostError> {
    let paragraphs = collect_element_ranges(events, row.events.clone(), b"p")?;
    if paragraphs.is_empty() {
        return Ok(false);
    }
    for paragraph in paragraphs {
        let Some(properties) = first_element_range(events, paragraph, b"pPr")? else {
            return Ok(false);
        };
        if !contains_local_element(&events[properties], property) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn contains_local_element(events: &[Event<'static>], name: &[u8]) -> bool {
    events.iter().any(|event| match event {
        Event::Start(start) | Event::Empty(start) => local_name(start) == name,
        _ => false,
    })
}

fn package_text(xml: &[u8]) -> Result<String, HostError> {
    let events = parse_xml_events(xml, "DOCX XML")?;
    text_in_range(&events, 0..events.len())
}

fn parse_xml_events(xml: &[u8], label: &str) -> Result<Vec<Event<'static>>, HostError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().check_end_names = true;
    let mut buffer = Vec::new();
    let mut events = Vec::new();
    let mut depth = 0_usize;
    let mut roots = 0_usize;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Eof) if depth == 0 && roots == 1 => break,
            Ok(Event::Eof) => return Err(validation(format!("{label} XML 结构无效"))),
            Ok(Event::DocType(_)) => return Err(validation(format!("{label} 不允许 DTD"))),
            Ok(Event::Start(start)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
                events.push(Event::Start(start.into_owned()));
            }
            Ok(Event::Empty(start)) => {
                if depth == 0 {
                    roots += 1;
                }
                events.push(Event::Empty(start.into_owned()));
            }
            Ok(Event::End(end)) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| validation(format!("{label} XML 结构无效")))?;
                events.push(Event::End(end.into_owned()));
            }
            Ok(event) => events.push(event.into_owned()),
            Err(error) => return Err(validation(format!("{label} XML 格式无效: {error}"))),
        }
        if roots > 1 {
            return Err(validation(format!("{label} XML 包含多个根节点")));
        }
        buffer.clear();
    }
    Ok(events)
}

fn write_xml_events(events: Vec<Event<'static>>) -> Result<Vec<u8>, HostError> {
    let mut writer = Writer::new(Vec::new());
    for event in events {
        writer
            .write_event(event)
            .map_err(|error| internal(format!("写入 DOCX XML 失败: {error}")))?;
    }
    Ok(writer.into_inner())
}

fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a [u8] {
    let qualified: &[u8] = start.as_ref();
    let end = qualified
        .iter()
        .position(|byte| byte.is_ascii_whitespace())
        .unwrap_or(qualified.len());
    let qualified = &qualified[..end];
    qualified
        .iter()
        .rposition(|byte| *byte == b':')
        .map_or(qualified, |colon| &qualified[colon + 1..])
}

fn attribute_value(start: &BytesStart<'_>, name: &[u8]) -> Result<Option<String>, HostError> {
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| validation("DOCX XML 属性无效"))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .unescape_value()
                .map(|value| Some(value.into_owned()))
                .map_err(|_| validation("DOCX XML 属性值无效"));
        }
    }
    Ok(None)
}

fn has_attribute(start: &BytesStart<'_>, name: &[u8]) -> Result<bool, HostError> {
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| validation("DOCX XML 属性无效"))?;
        if attribute.key.as_ref() == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_zip_name(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.starts_with('/')
        || name.starts_with("//")
        || (name.len() >= 2 && name.as_bytes()[1] == b':')
    {
        return Err(validation(format!("DOCX 包含不安全条目路径: {name}")));
    }
    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty()
        || trimmed
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(validation(format!("DOCX 包含路径逃逸条目: {name}")));
    }
    Ok(())
}

fn relationship_target_is_absolute(value: &str) -> bool {
    let value = value.trim();
    if value.starts_with('/')
        || value.starts_with('\\')
        || value.starts_with("//")
        || (value.len() >= 2 && value.as_bytes()[1] == b':')
    {
        return true;
    }
    value.find(':').is_some_and(|index| {
        index > 0
            && value[..index]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    })
}

fn read_file_limited(path: &Path, maximum: u64) -> Result<Vec<u8>, HostError> {
    let file = File::open(path).map_err(|error| internal(format!("读取文件失败: {error}")))?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| internal(format!("读取文件失败: {error}")))?;
    if bytes.len() as u64 > maximum {
        return Err(validation("文件超过大小限制"));
    }
    Ok(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn is_valid_xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || matches!(character as u32, 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
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

fn validation(message: impl Into<String>) -> HostError {
    HostError::validation(message)
}
fn internal(message: impl Into<String>) -> HostError {
    HostError::internal(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn external_qa_fixture(relative_path: &str) -> PathBuf {
        std::env::var_os("BSAIGC_EXTERNAL_QA_FIXTURE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/synthetic/business-v1"))
            .join(relative_path)
    }
    use tempfile::TempDir;

    fn valid_data() -> VideoCompletionAcceptanceTemplateData {
        let bytes = vec![
            0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
            1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 0, b'I', b'E', b'N', b'D',
            174, 66, 96, 130,
        ];
        let screenshot_sha = sha256_bytes(&bytes);
        VideoCompletionAcceptanceTemplateData {
            contract_title: "测试合同".to_owned(),
            project_title: "测试项目".to_owned(),
            completion_date: "2026-07-29".to_owned(),
            delivery_groups: vec![VideoDeliveryGroup {
                name: "交付组一".to_owned(),
                service_description: "完成视频成片及截图交付".to_owned(),
                videos: vec![VideoBlock {
                    title: "主片".to_owned(),
                    video_type: "横版成片".to_owned(),
                    content: "已按脚本完成剪辑、包装和审片".to_owned(),
                    duration: "30s".to_owned(),
                    asset_reference: VideoAssetReference {
                        asset_id: "asset-video-1".to_owned(),
                        file_name: "main.mp4".to_owned(),
                        sha256: "A".repeat(64),
                        external_link: None,
                    },
                    screenshots: vec![VideoScreenshot {
                        asset_id: "asset-shot-1".to_owned(),
                        sha256: screenshot_sha,
                        caption: "代表画面".to_owned(),
                        mime_type: "image/png".to_owned(),
                        image_bytes: bytes,
                        width_px: 1920,
                        height_px: 1080,
                    }],
                }],
            }],
            acceptance_conclusion: "本次视频成片已完成并通过验收".to_owned(),
            manually_confirmed: true,
        }
    }

    #[test]
    fn requires_manual_confirmation() {
        let mut data = valid_data();
        data.manually_confirmed = false;
        assert!(validate_data(&data).is_err());
    }

    #[test]
    fn rejects_screenshot_hash_drift() {
        let mut data = valid_data();
        data.delivery_groups[0].videos[0].screenshots[0].sha256 = "B".repeat(64);
        assert!(validate_data(&data).is_err());
    }

    #[test]
    fn preserves_aspect_ratio_with_bounded_dimensions() {
        let (width, height) = fit_image(1920, 1080);
        assert!(width <= 5_800_000);
        assert!(height <= 2_000_000);
        assert!(width >= 3_500_000);
        assert!(height >= 1_999_000);
        assert_eq!(width * 1080, height * 1920);
    }

    #[test]
    fn rejects_wrong_registered_template_hash() {
        assert!(validate_expected_template_hash(&"A".repeat(64)).is_err());
    }

    #[test]
    #[ignore = "reads the real external template without copying it into repo or target"]
    fn renders_real_external_template_read_only() {
        let source = external_qa_fixture("templates/synthetic-video-acceptance.docx");
        if !source.exists() {
            return;
        }
        let screenshot_paths = [
            external_qa_fixture("screenshots/synthetic-series-01-shot-01.png"),
            external_qa_fixture("screenshots/synthetic-series-01-shot-02.png"),
            external_qa_fixture("screenshots/synthetic-series-01-shot-03.png"),
            external_qa_fixture("screenshots/synthetic-series-02-shot-01.png"),
            external_qa_fixture("screenshots/synthetic-series-02-shot-02.png"),
            external_qa_fixture("screenshots/synthetic-series-02-shot-03.png"),
            external_qa_fixture("screenshots/synthetic-series-03-shot-01.png"),
            external_qa_fixture("screenshots/synthetic-series-03-shot-02.png"),
            external_qa_fixture("screenshots/synthetic-series-03-shot-03.png"),
        ];
        if screenshot_paths.iter().any(|path| !path.exists()) {
            return;
        }
        let screenshots = screenshot_paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let bytes = std::fs::read(path).unwrap();
                assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24);
                let width_px = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
                let height_px = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
                VideoScreenshot {
                    asset_id: format!("asset-shot-{}", index + 1),
                    sha256: sha256_bytes(&bytes),
                    caption: format!("代表截图 {}", index + 1),
                    mime_type: "image/png".to_owned(),
                    image_bytes: bytes,
                    width_px,
                    height_px,
                }
            })
            .collect::<Vec<_>>();
        let mut data = valid_data();
        data.delivery_groups = (0..3)
            .map(|group_index| VideoDeliveryGroup {
                name: format!("生活系列片 {}", group_index + 1),
                service_description: "视频策划、拍摄、剪辑与成片交付".to_owned(),
                videos: (0..2)
                    .map(|video_index| {
                        let screenshot_index = group_index * 3 + video_index;
                        VideoBlock {
                            title: format!("生活系列片 {}-{}", group_index + 1, video_index + 1),
                            video_type: "营销视频成片".to_owned(),
                            content: "已按确认脚本完成剪辑、包装和审片".to_owned(),
                            duration: if video_index == 0 { "32s" } else { "37s" }.to_owned(),
                            asset_reference: VideoAssetReference {
                                asset_id: format!(
                                    "asset-video-{}-{}",
                                    group_index + 1,
                                    video_index + 1
                                ),
                                file_name: format!(
                                    "生活系列片{}-{}.mp4",
                                    group_index + 1,
                                    video_index + 1
                                ),
                                sha256: format!("{:064X}", group_index * 2 + video_index + 1),
                                external_link: None,
                            },
                            screenshots: vec![
                                screenshots[screenshot_index].clone(),
                                screenshots[(screenshot_index + 1) % screenshots.len()].clone(),
                            ],
                        }
                    })
                    .collect(),
            })
            .collect();
        let temporary = TempDir::new().unwrap();
        let destination = std::env::var_os("BSAIGC_VIDEO_ACCEPTANCE_QA_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| temporary.path().join("video-acceptance.docx"));
        render_video_completion_acceptance_template(
            &source,
            VIDEO_COMPLETION_ACCEPTANCE_TEMPLATE_SHA256,
            &destination,
            &data,
        )
        .unwrap();
        assert!(destination.exists());
    }
}

use crate::protocol::HostError;
use quick_xml::{events::Event, Reader};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(crate) const PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256: &str =
    "7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF";

const DOCUMENT: &str = "word/document.xml";
const RELS: &str = "word/_rels/document.xml.rels";
const TYPES: &str = "[Content_Types].xml";
const MAX_SOURCE: u64 = 16 * 1024 * 1024;
const MAX_ENTRY: u64 = 16 * 1024 * 1024;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;
const MAX_IMAGES: usize = 256;
const MAX_IMAGE: usize = 12 * 1024 * 1024;
const STORYBOARD_COUNT: usize = 4;
const SHOT_COUNT: usize = 54;
const PAGE_WIDTH_DXA: u32 = 11_906;
const PAGE_LEFT_MARGIN_DXA: u32 = 1_134;
const PAGE_RIGHT_MARGIN_DXA: u32 = 1_134;
const PAGE_CONTENT_WIDTH_DXA: u32 = PAGE_WIDTH_DXA - PAGE_LEFT_MARGIN_DXA - PAGE_RIGHT_MARGIN_DXA;
const DELIVERY_TABLE_WIDTHS: [u32; 8] = [400, 700, 1_100, 900, 600, 700, 4_550, 450];
static STAGED_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductionResultConfirmationTemplateData {
    pub attachment_label: String,
    pub document_title: String,
    pub category: String,
    pub project_name: String,
    pub contract_title: String,
    pub payment_amount_cents: i64,
    pub contract_deliverable_summary: String,
    pub supplier_legal_name: String,
    pub procurement_period: String,
    pub acceptance_description: String,
    pub penalty_or_additions: String,
    pub delivery_items: Vec<ProductionResultConfirmationDeliveryItem>,
    pub execution_completed_date: String,
    pub acceptance_date: String,
    pub handler_signoff: String,
    pub professional_lead_signoff: String,
    pub other_department_signoff: String,
    pub supplier_handler_signoff: String,
    pub storyboards: Vec<ProductionResultConfirmationStoryboard>,
    pub clean_highlights: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductionResultConfirmationDeliveryItem {
    pub item_id: String,
    pub name: String,
    pub specification: String,
    pub required_quantity: String,
    pub unit: String,
    pub received_quantity: String,
    pub acceptance_note: String,
    pub images: Vec<ProductionResultConfirmationImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductionResultConfirmationStoryboard {
    pub title: String,
    pub specification: String,
    pub production_format: String,
    pub duration: String,
    pub shots: Vec<ProductionResultConfirmationShot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductionResultConfirmationShot {
    pub shot_number: String,
    pub scene: String,
    pub description: String,
    pub on_screen_copy: String,
    pub remarks: String,
    pub source_highlighted: bool,
    pub images: Vec<ProductionResultConfirmationImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductionResultConfirmationImage {
    pub asset_id: String,
    pub sha256: String,
    pub mime_type: String,
    pub width_px: u32,
    pub height_px: u32,
    pub alt_text: String,
    pub image_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ImagePlan {
    rel_id: String,
    path: String,
    extension: &'static str,
    content_type: &'static str,
    width: u32,
    height: u32,
    alt_text: String,
    bytes: Vec<u8>,
    drawing_id: u32,
}

#[derive(Debug)]
struct PackageEntry {
    name: String,
    is_dir: bool,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct Package {
    entries: Vec<PackageEntry>,
    by_name: HashMap<String, usize>,
}

pub(crate) fn render_production_result_confirmation_template(
    source: &Path,
    destination: &Path,
    data: &ProductionResultConfirmationTemplateData,
) -> Result<(), HostError> {
    let source = fs::canonicalize(source)
        .map_err(|error| internal(format!("读取制作成果确认模板失败: {error}")))?;
    let parent = fs::canonicalize(destination.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|error| internal(format!("读取输出目录失败: {error}")))?;
    let destination = parent.join(
        destination
            .file_name()
            .ok_or_else(|| validation("输出文件名无效"))?,
    );
    if paths_equal(&source, &destination) {
        return Err(validation("模板和输出路径不能相同"));
    }
    let bytes = read_limited(&source, MAX_SOURCE)?;
    render_production_result_confirmation_template_from_bytes(
        &bytes,
        PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256,
        &destination,
        data,
    )
}

pub(crate) fn render_production_result_confirmation_template_from_bytes(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &ProductionResultConfirmationTemplateData,
) -> Result<(), HostError> {
    if !expected_sha256.eq_ignore_ascii_case(PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256) {
        return Err(validation("必须使用登记的制作成果确认 v1 模板 SHA-256"));
    }
    validate_destination(destination)?;
    validate_data(data)?;
    validate_source(source, expected_sha256)?;
    render_package(load_package(source)?, destination, data)
}

fn validate_source(source: &[u8], expected: &str) -> Result<(), HostError> {
    if source.len() as u64 > MAX_SOURCE {
        return Err(validation("制作成果确认模板过大"));
    }
    let actual = sha256(source);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(validation(format!(
            "模板 SHA-256 不匹配，期望 {expected}，实际 {actual}"
        )));
    }
    let package = load_package(source)?;
    validate_safe_package(&package)?;
    let document = entry(&package, DOCUMENT)?;
    validate_xml(document, DOCUMENT)?;
    let xml = String::from_utf8_lossy(document);
    let visible_text = visible_text(document)?;
    if start_tag_count(&xml, "w:tr") != 11
        || ["项目分类", "合同对成果要求简述", "供应商名称", "附脚本"]
            .iter()
            .any(|value| !visible_text.contains(value))
    {
        return Err(validation("制作成果确认 v1 模板结构与登记映射不一致"));
    }
    Ok(())
}

fn validate_data(data: &ProductionResultConfirmationTemplateData) -> Result<(), HostError> {
    data.clean_highlights
        .ok_or_else(|| validation("必须人工确认 clean_highlights"))?;
    for (label, value) in [
        ("附件编号", &data.attachment_label),
        ("文档标题", &data.document_title),
        ("项目分类", &data.category),
        ("项目名称", &data.project_name),
        ("合同名称", &data.contract_title),
        ("合同成果要求", &data.contract_deliverable_summary),
        ("供应商名称", &data.supplier_legal_name),
        ("采购需求时间", &data.procurement_period),
        ("验收描述", &data.acceptance_description),
        ("执行完成日期", &data.execution_completed_date),
        ("成果验收日期", &data.acceptance_date),
    ] {
        required(label, value)?;
    }
    if data.payment_amount_cents < 0 || data.delivery_items.is_empty() {
        return Err(validation("付款金额不能为负，且至少需要一个交付项"));
    }
    if data.storyboards.len() != STORYBOARD_COUNT {
        return Err(validation("附脚本必须为 4 个章节"));
    }
    let shots = data
        .storyboards
        .iter()
        .map(|value| value.shots.len())
        .sum::<usize>();
    if shots != SHOT_COUNT {
        return Err(validation(format!("附脚本必须为 54 个镜号，当前 {shots}")));
    }
    let mut item_ids = HashSet::new();
    let mut shot_numbers = HashSet::new();
    let mut image_count = 0;
    for item in &data.delivery_items {
        for (label, value) in [
            ("交付项 ID", &item.item_id),
            ("交付项名称", &item.name),
            ("需求数量", &item.required_quantity),
            ("单位", &item.unit),
            ("实收数量", &item.received_quantity),
        ] {
            required(label, value)?;
        }
        if !item_ids.insert(item.item_id.trim()) {
            return Err(validation("交付项 ID 不能重复"));
        }
        image_count += item.images.len();
        for image in &item.images {
            validate_image(image)?;
        }
    }
    for storyboard in &data.storyboards {
        for value in [
            &storyboard.title,
            &storyboard.specification,
            &storyboard.production_format,
            &storyboard.duration,
        ] {
            required("脚本章节字段", value)?;
        }
        if storyboard.shots.is_empty() {
            return Err(validation("脚本章节不能没有镜号"));
        }
        for shot in &storyboard.shots {
            required("镜号", &shot.shot_number)?;
            required("画面描述", &shot.description)?;
            if !shot_numbers.insert(shot.shot_number.trim()) {
                return Err(validation("镜号不能重复"));
            }
            if !(1..=3).contains(&shot.images.len()) {
                return Err(validation(format!(
                    "镜号 {} 必须包含 1-3 张图片",
                    shot.shot_number
                )));
            }
            image_count += shot.images.len();
            for image in &shot.images {
                validate_image(image)?;
            }
        }
    }
    if image_count > MAX_IMAGES {
        return Err(validation("图片数量超限"));
    }
    Ok(())
}

fn required(label: &str, value: &str) -> Result<(), HostError> {
    if value.trim().is_empty()
        || value.chars().count() > 20_000
        || value.chars().any(|value| !valid_xml_char(value))
    {
        return Err(validation(format!("{label}为空、过长或包含无效字符")));
    }
    Ok(())
}

fn validate_image(image: &ProductionResultConfirmationImage) -> Result<(), HostError> {
    required("图片 Asset ID", &image.asset_id)?;
    if image.image_bytes.is_empty() || image.image_bytes.len() > MAX_IMAGE {
        return Err(validation("图片大小无效"));
    }
    if image.sha256.len() != 64
        || !image.sha256.bytes().all(|value| value.is_ascii_hexdigit())
        || !sha256(&image.image_bytes).eq_ignore_ascii_case(&image.sha256)
    {
        return Err(validation(format!(
            "图片 {} SHA-256 校验失败",
            image.asset_id
        )));
    }
    let (width, height, _, _) = image_info(&image.image_bytes, &image.mime_type)?;
    if width != image.width_px || height != image.height_px {
        return Err(validation(format!(
            "图片 {} 登记尺寸不一致",
            image.asset_id
        )));
    }
    Ok(())
}

fn render_package(
    mut package: Package,
    destination: &Path,
    data: &ProductionResultConfirmationTemplateData,
) -> Result<(), HostError> {
    let plans = image_plans(data)?;
    replace_entry(
        &mut package,
        DOCUMENT,
        build_document(data, &plans)?.into_bytes(),
    )?;
    replace_or_add(&mut package, RELS, relationships(&plans).into_bytes())?;
    let types = String::from_utf8(entry(&package, TYPES)?.to_vec())
        .map_err(|_| validation("Content Types 不是 UTF-8"))?;
    replace_entry(
        &mut package,
        TYPES,
        content_types(types, &plans)?.into_bytes(),
    )?;
    package
        .entries
        .retain(|value| !value.name.starts_with("word/media/"));
    rebuild_index(&mut package)?;
    for plan in &plans {
        add_entry(
            &mut package,
            PackageEntry {
                name: plan.path.clone(),
                is_dir: false,
                bytes: plan.bytes.clone(),
            },
        )?;
    }
    sanitize(&mut package)?;
    let output = write_package(&package)?;
    verify_output(&output, data, &plans)?;
    publish_no_replace(destination, &output)
}

fn image_plans(
    data: &ProductionResultConfirmationTemplateData,
) -> Result<Vec<ImagePlan>, HostError> {
    data.delivery_items
        .iter()
        .flat_map(|value| value.images.iter())
        .chain(
            data.storyboards
                .iter()
                .flat_map(|value| value.shots.iter())
                .flat_map(|value| value.images.iter()),
        )
        .enumerate()
        .map(|(index, image)| {
            let (_, _, extension, content_type) = image_info(&image.image_bytes, &image.mime_type)?;
            let drawing_id = u32::try_from(index + 1).map_err(|_| validation("图片标识超限"))?;
            Ok(ImagePlan {
                rel_id: format!("rIdProductionResult{drawing_id}"),
                path: format!("word/media/production-result-{drawing_id}.{extension}"),
                extension,
                content_type,
                width: image.width_px,
                height: image.height_px,
                alt_text: if image.alt_text.trim().is_empty() {
                    format!("成果图片 {drawing_id}")
                } else {
                    image.alt_text.trim().to_owned()
                },
                bytes: image.image_bytes.clone(),
                drawing_id,
            })
        })
        .collect()
}

fn build_document(
    data: &ProductionResultConfirmationTemplateData,
    plans: &[ImagePlan],
) -> Result<String, HostError> {
    let mut next_image = 0;
    let payment_amount = format_amount(data.payment_amount_cents);
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>"#,
    );
    xml.push_str(&paragraph(
        &format!("{}：（制作类）", data.attachment_label),
        true,
        false,
    ));
    xml.push_str(&paragraph(&data.document_title, true, false));
    xml.push_str(&main_table_start(&[2000, 2700, 2000, 2700]));
    for (a, b, c, d) in [
        (
            "项目分类",
            data.category.as_str(),
            "项目名称",
            data.project_name.as_str(),
        ),
        (
            "合同名称",
            data.contract_title.as_str(),
            "本次付款金额(元)",
            payment_amount.as_str(),
        ),
        (
            "供应商名称",
            data.supplier_legal_name.as_str(),
            "本次采购需求时间",
            data.procurement_period.as_str(),
        ),
        (
            "成果验收情况描述",
            data.acceptance_description.as_str(),
            "不合格处罚/新增部分",
            data.penalty_or_additions.as_str(),
        ),
    ] {
        xml.push_str(&pair_row(a, b, c, d));
    }
    xml.push_str(&row(
        &[
            cell("合同对成果要求简述", 2000, None),
            cell(&data.contract_deliverable_summary, 7400, None),
        ],
        true,
        false,
    ));
    xml.push_str("</w:tbl>");
    xml.push_str(&main_table_start(&DELIVERY_TABLE_WIDTHS));
    xml.push_str(&header_row(
        &[
            "序号",
            "名称/材质",
            "规格/型号/尺寸",
            "需求数量",
            "单位",
            "实收数量",
            "验收图片",
            "备注",
        ],
        &DELIVERY_TABLE_WIDTHS,
    ));
    for (index, item) in data.delivery_items.iter().enumerate() {
        let images = &plans[next_image..next_image + item.images.len()];
        next_image += item.images.len();
        if index == 1 && images.len() == 2 {
            xml.push_str(&row(
                &[
                    cell(&(index + 1).to_string(), DELIVERY_TABLE_WIDTHS[0], None),
                    cell(&item.name, DELIVERY_TABLE_WIDTHS[1], None),
                    cell(&item.specification, DELIVERY_TABLE_WIDTHS[2], None),
                    cell(&item.required_quantity, DELIVERY_TABLE_WIDTHS[3], None),
                    cell(&item.unit, DELIVERY_TABLE_WIDTHS[4], None),
                    cell(&item.received_quantity, DELIVERY_TABLE_WIDTHS[5], None),
                    delivery_image_cell(&images[..1], DELIVERY_TABLE_WIDTHS[6]),
                    cell(&item.acceptance_note, DELIVERY_TABLE_WIDTHS[7], None),
                ],
                true,
                false,
            ));
            xml.push_str("</w:tbl>");
            xml.push_str(&paragraph("", false, true));
            xml.push_str(&main_table_start(&DELIVERY_TABLE_WIDTHS));
            xml.push_str(&header_row(
                &[
                    "序号",
                    "名称/材质",
                    "规格/型号/尺寸",
                    "需求数量",
                    "单位",
                    "实收数量",
                    "验收图片",
                    "备注",
                ],
                &DELIVERY_TABLE_WIDTHS,
            ));
            xml.push_str(&row(
                &[
                    cell("", DELIVERY_TABLE_WIDTHS[0], None),
                    cell("", DELIVERY_TABLE_WIDTHS[1], None),
                    cell("", DELIVERY_TABLE_WIDTHS[2], None),
                    cell("", DELIVERY_TABLE_WIDTHS[3], None),
                    cell("", DELIVERY_TABLE_WIDTHS[4], None),
                    cell("", DELIVERY_TABLE_WIDTHS[5], None),
                    delivery_image_cell(&images[1..], DELIVERY_TABLE_WIDTHS[6]),
                    cell("", DELIVERY_TABLE_WIDTHS[7], None),
                ],
                true,
                false,
            ));
            continue;
        }
        xml.push_str(&row(
            &[
                cell(&(index + 1).to_string(), DELIVERY_TABLE_WIDTHS[0], None),
                cell(&item.name, DELIVERY_TABLE_WIDTHS[1], None),
                cell(&item.specification, DELIVERY_TABLE_WIDTHS[2], None),
                cell(&item.required_quantity, DELIVERY_TABLE_WIDTHS[3], None),
                cell(&item.unit, DELIVERY_TABLE_WIDTHS[4], None),
                cell(&item.received_quantity, DELIVERY_TABLE_WIDTHS[5], None),
                delivery_image_cell(images, DELIVERY_TABLE_WIDTHS[6]),
                cell(&item.acceptance_note, DELIVERY_TABLE_WIDTHS[7], None),
            ],
            true,
            false,
        ));
    }
    xml.push_str("</w:tbl>");
    xml.push_str(&paragraph("", false, true));
    xml.push_str(&main_table_start(&[2000, 2700, 2000, 2700]));
    for (a, b, c, d) in [
        (
            "本次执行完成日期",
            data.execution_completed_date.as_str(),
            "本次执行成果验收日期",
            data.acceptance_date.as_str(),
        ),
        (
            "经办人验收意见及签字",
            data.handler_signoff.as_str(),
            "专业负责人意见及签字(平台)",
            data.professional_lead_signoff.as_str(),
        ),
        (
            "其他部门意见及签字",
            data.other_department_signoff.as_str(),
            "供应商经办人意见及签字",
            data.supplier_handler_signoff.as_str(),
        ),
    ] {
        xml.push_str(&pair_row(a, b, c, d));
    }
    xml.push_str("</w:tbl>");
    xml.push_str(&paragraph("附脚本：", true, true));
    for (chapter_index, storyboard) in data.storyboards.iter().enumerate() {
        xml.push_str(&paragraph(
            &format!("第{}章 {}", chapter_index + 1, storyboard.title),
            true,
            chapter_index > 0,
        ));
        xml.push_str(&paragraph(
            &format!(
                "规格：{}    形式：{}    时长：{}",
                storyboard.specification, storyboard.production_format, storyboard.duration
            ),
            false,
            false,
        ));
        xml.push_str(&table_start(&[700, 3100, 2500, 1900, 1200]));
        xml.push_str(&header_row(
            &["镜号", "画面", "画面描述", "贴屏文案", "备注"],
            &[700, 3100, 2500, 1900, 1200],
        ));
        for (shot_index, shot) in storyboard.shots.iter().enumerate() {
            let images = &plans[next_image..next_image + shot.images.len()];
            next_image += shot.images.len();
            let highlight = (data.clean_highlights == Some(false) && shot.source_highlighted)
                .then_some("yellow");
            let closing_sequence_start = storyboard.shots.len().saturating_sub(4);
            let emphasizes_closing_sequence = chapter_index + 1 == data.storyboards.len()
                && shot_index >= closing_sequence_start
                && shot_index < closing_sequence_start + 2;
            let image_max_height = if emphasizes_closing_sequence {
                1_900_000
            } else {
                1_780_000
            };
            xml.push_str(&row(
                &[
                    cell(&shot.shot_number, 700, None),
                    image_cell_with_text(&shot.scene, images, 5_500_000, image_max_height, 3100)?,
                    cell(&shot.description, 2500, highlight),
                    cell(&shot.on_screen_copy, 1900, highlight),
                    cell(&shot.remarks, 1200, highlight),
                ],
                true,
                false,
            ));
        }
        xml.push_str("</w:tbl>");
    }
    xml.push_str(r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr></w:body></w:document>"#);
    validate_xml(xml.as_bytes(), "生成 document.xml")?;
    Ok(xml)
}

fn table_start(widths: &[u32]) -> String {
    table_start_with_width(widths, None)
}

fn main_table_start(widths: &[u32]) -> String {
    let width = widths.iter().copied().sum::<u32>();
    debug_assert!(width <= PAGE_CONTENT_WIDTH_DXA);
    table_start_with_width(widths, Some(width))
}

fn table_start_with_width(widths: &[u32], fixed_width: Option<u32>) -> String {
    let grid = widths
        .iter()
        .map(|value| format!("<w:gridCol w:w=\"{value}\"/>"))
        .collect::<String>();
    let table = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblLayout w:type="fixed"/><w:tblBorders><w:top w:val="single" w:sz="8"/><w:left w:val="single" w:sz="8"/><w:bottom w:val="single" w:sz="8"/><w:right w:val="single" w:sz="8"/><w:insideH w:val="single" w:sz="6"/><w:insideV w:val="single" w:sz="6"/></w:tblBorders></w:tblPr><w:tblGrid>{grid}</w:tblGrid>"#
    );
    match fixed_width {
        Some(width) => table.replacen(
            r#"<w:tblW w:w="0" w:type="auto"/>"#,
            &format!(r#"<w:tblW w:w="{width}" w:type="dxa"/>"#),
            1,
        ),
        None => table,
    }
}

fn pair_row(a: &str, b: &str, c: &str, d: &str) -> String {
    row(
        &[
            cell(a, 2000, None),
            cell(b, 2700, None),
            cell(c, 2000, None),
            cell(d, 2700, None),
        ],
        true,
        false,
    )
}

fn header_row(values: &[&str], widths: &[u32]) -> String {
    row(
        &values
            .iter()
            .zip(widths)
            .map(|(value, width)| cell(value, *width, None))
            .collect::<Vec<_>>(),
        true,
        true,
    )
}

fn row(cells: &[String], cant_split: bool, header: bool) -> String {
    let mut result = String::from("<w:tr><w:trPr>");
    if cant_split {
        result.push_str("<w:cantSplit/>");
    }
    if header {
        result.push_str("<w:tblHeader/>");
    }
    result.push_str("</w:trPr>");
    for cell in cells {
        result.push_str(cell);
    }
    result.push_str("</w:tr>");
    result
}

fn cell(value: &str, width: u32, highlight: Option<&str>) -> String {
    let highlight = highlight
        .map(|value| format!("<w:highlight w:val=\"{}\"/>", escape(value)))
        .unwrap_or_default();
    format!("<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/><w:vAlign w:val=\"center\"/></w:tcPr><w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:rPr>{highlight}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>", escape(value))
}

fn paragraph(value: &str, bold: bool, page_break: bool) -> String {
    format!("<w:p><w:pPr><w:jc w:val=\"center\"/>{}</w:pPr><w:r><w:rPr>{}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", if page_break { "<w:pageBreakBefore/>" } else { "" }, if bold { "<w:b/>" } else { "" }, escape(value))
}

fn image_cell(
    plans: &[ImagePlan],
    max_width: u64,
    max_height: u64,
    width: u32,
) -> Result<String, HostError> {
    image_cell_with_text("", plans, max_width, max_height, width)
}

fn delivery_image_cell(plans: &[ImagePlan], width: u32) -> String {
    let content = plans
        .iter()
        .map(|plan| {
            let (cx, cy) = fit_image(plan.width, plan.height, 3_900_000, 2_900_000);
            format!(
                "<w:p><w:pPr><w:keepNext/><w:keepLines/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p><w:p><w:pPr><w:jc w:val=\"center\"/><w:keepLines/></w:pPr>{}</w:p>",
                escape(&plan.alt_text),
                drawing(plan, cx, cy)
            )
        })
        .collect::<String>();
    format!("<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/><w:vAlign w:val=\"center\"/></w:tcPr>{content}</w:tc>")
}

fn image_cell_with_text(
    text: &str,
    plans: &[ImagePlan],
    max_width: u64,
    max_height: u64,
    width: u32,
) -> Result<String, HostError> {
    let slots = plans.len().max(1) as u64;
    let slot_width = max_width.saturating_sub(90_000 * (slots - 1)) / slots;
    let drawings = plans
        .iter()
        .map(|plan| {
            let (cx, cy) = fit_image(plan.width, plan.height, slot_width, max_height);
            drawing(plan, cx, cy)
        })
        .collect::<String>();
    Ok(format!("<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/><w:vAlign w:val=\"center\"/></w:tcPr><w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p><w:p><w:pPr><w:jc w:val=\"center\"/><w:keepLines/></w:pPr>{drawings}</w:p></w:tc>", escape(text)))
}

fn fit_image(width: u32, height: u32, max_width: u64, max_height: u64) -> (u64, u64) {
    let width = u64::from(width);
    let height = u64::from(height);
    let fitted_height = max_width * height / width;
    if fitted_height <= max_height {
        (max_width, fitted_height.max(1))
    } else {
        ((max_height * width / height).max(1), max_height)
    }
}

fn drawing(plan: &ImagePlan, cx: u64, cy: u64) -> String {
    let alt = escape(&plan.alt_text);
    let id = plan.drawing_id;
    format!(
        r#"<w:r><w:drawing><wp:inline><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="ProductionResult{id}" descr="{alt}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="ProductionResult{id}.{}"/><pic:cNvPicPr><a:picLocks noChangeAspect="1"/></pic:cNvPicPr></pic:nvPicPr><pic:blipFill><a:blip r:embed="{}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        plan.extension, plan.rel_id
    )
}

fn relationships(plans: &[ImagePlan]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for plan in plans {
        xml.push_str(&format!(r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/production-result-{}.{}"/>"#, plan.rel_id, plan.drawing_id, plan.extension));
    }
    xml.push_str("</Relationships>");
    xml
}

fn content_types(mut xml: String, plans: &[ImagePlan]) -> Result<String, HostError> {
    let close = xml
        .rfind("</Types>")
        .ok_or_else(|| validation("Content Types 结构无效"))?;
    let mut additions = String::new();
    for (extension, content_type) in plans
        .iter()
        .map(|value| (value.extension, value.content_type))
        .collect::<HashSet<_>>()
    {
        if !xml.contains(&format!("Extension=\"{extension}\"")) {
            additions.push_str(&format!(
                "<Default Extension=\"{extension}\" ContentType=\"{content_type}\"/>"
            ));
        }
    }
    xml.insert_str(close, &additions);
    validate_xml(xml.as_bytes(), TYPES)?;
    Ok(xml)
}

fn verify_output(
    bytes: &[u8],
    data: &ProductionResultConfirmationTemplateData,
    plans: &[ImagePlan],
) -> Result<(), HostError> {
    let package = load_package(bytes)?;
    validate_safe_package(&package)?;
    let xml = String::from_utf8_lossy(entry(&package, DOCUMENT)?);
    for value in [
        &data.attachment_label,
        &data.document_title,
        &data.project_name,
        &data.contract_title,
        &data.supplier_legal_name,
    ] {
        if !xml.contains(&escape(value)) {
            return Err(internal("输出缺少冻结业务字段"));
        }
    }
    let frozen_values = [
        data.category.as_str(),
        data.project_name.as_str(),
        data.contract_title.as_str(),
        data.supplier_legal_name.as_str(),
        data.procurement_period.as_str(),
    ];
    for stale in [
        "华润置地白鹅潭瑞玺",
        "广州华邦互娱科技有限公司",
        "2026年   月",
        "□市场咨询",
    ] {
        if xml.contains(stale) && !frozen_values.iter().any(|value| value.contains(stale)) {
            return Err(internal(format!("输出残留模板示例值: {stale}")));
        }
    }
    if xml.matches("<w:cantSplit/>").count()
        < data.delivery_items.len() + SHOT_COUNT + STORYBOARD_COUNT
    {
        return Err(internal("动态表格行缺少不可拆分页约束"));
    }
    if xml.matches("<wp:docPr ").count() != plans.len()
        || xml.matches("<pic:cNvPr ").count() != plans.len()
    {
        return Err(internal("图片 DrawingML 数量不一致"));
    }
    for plan in plans {
        if entry(&package, &plan.path)? != plan.bytes {
            return Err(internal("输出图片内容不一致"));
        }
    }
    Ok(())
}

fn sanitize(package: &mut Package) -> Result<(), HostError> {
    validate_safe_package(package)
}

fn validate_safe_package(package: &Package) -> Result<(), HostError> {
    for required in [TYPES, "_rels/.rels", DOCUMENT] {
        entry(package, required)?;
    }
    for value in &package.entries {
        let name = value.name.to_ascii_lowercase();
        if name.ends_with("vbaproject.bin")
            || name.contains("/embeddings/")
            || name.ends_with(".exe")
            || name.ends_with(".dll")
        {
            return Err(validation(format!(
                "DOCX 包含禁止的活动内容: {}",
                value.name
            )));
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            validate_xml(&value.bytes, &value.name)?;
            let xml = String::from_utf8_lossy(&value.bytes);
            if name.ends_with(".rels")
                && (xml.contains("TargetMode=\"External\"")
                    || xml.contains("TargetMode='External'"))
            {
                return Err(validation(format!("DOCX 包含外部关系: {}", value.name)));
            }
        }
    }
    Ok(())
}

fn validate_xml(bytes: &[u8], label: &str) -> Result<(), HostError> {
    let upper = String::from_utf8_lossy(bytes).to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(validation(format!("{label} 包含禁止的 DTD/实体")));
    }
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_end_names = true;
    let mut depth = 0_usize;
    let mut roots = 0_usize;
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
                if depth > 256 {
                    return Err(validation(format!("{label} XML 嵌套过深")));
                }
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    roots += 1;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| validation(format!("{label} XML 结构无效")))?
            }
            Ok(_) => {}
            Err(error) => return Err(validation(format!("{label} XML 格式无效: {error}"))),
        }
        if roots > 1 {
            return Err(validation(format!("{label} 包含多个根节点")));
        }
    }
    if roots != 1 || depth != 0 {
        return Err(validation(format!("{label} XML 根结构无效")));
    }
    Ok(())
}

fn visible_text(bytes: &[u8]) -> Result<String, HostError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_end_names = true;
    let mut inside_text = false;
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(value)) if value.local_name().as_ref() == b"t" => inside_text = true,
            Ok(Event::End(value)) if value.local_name().as_ref() == b"t" => inside_text = false,
            Ok(Event::Text(value)) if inside_text => output.push_str(
                &value
                    .decode()
                    .map_err(|_| validation("DOCX 可见文本编码无效"))?,
            ),
            Ok(_) => {}
            Err(error) => return Err(validation(format!("DOCX 可见文本 XML 无效: {error}"))),
        }
    }
    Ok(output)
}

fn load_package(bytes: &[u8]) -> Result<Package, HostError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| validation(format!("DOCX 不是有效 ZIP: {error}")))?;
    if archive.is_empty() || archive.len() > 256 {
        return Err(validation("DOCX 条目数量超限"));
    }
    let mut entries = Vec::new();
    let mut by_name = HashMap::new();
    let mut folded = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut source = archive
            .by_index(index)
            .map_err(|error| validation(format!("读取 DOCX 条目失败: {error}")))?;
        let name = source.name().to_owned();
        validate_zip_name(&name)?;
        if !folded.insert(name.to_ascii_lowercase()) {
            return Err(validation(format!("DOCX 重复条目: {name}")));
        }
        if source.size() > MAX_ENTRY
            || source.compressed_size() > 0
                && source.size() > source.compressed_size().saturating_mul(250)
        {
            return Err(validation(format!("DOCX 条目异常: {name}")));
        }
        total = total
            .checked_add(source.size())
            .ok_or_else(|| validation("DOCX 大小溢出"))?;
        if total > MAX_TOTAL {
            return Err(validation("DOCX 解压总大小超限"));
        }
        if source
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(validation("DOCX 禁止符号链接"));
        }
        let is_dir = source.is_dir();
        let mut contents = Vec::new();
        if !is_dir {
            source
                .read_to_end(&mut contents)
                .map_err(|error| validation(format!("解压 DOCX 失败: {error}")))?;
        }
        by_name.insert(name.clone(), entries.len());
        entries.push(PackageEntry {
            name,
            is_dir,
            bytes: contents,
        });
    }
    Ok(Package { entries, by_name })
}

fn write_package(package: &Package) -> Result<Vec<u8>, HostError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for value in &package.entries {
        if value.is_dir {
            writer
                .add_directory(&value.name, options)
                .map_err(|error| internal(error.to_string()))?;
        } else {
            writer
                .start_file(&value.name, options)
                .map_err(|error| internal(error.to_string()))?;
            writer
                .write_all(&value.bytes)
                .map_err(|error| internal(error.to_string()))?;
        }
    }
    writer
        .finish()
        .map(|value| value.into_inner())
        .map_err(|error| internal(error.to_string()))
}

fn entry<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], HostError> {
    package
        .by_name
        .get(name)
        .and_then(|index| package.entries.get(*index))
        .map(|value| value.bytes.as_slice())
        .ok_or_else(|| validation(format!("DOCX 缺少条目: {name}")))
}

fn replace_entry(package: &mut Package, name: &str, bytes: Vec<u8>) -> Result<(), HostError> {
    let index = *package
        .by_name
        .get(name)
        .ok_or_else(|| validation(format!("DOCX 缺少条目: {name}")))?;
    package.entries[index].bytes = bytes;
    Ok(())
}
fn replace_or_add(package: &mut Package, name: &str, bytes: Vec<u8>) -> Result<(), HostError> {
    if package.by_name.contains_key(name) {
        replace_entry(package, name, bytes)
    } else {
        add_entry(
            package,
            PackageEntry {
                name: name.to_owned(),
                is_dir: false,
                bytes,
            },
        )
    }
}
fn add_entry(package: &mut Package, value: PackageEntry) -> Result<(), HostError> {
    if package
        .by_name
        .insert(value.name.clone(), package.entries.len())
        .is_some()
    {
        return Err(validation("DOCX 重复条目"));
    }
    package.entries.push(value);
    Ok(())
}
fn rebuild_index(package: &mut Package) -> Result<(), HostError> {
    package.by_name.clear();
    for (index, value) in package.entries.iter().enumerate() {
        if package.by_name.insert(value.name.clone(), index).is_some() {
            return Err(validation("DOCX 重复条目"));
        }
    }
    Ok(())
}

fn validate_destination(path: &Path) -> Result<(), HostError> {
    fs::canonicalize(path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|error| internal(format!("读取输出目录失败: {error}")))?;
    if path.exists() {
        return Err(validation("输出文件已存在"));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("docx"))
    {
        return Err(validation("输出必须为 .docx"));
    }
    Ok(())
}

fn publish_no_replace(destination: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .ok_or_else(|| validation("输出文件名无效"))?
        .to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| internal(error.to_string()))?
        .as_nanos();
    let mut staged: Option<PathBuf> = None;
    for _ in 0..32 {
        let path = parent.join(format!(
            ".{name}.{}.{}.{}.tmp",
            std::process::id(),
            stamp,
            STAGED_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|error| internal(error.to_string()))?;
                file.sync_all()
                    .map_err(|error| internal(error.to_string()))?;
                staged = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(internal(error.to_string())),
        }
    }
    let staged = staged.ok_or_else(|| internal("无法创建临时输出"))?;
    let result = fs::hard_link(&staged, destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            validation("输出文件已存在")
        } else {
            internal(error.to_string())
        }
    });
    let cleanup = fs::remove_file(&staged);
    result.and(cleanup.map_err(|error| internal(error.to_string())))
}

fn image_info(
    bytes: &[u8],
    mime: &str,
) -> Result<(u32, u32, &'static str, &'static str), HostError> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => png_dimensions(bytes).map(|(w, h)| (w, h, "png", "image/png")),
        "image/jpeg" | "image/jpg" => {
            jpeg_dimensions(bytes).map(|(w, h)| (w, h, "jpg", "image/jpeg"))
        }
        _ => Err(validation("仅支持 PNG/JPEG 图片")),
    }
}
fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), HostError> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err(validation("PNG 文件头无效"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if width == 0 || height == 0 {
        return Err(validation("PNG 尺寸无效"));
    }
    Ok((width, height))
}
fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), HostError> {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return Err(validation("JPEG 文件头无效"));
    }
    let mut index = 2;
    while index + 4 <= bytes.len() {
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let marker = bytes[index];
        index += 1;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        if index + 2 > bytes.len() {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[index], bytes[index + 1]]));
        if length < 2 || index + length > bytes.len() {
            return Err(validation("JPEG 段无效"));
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 7 {
                return Err(validation("JPEG 尺寸段无效"));
            }
            let h = u32::from(u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]));
            let w = u32::from(u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]));
            return if w > 0 && h > 0 {
                Ok((w, h))
            } else {
                Err(validation("JPEG 尺寸无效"))
            };
        }
        index += length;
    }
    Err(validation("JPEG 缺少尺寸段"))
}

fn validate_zip_name(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.starts_with('/')
        || (name.len() >= 2 && name.as_bytes()[1] == b':')
        || name
            .trim_end_matches('/')
            .split('/')
            .any(|value| value.is_empty() || value == "." || value == "..")
    {
        return Err(validation(format!("DOCX 不安全条目: {name}")));
    }
    Ok(())
}
fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>, HostError> {
    let file = File::open(path).map_err(|error| internal(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| internal(error.to_string()))?;
    if bytes.len() as u64 > max {
        return Err(validation("文件过大"));
    }
    Ok(bytes)
}
fn format_amount(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}
fn start_tag_count(xml: &str, tag: &str) -> usize {
    xml.matches(&format!("<{tag}>")).count() + xml.matches(&format!("<{tag} ")).count()
}
fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn valid_xml_char(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{a}' | '\u{d}')
        || matches!(value as u32, 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
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

    fn png(_width: u32, _height: u32) -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x08,
            0x99, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x54, 0xa2, 0x4f, 0x5d, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }

    fn image(id: &str, width: u32, height: u32) -> ProductionResultConfirmationImage {
        let bytes = png(width, height);
        ProductionResultConfirmationImage {
            asset_id: id.to_owned(),
            sha256: sha256(&bytes),
            mime_type: "image/png".to_owned(),
            width_px: 1,
            height_px: 1,
            alt_text: format!("图片 {id}"),
            image_bytes: bytes,
        }
    }

    fn external_qa_fixture(relative_path: &str) -> PathBuf {
        std::env::var_os("BSAIGC_EXTERNAL_QA_FIXTURE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/synthetic/business-v1"))
            .join(relative_path)
    }

    fn fixture() -> ProductionResultConfirmationTemplateData {
        let mut storyboards = Vec::new();
        let mut shot_number = 1;
        for chapter in 0..4 {
            let count = if chapter < 2 { 14 } else { 13 };
            let mut shots = Vec::new();
            for local in 0..count {
                let image_count = local % 3 + 1;
                shots.push(ProductionResultConfirmationShot {
                    shot_number: shot_number.to_string(),
                    scene: format!("场景 {shot_number}"),
                    description: format!("描述 {shot_number}"),
                    on_screen_copy: format!("文案 {shot_number}"),
                    remarks: String::new(),
                    source_highlighted: shot_number % 2 == 0,
                    images: (0..image_count)
                        .map(|index| {
                            image(
                                &format!("shot-{shot_number}-{index}"),
                                1600 + index as u32 * 200,
                                900,
                            )
                        })
                        .collect(),
                });
                shot_number += 1;
            }
            storyboards.push(ProductionResultConfirmationStoryboard {
                title: format!("脚本章节 {}", chapter + 1),
                specification: "16:9".to_owned(),
                production_format: "实拍".to_owned(),
                duration: "60秒".to_owned(),
                shots,
            });
        }
        ProductionResultConfirmationTemplateData {
            attachment_label: "附件测试".to_owned(),
            document_title: "测试项目制作成果确认书".to_owned(),
            category: "影视片制作".to_owned(),
            project_name: "测试项目".to_owned(),
            contract_title: "测试合同".to_owned(),
            payment_amount_cents: 123_456,
            contract_deliverable_summary: "交付完整制作成果".to_owned(),
            supplier_legal_name: "测试供应商有限公司".to_owned(),
            procurement_period: "2026年7月".to_owned(),
            acceptance_description: "验收通过".to_owned(),
            penalty_or_additions: "无".to_owned(),
            delivery_items: vec![
                ProductionResultConfirmationDeliveryItem {
                    item_id: "delivery-1".to_owned(),
                    name: "合成宣传片 A、合成宣传片 B".to_owned(),
                    specification: "形象宣传片 30-60s".to_owned(),
                    required_quantity: "2".to_owned(),
                    unit: "条".to_owned(),
                    received_quantity: "2".to_owned(),
                    acceptance_note: "视频完成制作，现场播放".to_owned(),
                    images: vec![
                        image("delivery-1-1", 1920, 1080),
                        image("delivery-1-2", 1920, 1080),
                    ],
                },
                ProductionResultConfirmationDeliveryItem {
                    item_id: "delivery-2".to_owned(),
                    name: "合成品牌片 A、合成品牌片 B".to_owned(),
                    specification: "轻量级品牌调性片 30-60s".to_owned(),
                    required_quantity: "2".to_owned(),
                    unit: "条".to_owned(),
                    received_quantity: "2".to_owned(),
                    acceptance_note: "视频完成制作，现场播放".to_owned(),
                    images: vec![
                        image("delivery-2-1", 1920, 1080),
                        image("delivery-2-2", 1920, 1080),
                    ],
                },
                ProductionResultConfirmationDeliveryItem {
                    item_id: "delivery-3".to_owned(),
                    name: "合成 AIGC 视频 A、合成 AIGC 视频 B".to_owned(),
                    specification: "AIGC 创意广告视频 30-60s".to_owned(),
                    required_quantity: "2".to_owned(),
                    unit: "条".to_owned(),
                    received_quantity: "2".to_owned(),
                    acceptance_note: "视频完成制作，现场播放".to_owned(),
                    images: vec![
                        image("delivery-3-1", 1920, 1080),
                        image("delivery-3-2", 1920, 1080),
                    ],
                },
            ],
            execution_completed_date: "2026年7月28日".to_owned(),
            acceptance_date: "2026年7月29日".to_owned(),
            handler_signoff: String::new(),
            professional_lead_signoff: String::new(),
            other_department_signoff: String::new(),
            supplier_handler_signoff: String::new(),
            storyboards,
            clean_highlights: Some(true),
        }
    }

    fn registered_fixture() -> ProductionResultConfirmationTemplateData {
        let paths = [
            external_qa_fixture("scripts/synthetic-series-01.docx"),
            external_qa_fixture("scripts/synthetic-series-02.docx"),
            external_qa_fixture("scripts/synthetic-series-03.docx"),
        ];
        let mut seen = HashSet::new();
        let mut images = Vec::new();
        for path in paths {
            let package = load_package(&read_limited(&path, MAX_SOURCE).unwrap()).unwrap();
            for entry in package.entries {
                if entry.is_dir || !entry.name.starts_with("word/media/") {
                    continue;
                }
                let mime_type = match Path::new(&entry.name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref()
                {
                    Some("png") => "image/png",
                    Some("jpg" | "jpeg") => "image/jpeg",
                    _ => continue,
                };
                let Ok((width_px, height_px, _, _)) = image_info(&entry.bytes, mime_type) else {
                    continue;
                };
                let digest = sha256(&entry.bytes);
                if !seen.insert(digest.clone()) {
                    continue;
                }
                images.push(ProductionResultConfirmationImage {
                    asset_id: format!("real-{}", images.len() + 1),
                    sha256: digest,
                    mime_type: mime_type.to_owned(),
                    width_px,
                    height_px,
                    alt_text: entry.name,
                    image_bytes: entry.bytes,
                });
            }
        }
        images.sort_by(|left, right| {
            let left_landscape = left.width_px >= left.height_px;
            let right_landscape = right.width_px >= right.height_px;
            right_landscape
                .cmp(&left_landscape)
                .then_with(|| right.image_bytes.len().cmp(&left.image_bytes.len()))
                .then_with(|| left.asset_id.cmp(&right.asset_id))
        });
        assert!(images.len() >= 60, "真实脚本图片不足 60 张");
        let mut images = images.into_iter();
        let mut data = fixture();
        let evidence_labels = [
            "[SYNTHETIC_FIXTURE]\n[视频名称] 合成长视频\n[视频类型] 长视频-形象宣传片\n[视频内容] 合成宣传片 A、合成宣传片 B\n[视频时长] 30-60s\n[服务内容] 合成策划、脚本、拍摄、剪辑、动画与配音\n[含税价] 10000元*2条=20000元\n第一条\n合成验收附件：synthetic-video-01.mp4\n链接：https://example.invalid/files/synthetic-video-01\n访问码：SYNTHETIC-01",
            "[SYNTHETIC_FIXTURE]\n第二条\n合成验收附件：synthetic-video-02.mp4\n链接：https://example.invalid/files/synthetic-video-02\n访问码：SYNTHETIC-02",
            "[SYNTHETIC_FIXTURE]\n[视频名称] 合成品牌类短视频\n[视频类型] 品牌类短视频-轻量级品牌调性片\n[视频内容] 合成品牌片 A、合成品牌片 B\n[视频时长] 30-60s\n[服务内容] 合成策划、脚本、拍摄、剪辑、花字与配音\n[含税价] 8000元*2条=16000元\n第一条\n合成验收附件：synthetic-video-03.mp4\n链接：https://example.invalid/files/synthetic-video-03\n访问码：SYNTHETIC-03",
            "[SYNTHETIC_FIXTURE]\n第二条\n合成验收附件：synthetic-video-04.mp4\n链接：https://example.invalid/files/synthetic-video-04\n访问码：SYNTHETIC-04",
            "[SYNTHETIC_FIXTURE]\n[视频名称] 合成 AIGC 类\n[视频类型] AIGC 类-创意广告视频\n[视频内容] 合成 AIGC 视频 A、合成 AIGC 视频 B\n[视频时长] 30-60s\n[服务内容] 合成脚本、分镜、效果渲染、剪辑、平面设计与配音\n[含税价] 9000元*2条=18000元\n第一条\n合成验收附件：synthetic-video-05.mp4\n链接：https://example.invalid/files/synthetic-video-05\n访问码：SYNTHETIC-05",
            "[SYNTHETIC_FIXTURE]\n第二条\n合成验收附件：synthetic-video-06.mp4\n链接：https://example.invalid/files/synthetic-video-06\n访问码：SYNTHETIC-06",
        ];
        for (reference, label) in data
            .delivery_items
            .iter_mut()
            .flat_map(|item| item.images.iter_mut())
            .zip(evidence_labels)
        {
            let mut real = images.next().unwrap();
            real.alt_text = label.to_owned();
            *reference = real;
        }
        for shot in data
            .storyboards
            .iter_mut()
            .flat_map(|storyboard| storyboard.shots.iter_mut())
        {
            let mut real = images.next().unwrap();
            real.alt_text = format!("镜号 {}：{}", shot.shot_number, shot.description);
            shot.images = vec![real];
        }
        data
    }

    fn package(extra: Option<&str>) -> Vec<u8> {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, xml) in [
            (
                TYPES,
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                DOCUMENT,
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>附脚本</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>项目分类</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>合同对成果要求简述</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>供应商名称</w:t></w:r></w:p></w:tc></w:tr><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/><w:tr/></w:tbl></w:body></w:document>"#,
            ),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(xml.as_bytes()).unwrap();
        }
        if let Some(name) = extra {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"bad").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn requires_explicit_highlight_decision_and_exact_storyboard_shape() {
        let mut data = fixture();
        data.clean_highlights = None;
        assert!(validate_data(&data).is_err());
        data.clean_highlights = Some(true);
        data.storyboards.pop();
        assert!(validate_data(&data).is_err());
    }

    #[test]
    fn rejects_bad_image_hash_dimensions_format_and_count() {
        let mut data = fixture();
        data.storyboards[0].shots[0].images[0].sha256 = "A".repeat(64);
        assert!(validate_data(&data).is_err());
        let mut data = fixture();
        data.storyboards[0].shots[0].images[0].width_px += 1;
        assert!(validate_data(&data).is_err());
        let mut data = fixture();
        data.storyboards[0].shots[0].images[0].mime_type = "image/gif".to_owned();
        assert!(validate_data(&data).is_err());
        let mut data = fixture();
        data.storyboards[0].shots[0].images.clear();
        assert!(validate_data(&data).is_err());
    }

    #[test]
    fn rejects_malicious_zip_paths_active_content_and_external_relationships() {
        assert!(load_package(&package(Some("../escape.xml"))).is_err());
        assert!(validate_safe_package(
            &load_package(&package(Some("word/vbaProject.bin"))).unwrap()
        )
        .is_err());
        let mut value = load_package(&package(None)).unwrap();
        add_entry(&mut value, PackageEntry { name: RELS.to_owned(), is_dir: false, bytes: br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="x" TargetMode="External" Target="https://example.invalid"/></Relationships>"#.to_vec() }).unwrap();
        assert!(validate_safe_package(&value).is_err());
    }

    #[test]
    fn template_hash_gate_is_strict_and_failure_is_atomic() {
        let source = package(None);
        let destination = std::env::temp_dir().join(format!(
            "production-result-hash-{}.docx",
            std::process::id()
        ));
        let _ = fs::remove_file(&destination);
        assert!(render_production_result_confirmation_template_from_bytes(
            &source,
            &sha256(&source),
            &destination,
            &fixture()
        )
        .is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn renders_one_two_three_images_and_pagination_guards() {
        let source = package(None);
        let destination = std::env::temp_dir().join(format!(
            "production-result-layout-{}-{}.docx",
            std::process::id(),
            STAGED_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&destination);
        let data = fixture();
        render_package(load_package(&source).unwrap(), &destination, &data).unwrap();
        let output = load_package(&read_limited(&destination, MAX_SOURCE).unwrap()).unwrap();
        let xml = String::from_utf8_lossy(entry(&output, DOCUMENT).unwrap());
        assert_eq!(PAGE_CONTENT_WIDTH_DXA, 9_638);
        for widths in [&[2000, 2700, 2000, 2700][..], &DELIVERY_TABLE_WIDTHS[..]] {
            assert!(widths.iter().copied().sum::<u32>() <= PAGE_CONTENT_WIDTH_DXA);
        }
        assert_eq!(
            xml.matches(r#"<w:tblW w:w="9400" w:type="dxa"/>"#).count(),
            4
        );
        assert_eq!(
            xml.matches(r#"<w:tblW w:w="0" w:type="auto"/>"#).count(),
            STORYBOARD_COUNT
        );
        assert_eq!(xml.matches("<w:tblHeader/>").count(), 6);
        assert_eq!(xml.matches("<w:pageBreakBefore/>").count(), 6);
        assert!(xml.matches("<w:cantSplit/>").count() >= 1 + 4 + 54);
        assert_eq!(data.delivery_items.len(), 3);
        assert_eq!(
            data.delivery_items
                .iter()
                .map(|item| item.images.len())
                .sum::<usize>(),
            6
        );
        assert_eq!(xml.matches("<w:keepNext/>").count(), 6);
        assert_eq!(
            xml.matches("<wp:docPr ").count(),
            image_plans(&data).unwrap().len()
        );
        assert!(!xml.contains("<w:highlight"));
        assert!(render_package(load_package(&source).unwrap(), &destination, &data).is_err());
        fs::remove_file(destination).unwrap();
    }

    #[test]
    fn preserves_source_highlights_only_after_explicit_false() {
        let mut data = fixture();
        data.clean_highlights = Some(false);
        let xml = build_document(&data, &image_plans(&data).unwrap()).unwrap();
        assert!(xml.contains("<w:highlight w:val=\"yellow\"/>"));
    }

    #[test]
    #[ignore = "requires registered customer template outside repository"]
    fn registered_template_hash_and_structure() {
        let path = external_qa_fixture("templates/synthetic-production-confirmation.docx");
        let bytes = read_limited(&path, MAX_SOURCE).unwrap();
        assert_eq!(
            sha256(&bytes),
            PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256
        );
        validate_source(&bytes, PRODUCTION_RESULT_CONFIRMATION_TEMPLATE_SHA256).unwrap();
    }

    #[test]
    #[ignore = "requires registered customer template outside repository"]
    fn registered_template_end_to_end_render() {
        let source = external_qa_fixture("templates/synthetic-production-confirmation.docx");
        let destination = std::env::temp_dir().join(format!(
            "production-result-real-{}-{}.docx",
            std::process::id(),
            STAGED_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&destination);
        render_production_result_confirmation_template(
            &source,
            &destination,
            &registered_fixture(),
        )
        .unwrap();
        assert!(destination.metadata().unwrap().len() > 20_000);
        if std::env::var_os("BSAIGC_KEEP_RENDERED_ARTIFACT").is_none() {
            fs::remove_file(destination).unwrap();
        } else {
            eprintln!(
                "rendered production result confirmation: {}",
                destination.display()
            );
        }
    }
}

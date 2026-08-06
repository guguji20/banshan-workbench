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
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const PAYMENT_APPLICATION_LEGACY_DOC_SHA256: &str =
    "E1BF122AFDF3EF15017F3D82E9CAB5DA1C8D3BE38FEA40299906EE61538D5072";

const DOCUMENT_PATH: &str = "word/document.xml";
const CORE_PROPERTIES_PATH: &str = "docProps/core.xml";
const SETTINGS_PATH: &str = "word/settings.xml";
const CONTENT_TYPES_PATH: &str = "[Content_Types].xml";
const ROOT_RELATIONSHIPS_PATH: &str = "_rels/.rels";
const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 2_048;
const MAX_FIELD_BYTES: usize = 8 * 1024;
const MAX_SETTLEMENT_ITEMS: usize = 32;
const QUANTITY_SCALE: i64 = 1_000;

static STAGED_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentBankAccount {
    pub recipient_name: String,
    pub bank_name: String,
    pub account_number: String,
    pub routing_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentSettlementItem {
    pub name: String,
    pub unit: String,
    pub contract_unit_price_cents: i64,
    pub original_quantity_millis: i64,
    pub settlement_quantity_millis: i64,
    pub remarks: String,
}

impl PaymentSettlementItem {
    pub fn original_amount_cents(&self) -> Result<i64, HostError> {
        calculate_line_amount(
            self.contract_unit_price_cents,
            self.original_quantity_millis,
        )
    }

    pub fn settlement_amount_cents(&self) -> Result<i64, HostError> {
        calculate_line_amount(
            self.contract_unit_price_cents,
            self.settlement_quantity_millis,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentApplicationTemplateData {
    pub customer_legal_name: String,
    pub project_title: String,
    pub contract_title: String,
    pub contract_number: String,
    pub supplier_legal_name: String,
    pub work_summary: String,
    pub payment_period_start: String,
    pub payment_period_end: String,
    pub settlement_period: String,
    pub payment_sequence: u32,
    pub invoice_amount_cents: i64,
    pub cumulative_recognized_amount_cents: i64,
    pub payable_amount_cents: i64,
    pub withheld_amount_cents: i64,
    pub cumulative_paid_cents: i64,
    pub application_date: String,
    pub bank_account: PaymentBankAccount,
    pub settlement_items: Vec<PaymentSettlementItem>,
}

impl PaymentApplicationTemplateData {
    pub fn settlement_total_cents(&self) -> Result<i64, HostError> {
        self.settlement_items.iter().try_fold(0_i64, |total, item| {
            total
                .checked_add(item.settlement_amount_cents()?)
                .ok_or_else(|| template_error("结算总金额计算溢出"))
        })
    }

    pub fn remaining_payable_cents(&self) -> Result<i64, HostError> {
        self.settlement_total_cents()?
            .checked_sub(self.cumulative_paid_cents)
            .ok_or_else(|| template_error("剩余应付金额计算溢出"))
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyChildKind {
    Paragraph,
    Table,
    Section,
    Other,
}

#[derive(Debug, Clone)]
struct BodyChild {
    kind: BodyChildKind,
    events: Range<usize>,
}

struct SourceTemplateStructure {
    account: TableRange,
    settlement: TableRange,
    business_paragraphs: Vec<BodyChild>,
    between_tables: Vec<BodyChild>,
}

pub(crate) fn validate_payment_application_template_source_from_bytes(
    source: &[u8],
    expected_sha256: &str,
) -> Result<(), HostError> {
    validate_sha256(expected_sha256)?;
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(template_error("付款申请模板超过大小限制"));
    }
    if !sha256_bytes(source).eq_ignore_ascii_case(expected_sha256) {
        return Err(template_error("付款申请模板 SHA-256 与登记版本不匹配"));
    }
    let package = load_package(source)?;
    validate_safe_docx(&package)?;
    let events = parse_xml_events(package_entry(&package, DOCUMENT_PATH)?, DOCUMENT_PATH)?;
    analyze_source_template(&events).map(|_| ())
}

pub(crate) fn render_payment_application_template(
    source: &Path,
    expected_sha256: &str,
    destination: &Path,
    data: &PaymentApplicationTemplateData,
) -> Result<(), HostError> {
    validate_source_and_destination(source, destination)?;
    let source_bytes = read_file_limited(source, MAX_SOURCE_BYTES)?;
    render_payment_application_template_from_bytes(
        &source_bytes,
        expected_sha256,
        destination,
        data,
    )
}

pub(crate) fn render_payment_application_template_from_bytes(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &PaymentApplicationTemplateData,
) -> Result<(), HostError> {
    validate_data(data)?;
    validate_destination(destination)?;
    validate_sha256(expected_sha256)?;
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err(template_error("付款申请模板超过大小限制"));
    }
    let actual_sha256 = sha256_bytes(source);
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(template_error("付款申请模板 SHA-256 与登记版本不匹配"));
    }

    let mut package = load_package(source)?;
    validate_safe_docx(&package)?;
    let source_document = package_entry(&package, DOCUMENT_PATH)?.to_vec();
    let (document, stale_values) = transform_document(&source_document, data)?;
    replace_package_entry(&mut package, DOCUMENT_PATH, document)?;
    sanitize_package(&mut package)?;
    assert_no_stale_business_values(&package, &stale_values, data)?;

    let output = write_package(&package)?;
    let verified = load_package(&output)?;
    validate_safe_docx(&verified)?;
    verify_rendered_document(package_entry(&verified, DOCUMENT_PATH)?, data)?;
    assert_no_stale_business_values(&verified, &stale_values, data)?;
    publish_no_replace(destination, &output)
}

fn validate_data(data: &PaymentApplicationTemplateData) -> Result<(), HostError> {
    let fields = [
        ("customerLegalName", data.customer_legal_name.as_str()),
        ("projectTitle", data.project_title.as_str()),
        ("contractTitle", data.contract_title.as_str()),
        ("contractNumber", data.contract_number.as_str()),
        ("supplierLegalName", data.supplier_legal_name.as_str()),
        ("workSummary", data.work_summary.as_str()),
        ("paymentPeriodStart", data.payment_period_start.as_str()),
        ("paymentPeriodEnd", data.payment_period_end.as_str()),
        ("settlementPeriod", data.settlement_period.as_str()),
        ("applicationDate", data.application_date.as_str()),
        (
            "bank.recipientName",
            data.bank_account.recipient_name.as_str(),
        ),
        ("bank.bankName", data.bank_account.bank_name.as_str()),
        (
            "bank.accountNumber",
            data.bank_account.account_number.as_str(),
        ),
        (
            "bank.routingNumber",
            data.bank_account.routing_number.as_str(),
        ),
    ];
    for (label, value) in fields {
        validate_field(label, value, true)?;
    }
    if data.payment_sequence == 0 {
        return Err(template_error("付款次数必须是正整数"));
    }
    for amount in [
        data.invoice_amount_cents,
        data.cumulative_recognized_amount_cents,
        data.payable_amount_cents,
        data.withheld_amount_cents,
        data.cumulative_paid_cents,
    ] {
        if amount < 0 {
            return Err(template_error("付款和结算金额不能为负数"));
        }
    }
    if data.settlement_items.is_empty() || data.settlement_items.len() > MAX_SETTLEMENT_ITEMS {
        return Err(template_error(format!(
            "结算行数量必须在 1 到 {MAX_SETTLEMENT_ITEMS} 之间"
        )));
    }
    for (index, item) in data.settlement_items.iter().enumerate() {
        validate_field(&format!("settlementItems[{index}].name"), &item.name, true)?;
        validate_field(&format!("settlementItems[{index}].unit"), &item.unit, true)?;
        validate_field(
            &format!("settlementItems[{index}].remarks"),
            &item.remarks,
            false,
        )?;
        if item.contract_unit_price_cents < 0
            || item.original_quantity_millis < 0
            || item.settlement_quantity_millis < 0
        {
            return Err(template_error("结算单价和数量不能为负数"));
        }
        item.original_amount_cents()?;
        item.settlement_amount_cents()?;
    }
    let remaining = data.remaining_payable_cents()?;
    if remaining < 0 {
        return Err(template_error("累计已付金额不能超过结算总金额"));
    }
    if data.payable_amount_cents != remaining {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH",
            "payableAmountCents must equal settlementTotalCents minus cumulativePaidCents",
            false,
        ));
    }
    if !data
        .bank_account
        .account_number
        .chars()
        .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-'))
        || !data
            .bank_account
            .routing_number
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-'))
    {
        return Err(template_error("银行账号和联行号只能包含数字、空格或连字符"));
    }
    Ok(())
}

fn validate_field(label: &str, value: &str, required: bool) -> Result<(), HostError> {
    if required && value.trim().is_empty() {
        return Err(template_error(format!("{label} 不能为空")));
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(template_error(format!(
            "{label} 超过 {MAX_FIELD_BYTES} 字节"
        )));
    }
    if value.chars().any(|character| {
        !matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
            && !matches!(character as u32, 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
    }) {
        return Err(template_error(format!("{label} 包含非法 XML 字符")));
    }
    Ok(())
}

fn calculate_line_amount(unit_price_cents: i64, quantity_millis: i64) -> Result<i64, HostError> {
    let product = i128::from(unit_price_cents)
        .checked_mul(i128::from(quantity_millis))
        .ok_or_else(|| template_error("结算行金额计算溢出"))?;
    if product % i128::from(QUANTITY_SCALE) != 0 {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_LINE_AMOUNT_FRACTIONAL_CENT",
            "unit price multiplied by quantity must resolve to whole cents",
            false,
        ));
    }
    i64::try_from(product / i128::from(QUANTITY_SCALE))
        .map_err(|_| template_error("结算行金额超出支持范围"))
}

fn transform_document(
    xml: &[u8],
    data: &PaymentApplicationTemplateData,
) -> Result<(Vec<u8>, HashSet<String>), HostError> {
    let mut events = parse_xml_events(xml, DOCUMENT_PATH)?;
    let structure = analyze_source_template(&events)?;
    let SourceTemplateStructure {
        account,
        settlement,
        business_paragraphs,
        between_tables,
    } = structure;

    let mut stale_values = HashSet::new();
    for paragraph in &business_paragraphs[..4] {
        collect_stale_candidates(
            &text_in_range(&events, paragraph.events.clone())?,
            &mut stale_values,
        );
    }
    for row in &account.rows[..5] {
        collect_stale_candidates(&cell_text(&events, &row.cells[0])?, &mut stale_values);
    }
    for (row, cells) in [
        (0_usize, vec![1_usize, 3]),
        (1, vec![1, 3]),
        (2, vec![1, 2, 3]),
        (5, vec![2, 3]),
        (6, (0..settlement.rows[6].cells.len()).collect()),
        (7, vec![2, 3]),
        (8, vec![2, 3]),
    ] {
        for cell in cells {
            collect_stale_candidates(
                &cell_text(&events, &settlement.rows[row].cells[cell])?,
                &mut stale_values,
            );
        }
    }

    let paragraph_values = payment_paragraph_values(data);
    let account_values = account_table_values(data)?;
    let transformed_account = transform_account_table(&events, &account, &account_values)?;
    let transformed_settlement = transform_settlement_table(&events, &settlement, data)?;

    let mut replacements = vec![
        (account.events.clone(), transformed_account),
        (settlement.events.clone(), transformed_settlement),
        (
            between_tables[1].events.clone(),
            replace_paragraph_text(&events, &between_tables[1].events, &data.application_date)?,
        ),
    ];
    for (paragraph, value) in business_paragraphs.iter().zip(
        paragraph_values
            .iter()
            .map(String::as_str)
            .chain(std::iter::once("")),
    ) {
        replacements.push((
            paragraph.events.clone(),
            replace_paragraph_text(&events, &paragraph.events, value)?,
        ));
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in replacements {
        events.splice(range, replacement);
    }
    Ok((write_xml_events(events)?, stale_values))
}

fn analyze_source_template(
    events: &[Event<'static>],
) -> Result<SourceTemplateStructure, HostError> {
    let children = collect_body_children(events)?;
    let title = unique_paragraph(events, &children, "付款申请书")?;
    let account_table = first_table_after(&children, title.events.end)?;
    let settlement_title = unique_paragraph(events, &children, "合同结算计算表")?;
    let settlement_table = first_table_after(&children, settlement_title.events.end)?;
    validate_page_order(&account_table, &settlement_title, &settlement_table)?;
    assert_page_break(events, &settlement_title.events)?;

    let business_paragraphs =
        paragraphs_between(&children, title.events.end, account_table.events.start);
    if business_paragraphs.len() != 5 {
        return Err(template_error(
            "付款申请正文必须包含四个业务段落和一个留白段落",
        ));
    }
    let between_tables = paragraphs_between(
        &children,
        account_table.events.end,
        settlement_title.events.start,
    );
    if between_tables.len() != 2 {
        return Err(template_error("付款申请盖章和日期段落结构无效"));
    }

    let account = analyze_table(events, account_table.events.clone())?;
    let settlement = analyze_table(events, settlement_table.events.clone())?;
    validate_account_table(&account)?;
    validate_settlement_table(events, &settlement)?;
    Ok(SourceTemplateStructure {
        account,
        settlement,
        business_paragraphs,
        between_tables,
    })
}

fn payment_paragraph_values(data: &PaymentApplicationTemplateData) -> Vec<String> {
    vec![
        format!("致：{}", data.customer_legal_name),
        format!(
            "我司已按《{}》完成{}。现申请支付{}至{}期间第{}次款项。",
            data.contract_title,
            data.work_summary,
            data.payment_period_start,
            data.payment_period_end,
            data.payment_sequence
        ),
        format!(
            "本期开票金额：{}；累计产值：{}。",
            format_money(data.invoice_amount_cents),
            format_money(data.cumulative_recognized_amount_cents)
        ),
        format!(
            "本期应付金额：{}；代缴金额：{}。",
            format_money(data.payable_amount_cents),
            format_money(data.withheld_amount_cents)
        ),
    ]
}

fn account_table_values(data: &PaymentApplicationTemplateData) -> Result<Vec<String>, HostError> {
    Ok(vec![
        format!(
            "本期应付金额（大写）：{}；小写：{}",
            chinese_upper_money(data.payable_amount_cents)?,
            format_money(data.payable_amount_cents)
        ),
        format!("收款单位：{}", data.bank_account.recipient_name),
        format!("开户行：{}", data.bank_account.bank_name),
        format!("银行账号：{}", data.bank_account.account_number),
        format!("联行号：{}", data.bank_account.routing_number),
        String::new(),
        String::new(),
    ])
}

fn validate_account_table(table: &TableRange) -> Result<(), HostError> {
    if table.rows.len() != 7 || table.rows.iter().any(|row| row.cells.len() != 1) {
        return Err(template_error("付款申请收款账户表必须是 7 行单单元格结构"));
    }
    Ok(())
}

fn validate_settlement_table(
    events: &[Event<'static>],
    table: &TableRange,
) -> Result<(), HostError> {
    if table.grid_columns != 12 || table.rows.len() != 11 {
        return Err(template_error("合同结算计算表必须是 11 行、12 逻辑列"));
    }
    let expected_cells = [4, 4, 4, 7, 9, 4, 9, 4, 4, 2, 2];
    if table
        .rows
        .iter()
        .zip(expected_cells)
        .any(|(row, expected)| row.cells.len() != expected)
    {
        return Err(template_error("合同结算计算表合并单元格结构不匹配"));
    }
    let required = [
        "项目名称",
        "合同名称",
        "合同编号",
        "施工单位",
        "合同单价",
        "结算金额",
        "累计已付",
        "剩余应付",
        "经办人",
        "专业经理",
    ];
    let text = text_in_range(events, table.events.clone())?;
    if required.iter().any(|label| !text.contains(label)) {
        return Err(template_error("合同结算计算表缺少必要语义标签"));
    }
    Ok(())
}

fn transform_account_table(
    events: &[Event<'static>],
    table: &TableRange,
    values: &[String],
) -> Result<Vec<Event<'static>>, HostError> {
    let mut output = events[table.events.clone()].to_vec();
    let local = analyze_table(&output, 0..output.len())?;
    let mut replacements = local
        .rows
        .iter()
        .zip(values)
        .map(|(row, value)| (row.cells[0].events.clone(), value.as_str()))
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, value) in replacements {
        output.splice(range.clone(), replace_cell_text(&output, &range, value)?);
    }
    Ok(output)
}

fn transform_settlement_table(
    events: &[Event<'static>],
    table: &TableRange,
    data: &PaymentApplicationTemplateData,
) -> Result<Vec<Event<'static>>, HostError> {
    let mut output = events[table.events.clone()].to_vec();
    let local = analyze_table(&output, 0..output.len())?;
    let template_row = output[local.rows[6].events.clone()].to_vec();
    let rendered_rows = data
        .settlement_items
        .iter()
        .enumerate()
        .map(|(index, item)| render_settlement_item_row(&template_row, index, item))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    output.splice(local.rows[6].events.clone(), rendered_rows);

    let table = analyze_table(&output, 0..output.len())?;
    let item_count = data.settlement_items.len();
    let cumulative_row = 6 + item_count;
    let remaining_row = cumulative_row + 1;
    let total = data.settlement_total_cents()?;
    let mut replacements = vec![
        (
            table.rows[0].cells[1].events.clone(),
            data.project_title.clone(),
        ),
        (
            table.rows[0].cells[3].events.clone(),
            data.settlement_period.clone(),
        ),
        (
            table.rows[1].cells[1].events.clone(),
            data.contract_title.clone(),
        ),
        (
            table.rows[1].cells[3].events.clone(),
            data.contract_number.clone(),
        ),
        (
            table.rows[2].cells[1].events.clone(),
            data.supplier_legal_name.clone(),
        ),
        (table.rows[2].cells[2].events.clone(), String::new()),
        (table.rows[2].cells[3].events.clone(), String::new()),
        (table.rows[5].cells[2].events.clone(), format_money(total)),
        (table.rows[5].cells[3].events.clone(), String::new()),
        (
            table.rows[cumulative_row].cells[2].events.clone(),
            format_money(data.cumulative_paid_cents),
        ),
        (
            table.rows[cumulative_row].cells[3].events.clone(),
            String::new(),
        ),
        (
            table.rows[remaining_row].cells[2].events.clone(),
            format_money(data.remaining_payable_cents()?),
        ),
        (
            table.rows[remaining_row].cells[3].events.clone(),
            String::new(),
        ),
    ];
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, value) in replacements {
        output.splice(range.clone(), replace_cell_text(&output, &range, &value)?);
    }
    Ok(output)
}

fn render_settlement_item_row(
    template_row: &[Event<'static>],
    index: usize,
    item: &PaymentSettlementItem,
) -> Result<Vec<Event<'static>>, HostError> {
    let mut row = template_row.to_vec();
    let analyzed = analyze_row(&row, 0..row.len())?;
    let values = [
        (index + 1).to_string(),
        item.name.clone(),
        item.unit.clone(),
        format_money(item.contract_unit_price_cents),
        format_quantity(item.original_quantity_millis),
        format_money(item.original_amount_cents()?),
        format_quantity(item.settlement_quantity_millis),
        format_money(item.settlement_amount_cents()?),
        item.remarks.clone(),
    ];
    let mut replacements = analyzed
        .cells
        .iter()
        .zip(values)
        .map(|(cell, value)| (cell.events.clone(), value))
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, value) in replacements {
        row.splice(range.clone(), replace_cell_text(&row, &range, &value)?);
    }
    strip_paragraph_ids(row)
}

fn verify_rendered_document(
    xml: &[u8],
    data: &PaymentApplicationTemplateData,
) -> Result<(), HostError> {
    let events = parse_xml_events(xml, DOCUMENT_PATH)?;
    let children = collect_body_children(&events)?;
    let title = unique_paragraph(&events, &children, "付款申请书")?;
    let account = analyze_table(
        &events,
        first_table_after(&children, title.events.end)?.events,
    )?;
    let settlement_title = unique_paragraph(&events, &children, "合同结算计算表")?;
    assert_page_break(&events, &settlement_title.events)?;
    let settlement = analyze_table(
        &events,
        first_table_after(&children, settlement_title.events.end)?.events,
    )?;
    validate_account_table(&account)?;
    if settlement.grid_columns != 12 || settlement.rows.len() != 10 + data.settlement_items.len() {
        return Err(template_error("渲染后的合同结算行数或逻辑列数无效"));
    }
    let account_values = account_table_values(data)?;
    for (row, expected) in account.rows.iter().zip(account_values) {
        if cell_text(&events, &row.cells[0])? != expected {
            return Err(template_error("渲染后的收款账户区域校验失败"));
        }
    }
    let text = text_in_range(&events, 0..events.len())?;
    for expected in expected_business_values(data)? {
        if !expected.is_empty() && !compact(&text).contains(&compact(&expected)) {
            return Err(template_error("渲染后的付款或结算字段校验失败"));
        }
    }
    if data.remaining_payable_cents()?
        != data
            .settlement_total_cents()?
            .checked_sub(data.cumulative_paid_cents)
            .ok_or_else(|| template_error("剩余应付校验溢出"))?
    {
        return Err(template_error("剩余应付金额公式校验失败"));
    }
    Ok(())
}

fn assert_no_stale_business_values(
    package: &Package,
    stale_values: &HashSet<String>,
    data: &PaymentApplicationTemplateData,
) -> Result<(), HostError> {
    let expected = expected_business_values(data)?
        .into_iter()
        .map(|value| compact(&value))
        .collect::<HashSet<_>>();
    let parts = package_text_contents(package)?;
    let text = compact(
        &parts
            .iter()
            .map(|part| part.searchable.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let stale = stale_values.iter().any(|candidate| {
        !candidate.is_empty()
            && text.contains(candidate)
            && !expected.iter().any(|value| value.contains(candidate))
    });
    if stale || contains_unresolved_placeholder(&text) {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_TEMPLATE_STALE_VALUE",
            "generated payment application contains a stale example value or unresolved placeholder",
            false,
        ));
    }

    let allowed_long_numbers = expected
        .iter()
        .flat_map(|value| digit_runs(value, 8))
        .collect::<HashSet<_>>();
    let value_text = compact(
        &parts
            .iter()
            .map(|part| part.values.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    if digit_runs(&value_text, 8)
        .iter()
        .any(|value| !allowed_long_numbers.contains(value))
    {
        return Err(HostError::new(
            "BUSINESS_PAYMENT_TEMPLATE_STALE_VALUE",
            "generated payment application contains an unapproved long numeric value",
            false,
        ));
    }
    Ok(())
}

struct TextualPartContents {
    searchable: String,
    values: String,
}

fn package_text_contents(package: &Package) -> Result<Vec<TextualPartContents>, HostError> {
    package
        .entries
        .iter()
        .filter(|entry| !entry.is_dir)
        .filter_map(|entry| {
            textual_part_kind(&entry.name).map(|kind| decode_textual_part(entry, kind))
        })
        .collect()
}

#[derive(Clone, Copy)]
enum TextualPartKind {
    Xml,
    PlainText,
}

fn textual_part_kind(name: &str) -> Option<TextualPartKind> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".xml") || lower.ends_with(".rels") {
        return Some(TextualPartKind::Xml);
    }
    if [
        ".txt", ".csv", ".tsv", ".json", ".md", ".yaml", ".yml", ".html", ".htm", ".xhtml",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension))
    {
        return Some(TextualPartKind::PlainText);
    }
    None
}

fn decode_textual_part(
    entry: &PackageEntry,
    kind: TextualPartKind,
) -> Result<TextualPartContents, HostError> {
    let decoded = std::str::from_utf8(&entry.contents)
        .map_err(|_| template_error(format!("DOCX 文本部件 {} 不是有效 UTF-8", entry.name)))?;
    match kind {
        TextualPartKind::PlainText => Ok(TextualPartContents {
            searchable: decoded.to_owned(),
            values: decoded.to_owned(),
        }),
        TextualPartKind::Xml => extract_xml_search_text(&entry.contents, &entry.name),
    }
}

fn extract_xml_search_text(xml: &[u8], label: &str) -> Result<TextualPartContents, HostError> {
    let mut searchable = std::str::from_utf8(xml)
        .map_err(|_| template_error(format!("DOCX XML 部件 {label} 不是有效 UTF-8")))?
        .to_owned();
    let mut values = String::new();
    for event in parse_xml_events(xml, label)? {
        match event {
            Event::Start(start) | Event::Empty(start) => {
                for attribute in start.attributes().with_checks(true) {
                    let attribute = attribute
                        .map_err(|_| template_error(format!("DOCX XML 部件 {label} 属性无效")))?;
                    searchable.push('\n');
                    searchable.push_str(&attribute.unescape_value().map_err(|_| {
                        template_error(format!("DOCX XML 部件 {label} 属性值无法解码"))
                    })?);
                }
            }
            Event::Text(text) => {
                let text = decode_text(&text)?;
                searchable.push('\n');
                searchable.push_str(&text);
                values.push('\n');
                values.push_str(&text);
            }
            Event::CData(text) => {
                let text = text
                    .decode()
                    .map_err(|_| template_error(format!("DOCX XML 部件 {label} 文本无法解码")))?
                    .into_owned();
                searchable.push('\n');
                searchable.push_str(&text);
                values.push('\n');
                values.push_str(&text);
            }
            Event::Comment(text) => {
                let text = text
                    .decode()
                    .map_err(|_| template_error(format!("DOCX XML 部件 {label} 注释无法解码")))?
                    .into_owned();
                searchable.push('\n');
                searchable.push_str(&text);
                values.push('\n');
                values.push_str(&text);
            }
            _ => {}
        }
    }
    Ok(TextualPartContents { searchable, values })
}

fn expected_business_values(
    data: &PaymentApplicationTemplateData,
) -> Result<Vec<String>, HostError> {
    let mut values = payment_paragraph_values(data);
    values.extend(account_table_values(data)?);
    values.extend([
        data.customer_legal_name.clone(),
        data.project_title.clone(),
        data.contract_title.clone(),
        data.contract_number.clone(),
        data.supplier_legal_name.clone(),
        data.settlement_period.clone(),
        data.application_date.clone(),
        format_money(data.settlement_total_cents()?),
        format_money(data.cumulative_paid_cents),
        format_money(data.remaining_payable_cents()?),
    ]);
    for item in &data.settlement_items {
        values.extend([
            item.name.clone(),
            item.unit.clone(),
            item.remarks.clone(),
            format_money(item.contract_unit_price_cents),
            format_quantity(item.original_quantity_millis),
            format_money(item.original_amount_cents()?),
            format_quantity(item.settlement_quantity_millis),
            format_money(item.settlement_amount_cents()?),
        ]);
    }
    Ok(values)
}

fn collect_stale_candidates(value: &str, output: &mut HashSet<String>) {
    let value = compact(value);
    if value.chars().count() >= 4 {
        output.insert(value.clone());
    }
    for separator in ['：', ':'] {
        if let Some((_, suffix)) = value.rsplit_once(separator) {
            if suffix.chars().count() >= 4 {
                output.insert(suffix.to_owned());
            }
        }
    }
    output.extend(digit_runs(&value, 4));
}

fn contains_unresolved_placeholder(value: &str) -> bool {
    ["{{", "}}", "${", "<<", ">>"]
        .iter()
        .any(|marker| value.contains(marker))
}

fn digit_runs(value: &str, minimum: usize) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    for character in value.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() {
            current.push(character);
        } else {
            if current.len() >= minimum {
                output.push(std::mem::take(&mut current));
            }
            current.clear();
        }
    }
    output
}

fn format_money(cents: i64) -> String {
    let yuan = cents / 100;
    let fraction = cents % 100;
    let digits = yuan.to_string();
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(character);
    }
    format!("{grouped}.{fraction:02}")
}

fn format_quantity(millis: i64) -> String {
    let whole = millis / QUANTITY_SCALE;
    let fraction = millis % QUANTITY_SCALE;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn chinese_upper_money(cents: i64) -> Result<String, HostError> {
    if cents < 0 {
        return Err(template_error("人民币大写金额不能为负数"));
    }
    let yuan = cents / 100;
    let jiao = (cents / 10) % 10;
    let fen = cents % 10;
    let mut output = if yuan == 0 {
        "零元".to_owned()
    } else {
        format!("{}元", chinese_upper_integer(yuan)?)
    };
    const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    if jiao == 0 && fen == 0 {
        output.push('整');
    } else {
        if jiao > 0 {
            output.push_str(DIGITS[jiao as usize]);
            output.push('角');
        } else if yuan > 0 && fen > 0 {
            output.push('零');
        }
        if fen > 0 {
            output.push_str(DIGITS[fen as usize]);
            output.push('分');
        }
    }
    Ok(output)
}

fn chinese_upper_integer(mut value: i64) -> Result<String, HostError> {
    if value <= 0 {
        return Ok("零".to_owned());
    }
    const GROUP_UNITS: [&str; 4] = ["", "万", "亿", "兆"];
    let mut groups = Vec::new();
    while value > 0 {
        groups.push((value % 10_000) as u16);
        value /= 10_000;
    }
    if groups.len() > GROUP_UNITS.len() {
        return Err(template_error("人民币大写金额超出支持范围"));
    }
    let mut output = String::new();
    let mut pending_zero = false;
    for index in (0..groups.len()).rev() {
        let group = groups[index];
        if group == 0 {
            if !output.is_empty() {
                pending_zero = true;
            }
            continue;
        }
        if !output.is_empty() && (pending_zero || group < 1_000) {
            output.push('零');
        }
        output.push_str(&chinese_upper_group(group));
        output.push_str(GROUP_UNITS[index]);
        pending_zero = false;
    }
    Ok(output)
}

fn chinese_upper_group(value: u16) -> String {
    const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    const UNITS: [&str; 4] = ["仟", "佰", "拾", ""];
    let divisors = [1_000_u16, 100, 10, 1];
    let mut output = String::new();
    let mut zero = false;
    for (index, divisor) in divisors.into_iter().enumerate() {
        let digit = (value / divisor) % 10;
        if digit == 0 {
            zero |= !output.is_empty() && !value.is_multiple_of(divisor);
        } else {
            if zero {
                output.push('零');
                zero = false;
            }
            output.push_str(DIGITS[digit as usize]);
            output.push_str(UNITS[index]);
        }
    }
    output
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn validate_source_and_destination(source: &Path, destination: &Path) -> Result<(), HostError> {
    let source = fs::canonicalize(source).map_err(io_error("读取付款申请模板"))?;
    if !source.is_file() {
        return Err(template_error("付款申请模板不是文件"));
    }
    let destination = absolute_destination(destination)?;
    if paths_equal(&source, &destination) {
        return Err(template_error("付款申请模板与输出路径不能相同"));
    }
    validate_destination(&destination)
}

fn validate_destination(destination: &Path) -> Result<(), HostError> {
    let destination = absolute_destination(destination)?;
    if destination.exists() {
        return Err(template_error("付款申请输出文件已存在"));
    }
    if destination
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("docx"))
    {
        return Err(template_error("付款申请输出必须使用 .docx 扩展名"));
    }
    Ok(())
}

fn absolute_destination(destination: &Path) -> Result<PathBuf, HostError> {
    let parent = destination
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(io_error("读取付款申请输出目录"))?;
    let name = destination
        .file_name()
        .ok_or_else(|| template_error("付款申请输出路径无效"))?;
    Ok(parent.join(name))
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

fn validate_sha256(value: &str) -> Result<(), HostError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(template_error("付款申请模板 SHA-256 必须是 64 位十六进制"));
    }
    Ok(())
}

fn read_file_limited(path: &Path, limit: u64) -> Result<Vec<u8>, HostError> {
    let file = File::open(path).map_err(io_error("打开付款申请模板"))?;
    let mut output = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut output)
        .map_err(io_error("读取付款申请模板"))?;
    if output.len() as u64 > limit {
        return Err(template_error("付款申请模板超过大小限制"));
    }
    Ok(output)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn load_package(source: &[u8]) -> Result<Package, HostError> {
    let mut archive = ZipArchive::new(Cursor::new(source))
        .map_err(|_| template_error("付款申请 DOCX 不是有效 ZIP 包"))?;
    if archive.is_empty() || archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(template_error("付款申请 DOCX 包条目数无效"));
    }
    let mut entries = Vec::with_capacity(archive.len());
    let mut index_by_name = HashMap::new();
    let mut folded = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| template_error("无法读取付款申请 DOCX 条目"))?;
        let name = entry.name().to_owned();
        validate_zip_entry_name(&name)?;
        if entry.encrypted() || entry.is_symlink() {
            return Err(template_error("付款申请 DOCX 包含加密或链接条目"));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(template_error("付款申请 DOCX 使用不支持的压缩算法"));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(template_error("付款申请 DOCX 包含超大条目"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| template_error("付款申请 DOCX 解压大小溢出"))?;
        if total > MAX_PACKAGE_BYTES {
            return Err(template_error("付款申请 DOCX 解压后超过大小限制"));
        }
        if index_by_name.contains_key(&name) || !folded.insert(name.to_ascii_lowercase()) {
            return Err(template_error("付款申请 DOCX 包含重复条目"));
        }
        let mut contents = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut contents)
            .map_err(|_| template_error("无法解压付款申请 DOCX 条目"))?;
        index_by_name.insert(name.clone(), entries.len());
        entries.push(PackageEntry {
            name,
            options: entry.options(),
            is_dir: entry.is_dir(),
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
        || has_windows_drive_prefix(name)
        || name
            .trim_end_matches('/')
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(template_error("付款申请 DOCX 包含不安全路径"));
    }
    Ok(())
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn validate_safe_docx(package: &Package) -> Result<(), HostError> {
    for required in [CONTENT_TYPES_PATH, ROOT_RELATIONSHIPS_PATH, DOCUMENT_PATH] {
        if !package.index_by_name.contains_key(required) {
            return Err(template_error("付款申请 DOCX 缺少标准包条目"));
        }
    }
    for entry in &package.entries {
        let lower = entry.name.to_ascii_lowercase();
        if lower.ends_with("vbaproject.bin")
            || lower.contains("/macros/")
            || lower.contains("/activex/")
            || lower.contains("/embeddings/")
            || lower.contains("oleobject")
        {
            return Err(template_error("付款申请 DOCX 包含宏、ActiveX 或 OLE 内容"));
        }
        if entry.name.ends_with(".rels") {
            validate_relationships(&entry.contents)?;
        }
        if entry.name.starts_with("word/") && entry.name.ends_with(".xml") {
            validate_word_xml(&entry.contents)?;
        }
    }
    validate_content_types(package_entry(package, CONTENT_TYPES_PATH)?)
}

fn validate_content_types(xml: &[u8]) -> Result<(), HostError> {
    let events = parse_xml_events(xml, CONTENT_TYPES_PATH)?;
    let mut standard_document = false;
    for event in events {
        let start = match event {
            Event::Start(start) | Event::Empty(start) => start,
            _ => continue,
        };
        if !matches!(local_name(&start), b"Default" | b"Override") {
            continue;
        }
        let content_type = attribute_value(&start, b"ContentType")?.unwrap_or_default();
        let lower = content_type.to_ascii_lowercase();
        if lower.contains("macroenabled")
            || lower.contains("vbaproject")
            || lower.contains("activex")
            || lower.contains("oleobject")
        {
            return Err(template_error("付款申请 DOCX 内容类型包含活动内容"));
        }
        if local_name(&start) == b"Override"
            && attribute_value(&start, b"PartName")?.as_deref() == Some("/word/document.xml")
        {
            standard_document = content_type
                == "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
        }
    }
    if !standard_document {
        return Err(template_error("付款申请 DOCX 主文档类型不是标准无宏 DOCX"));
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
        let target = attribute_value(&start, b"Target")?.unwrap_or_default();
        let mode = attribute_value(&start, b"TargetMode")?.unwrap_or_default();
        let kind = attribute_value(&start, b"Type")?
            .unwrap_or_default()
            .to_ascii_lowercase();
        if mode.eq_ignore_ascii_case("external") || relationship_target_is_absolute(&target) {
            return Err(template_error("付款申请 DOCX 包含外部关系"));
        }
        if kind.contains("macro")
            || kind.contains("activex")
            || kind.ends_with("/oleobject")
            || kind.ends_with("/package")
        {
            return Err(template_error("付款申请 DOCX 关系包含活动内容"));
        }
    }
    Ok(())
}

fn relationship_target_is_absolute(target: &str) -> bool {
    let target = target.trim();
    if target.starts_with('/') || target.starts_with('\\') || has_windows_drive_prefix(target) {
        return true;
    }
    target
        .as_bytes()
        .iter()
        .position(|byte| *byte == b':')
        .is_some_and(|colon| {
            colon > 0
                && target.as_bytes()[..colon]
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        })
}

fn validate_word_xml(xml: &[u8]) -> Result<(), HostError> {
    for event in parse_xml_events(xml, "Word XML")? {
        let start = match event {
            Event::Start(start) | Event::Empty(start) => start,
            _ => continue,
        };
        if matches!(
            local_name(&start),
            b"altChunk" | b"object" | b"oleObject" | b"control"
        ) {
            return Err(template_error(
                "付款申请 DOCX 包含 altChunk、OLE 或 ActiveX 标记",
            ));
        }
    }
    Ok(())
}

fn package_entry<'a>(package: &'a Package, name: &str) -> Result<&'a [u8], HostError> {
    package
        .index_by_name
        .get(name)
        .map(|index| package.entries[*index].contents.as_slice())
        .ok_or_else(|| template_error("付款申请 DOCX 缺少所需条目"))
}

fn replace_package_entry(
    package: &mut Package,
    name: &str,
    contents: Vec<u8>,
) -> Result<(), HostError> {
    let index = *package
        .index_by_name
        .get(name)
        .ok_or_else(|| template_error("付款申请 DOCX 缺少待替换条目"))?;
    package.entries[index].contents = contents;
    Ok(())
}

fn sanitize_package(package: &mut Package) -> Result<(), HostError> {
    if package.index_by_name.contains_key(CORE_PROPERTIES_PATH) {
        let value = remove_named_elements(
            package_entry(package, CORE_PROPERTIES_PATH)?,
            &[b"creator", b"lastModifiedBy"],
        )?;
        replace_package_entry(package, CORE_PROPERTIES_PATH, value)?;
    }
    if package.index_by_name.contains_key(SETTINGS_PATH) {
        let value = remove_named_elements(
            package_entry(package, SETTINGS_PATH)?,
            &[b"attachedTemplate"],
        )?;
        replace_package_entry(package, SETTINGS_PATH, value)?;
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
                    .map_err(zip_error("创建 DOCX 目录条目"))?;
            } else {
                writer
                    .start_file(&entry.name, entry.options)
                    .map_err(zip_error("创建 DOCX 文件条目"))?;
                writer
                    .write_all(&entry.contents)
                    .map_err(io_error("写入 DOCX 条目"))?;
            }
        }
        writer.finish().map_err(zip_error("完成 DOCX 包"))?;
    }
    Ok(output.into_inner())
}

fn publish_no_replace(destination: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let destination = absolute_destination(destination)?;
    let parent = destination
        .parent()
        .expect("absolute destination has parent");
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| template_error("付款申请输出文件名无效"))?;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| template_error("系统时间无效"))?
        .as_nanos();
    let mut staged_path = None;
    let mut staged_file = None;
    for _ in 0..32 {
        let counter = STAGED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            epoch,
            counter
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                staged_path = Some(candidate);
                staged_file = Some(file);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error("创建付款申请临时输出")(error)),
        }
    }
    let staged_path = staged_path.ok_or_else(|| template_error("无法分配付款申请临时输出"))?;
    let result = (|| {
        let mut file = staged_file.expect("staged file exists");
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(io_error("写入付款申请临时输出"))?;
        fs::hard_link(&staged_path, &destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                template_error("付款申请输出文件已存在")
            } else {
                HostError::internal(format!("原子发布付款申请失败: {error}"))
            }
        })?;
        Ok(())
    })();
    let cleanup = fs::remove_file(&staged_path);
    if result.is_ok() {
        cleanup.map_err(io_error("清理付款申请临时输出"))?;
    }
    result
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
            Ok(Event::Eof) => return Err(template_error(format!("{label} XML 结构无效"))),
            Ok(Event::DocType(_)) => return Err(template_error(format!("{label} 不允许 DTD"))),
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
                    .ok_or_else(|| template_error(format!("{label} XML 结构无效")))?;
                events.push(Event::End(end.into_owned()));
            }
            Ok(event) => events.push(event.into_owned()),
            Err(_) => return Err(template_error(format!("{label} XML 格式无效"))),
        }
        if roots > 1 {
            return Err(template_error(format!("{label} XML 包含多个根节点")));
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
            .map_err(|_| template_error("写入付款申请 XML 失败"))?;
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
        let attribute = attribute.map_err(|_| template_error("DOCX XML 属性无效"))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .unescape_value()
                .map(|value| Some(value.into_owned()))
                .map_err(|_| template_error("DOCX XML 属性值无效"));
        }
    }
    Ok(None)
}

fn collect_body_children(events: &[Event<'static>]) -> Result<Vec<BodyChild>, HostError> {
    let mut output = Vec::new();
    let mut inside_body = false;
    let mut depth = 0_usize;
    let mut current: Option<(usize, BodyChildKind)> = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            Event::Start(start) if local_name(start) == b"body" && !inside_body => {
                inside_body = true;
            }
            Event::Start(start) if inside_body => {
                if depth == 0 {
                    current = Some((index, body_child_kind(local_name(start))));
                }
                depth += 1;
            }
            Event::Empty(start) if inside_body && depth == 0 => output.push(BodyChild {
                kind: body_child_kind(local_name(start)),
                events: index..index + 1,
            }),
            Event::End(end)
                if inside_body && end.local_name().as_ref() == b"body" && depth == 0 =>
            {
                inside_body = false;
            }
            Event::End(_) if inside_body => {
                if depth == 0 {
                    return Err(template_error("DOCX body 结构无效"));
                }
                depth -= 1;
                if depth == 0 {
                    let (start, kind) = current
                        .take()
                        .ok_or_else(|| template_error("DOCX body 子元素结构无效"))?;
                    output.push(BodyChild {
                        kind,
                        events: start..index + 1,
                    });
                }
            }
            _ => {}
        }
    }
    if inside_body || depth != 0 || output.is_empty() {
        return Err(template_error("DOCX body 未闭合或为空"));
    }
    Ok(output)
}

fn body_child_kind(name: &[u8]) -> BodyChildKind {
    match name {
        b"p" => BodyChildKind::Paragraph,
        b"tbl" => BodyChildKind::Table,
        b"sectPr" => BodyChildKind::Section,
        _ => BodyChildKind::Other,
    }
}

fn unique_paragraph(
    events: &[Event<'static>],
    children: &[BodyChild],
    marker: &str,
) -> Result<BodyChild, HostError> {
    let matches = children
        .iter()
        .filter(|child| child.kind == BodyChildKind::Paragraph)
        .filter_map(|child| {
            text_in_range(events, child.events.clone())
                .ok()
                .filter(|text| compact(text).contains(marker))
                .map(|_| child.clone())
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(template_error(format!(
            "付款申请模板必须包含唯一的“{marker}”段落"
        )));
    }
    Ok(matches[0].clone())
}

fn first_table_after(children: &[BodyChild], index: usize) -> Result<BodyChild, HostError> {
    children
        .iter()
        .find(|child| child.kind == BodyChildKind::Table && child.events.start >= index)
        .cloned()
        .ok_or_else(|| template_error("付款申请模板缺少语义表格"))
}

fn validate_page_order(
    account_table: &BodyChild,
    settlement_title: &BodyChild,
    settlement_table: &BodyChild,
) -> Result<(), HostError> {
    if account_table.events.end > settlement_title.events.start
        || settlement_table.events.start < settlement_title.events.end
    {
        return Err(template_error("付款申请模板页面顺序无效"));
    }
    Ok(())
}

fn paragraphs_between(children: &[BodyChild], start: usize, end: usize) -> Vec<BodyChild> {
    children
        .iter()
        .filter(|child| {
            child.kind == BodyChildKind::Paragraph
                && child.events.start >= start
                && child.events.end <= end
        })
        .cloned()
        .collect()
}

fn assert_page_break(events: &[Event<'static>], range: &Range<usize>) -> Result<(), HostError> {
    for event in &events[range.clone()] {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"br" => start,
            _ => continue,
        };
        if attribute_value(start, b"type")?.as_deref() == Some("page") {
            return Ok(());
        }
    }
    Err(template_error("合同结算计算表标题必须保留显式分页"))
}

fn analyze_table(events: &[Event<'static>], table: Range<usize>) -> Result<TableRange, HostError> {
    let mut rows = Vec::new();
    let mut table_depth = 0_usize;
    let mut row_start = None;
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
                rows.push(analyze_row(
                    events,
                    row_start
                        .take()
                        .ok_or_else(|| template_error("DOCX 表格行结构无效"))?
                        ..index + 1,
                )?);
            }
            Event::Start(start) | Event::Empty(start)
                if local_name(start) == b"gridCol" && table_depth == 1 =>
            {
                grid_columns += 1
            }
            _ => {}
        }
    }
    Ok(TableRange {
        events: table,
        rows,
        grid_columns,
    })
}

fn analyze_row(events: &[Event<'static>], row: Range<usize>) -> Result<RowRange, HostError> {
    let mut cells = Vec::new();
    let mut cell_start = None;
    let mut nested_table_depth = 0_usize;
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
                let events_range = cell_start
                    .take()
                    .ok_or_else(|| template_error("DOCX 单元格结构无效"))?
                    ..index + 1;
                cells.push(CellRange {
                    logical_columns: cell_logical_columns(events, events_range.clone())?,
                    events: events_range,
                });
            }
            _ => {}
        }
    }
    Ok(RowRange { events: row, cells })
}

fn cell_logical_columns(
    events: &[Event<'static>],
    range: Range<usize>,
) -> Result<usize, HostError> {
    for event in &events[range] {
        let start = match event {
            Event::Start(start) | Event::Empty(start) if local_name(start) == b"gridSpan" => start,
            _ => continue,
        };
        return attribute_value(start, b"val")?
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| template_error("DOCX gridSpan 无效"));
    }
    Ok(1)
}

fn text_in_range(events: &[Event<'static>], range: Range<usize>) -> Result<String, HostError> {
    let mut output = String::new();
    let mut inside_text = false;
    for event in &events[range] {
        match event {
            Event::Start(start) if local_name(start) == b"t" => inside_text = true,
            Event::End(end) if end.local_name().as_ref() == b"t" => inside_text = false,
            Event::Text(text) if inside_text => output.push_str(&decode_text(text)?),
            Event::CData(text) if inside_text => output.push_str(
                &text
                    .decode()
                    .map_err(|_| template_error("DOCX CDATA 文本无效"))?,
            ),
            _ => {}
        }
    }
    Ok(output)
}

fn cell_text(events: &[Event<'static>], cell: &CellRange) -> Result<String, HostError> {
    text_in_range(events, cell.events.clone())
}

fn decode_text(text: &BytesText<'_>) -> Result<String, HostError> {
    let decoded = text
        .decode()
        .map_err(|_| template_error("DOCX 文本编码无效"))?;
    Ok(unescape(&decoded)
        .map_err(|_| template_error("DOCX 文本转义无效"))?
        .into_owned())
}

fn replace_paragraph_text(
    events: &[Event<'static>],
    paragraph: &Range<usize>,
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    replace_text_container(events, paragraph, b"p", value)
}

fn replace_cell_text(
    events: &[Event<'static>],
    cell: &Range<usize>,
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    let paragraph = find_first_element(events, cell.clone(), b"p")?
        .ok_or_else(|| template_error("DOCX 单元格缺少段落"))?;
    let replacement = replace_text_container(events, &paragraph, b"p", value)?;
    let mut output = events[cell.clone()].to_vec();
    let local_start = paragraph.start - cell.start;
    let local_end = paragraph.end - cell.start;
    output.splice(local_start..local_end, replacement);
    Ok(output)
}

fn replace_text_container(
    events: &[Event<'static>],
    range: &Range<usize>,
    expected: &[u8],
    value: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    if !matches!(&events[range.start], Event::Start(start) if local_name(start) == expected) {
        return Err(template_error("DOCX 文本容器结构无效"));
    }
    let properties_name = if expected == b"p" {
        b"pPr".as_slice()
    } else {
        b"tcPr".as_slice()
    };
    let properties = find_first_element(events, range.clone(), properties_name)?
        .filter(|properties| properties.start > range.start && properties.end < range.end);
    let insert_at = properties
        .as_ref()
        .map_or(range.start + 1, |value| value.end);
    let run_properties = find_first_element(events, range.clone(), b"rPr")?;
    let mut output = events[range.start..insert_at].to_vec();
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
    output.push(events[range.end - 1].clone());
    Ok(output)
}

fn find_first_element(
    events: &[Event<'static>],
    range: Range<usize>,
    name: &[u8],
) -> Result<Option<Range<usize>>, HostError> {
    let mut start = None;
    let mut depth = 0_usize;
    for index in range {
        match &events[index] {
            Event::Start(value) if local_name(value) == name => {
                if start.is_none() {
                    start = Some(index);
                }
                depth += 1;
            }
            Event::Empty(value) if local_name(value) == name && start.is_none() => {
                return Ok(Some(index..index + 1));
            }
            Event::End(value) if value.local_name().as_ref() == name && start.is_some() => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| template_error("DOCX XML 元素结构无效"))?;
                if depth == 0 {
                    return Ok(Some(start.expect("start exists")..index + 1));
                }
            }
            _ => {}
        }
    }
    if start.is_some() {
        return Err(template_error("DOCX XML 元素未闭合"));
    }
    Ok(None)
}

fn strip_paragraph_ids(events: Vec<Event<'static>>) -> Result<Vec<Event<'static>>, HostError> {
    events
        .into_iter()
        .map(|event| match event {
            Event::Start(start) if local_name(&start) == b"p" => {
                Ok(Event::Start(strip_ids(&start)?))
            }
            Event::Empty(start) if local_name(&start) == b"p" => {
                Ok(Event::Empty(strip_ids(&start)?))
            }
            event => Ok(event),
        })
        .collect()
}

fn strip_ids(start: &BytesStart<'_>) -> Result<BytesStart<'static>, HostError> {
    let mut output = start.clone().into_owned();
    output.clear_attributes();
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| template_error("DOCX 段落属性无效"))?;
        if !matches!(attribute.key.local_name().as_ref(), b"paraId" | b"textId") {
            output.push_attribute(attribute);
        }
    }
    Ok(output)
}

fn remove_named_elements(xml: &[u8], names: &[&[u8]]) -> Result<Vec<u8>, HostError> {
    let events = parse_xml_events(xml, "DOCX metadata")?;
    let mut output = Vec::new();
    let mut skip_depth = 0_usize;
    for event in events {
        if skip_depth > 0 {
            match event {
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
        return Err(template_error("DOCX 元数据清理结构无效"));
    }
    write_xml_events(output)
}

fn extract_word_text(xml: &[u8]) -> Result<String, HostError> {
    let events = parse_xml_events(xml, "Word XML")?;
    text_in_range(&events, 0..events.len())
}

fn template_error(message: impl Into<String>) -> HostError {
    HostError::new("BUSINESS_PAYMENT_TEMPLATE_INVALID", message, false)
}

fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> HostError {
    move |error| HostError::internal(format!("{action}失败: {error}"))
}

fn zip_error(action: &'static str) -> impl FnOnce(zip::result::ZipError) -> HostError {
    move |error| HostError::internal(format!("{action}失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> PaymentSettlementItem {
        PaymentSettlementItem {
            name: "视频制作服务".to_owned(),
            unit: "项".to_owned(),
            contract_unit_price_cents: 2_120_000,
            original_quantity_millis: 4_000,
            settlement_quantity_millis: 4_000,
            remarks: "已验收".to_owned(),
        }
    }

    fn data() -> PaymentApplicationTemplateData {
        PaymentApplicationTemplateData {
            customer_legal_name: "测试甲方".to_owned(),
            project_title: "白鹅潭测试项目".to_owned(),
            contract_title: "视频制作服务合同".to_owned(),
            contract_number: "HT-2026-001".to_owned(),
            supplier_legal_name: "测试乙方".to_owned(),
            work_summary: "约定服务".to_owned(),
            payment_period_start: "2026年1月1日".to_owned(),
            payment_period_end: "2026年3月31日".to_owned(),
            settlement_period: "2026年第一季度".to_owned(),
            payment_sequence: 1,
            invoice_amount_cents: 7_990_000,
            cumulative_recognized_amount_cents: 8_480_000,
            payable_amount_cents: 7_990_000,
            withheld_amount_cents: 0,
            cumulative_paid_cents: 490_000,
            application_date: "2026年7月29日".to_owned(),
            bank_account: PaymentBankAccount {
                recipient_name: "测试乙方".to_owned(),
                bank_name: "测试银行".to_owned(),
                account_number: "6222000000000000".to_owned(),
                routing_number: "123456789012".to_owned(),
            },
            settlement_items: vec![item()],
        }
    }

    #[test]
    fn computes_remaining_payable_from_settlement_total_minus_cumulative_paid() {
        let data = data();
        assert_eq!(data.settlement_total_cents().unwrap(), 8_480_000);
        assert_eq!(data.remaining_payable_cents().unwrap(), 7_990_000);
        validate_data(&data).unwrap();
    }

    #[test]
    fn rejects_payable_amount_that_does_not_match_formula() {
        let mut data = data();
        data.payable_amount_cents -= 1;
        let error = validate_data(&data).unwrap_err();
        assert_eq!(error.code, "BUSINESS_PAYMENT_REMAINING_PAYABLE_MISMATCH");
    }

    #[test]
    fn rejects_fractional_cent_line_amounts() {
        let mut item = item();
        item.contract_unit_price_cents = 1;
        item.settlement_quantity_millis = 1;
        let error = item.settlement_amount_cents().unwrap_err();
        assert_eq!(error.code, "BUSINESS_PAYMENT_LINE_AMOUNT_FRACTIONAL_CENT");
    }

    #[test]
    fn stale_scan_detects_unapproved_values_and_placeholders() {
        let mut stale = HashSet::new();
        collect_stale_candidates("银行账号：9999888877776666", &mut stale);
        assert!(stale.contains("9999888877776666"));
        assert!(contains_unresolved_placeholder("项目：{{projectTitle}}"));
        assert!(!contains_unresolved_placeholder("白鹅潭测试项目"));
    }

    fn package_with_text_part(name: &str, contents: &[u8]) -> Package {
        Package {
            entries: vec![PackageEntry {
                name: name.to_owned(),
                options: SimpleFileOptions::default(),
                is_dir: false,
                contents: contents.to_vec(),
            }],
            index_by_name: HashMap::from([(name.to_owned(), 0)]),
        }
    }

    fn stale_account_candidates() -> HashSet<String> {
        let mut stale = HashSet::new();
        collect_stale_candidates("银行账号：9999888877776666", &mut stale);
        stale
    }

    #[test]
    fn rejects_stale_account_hidden_in_custom_xml() {
        let package = package_with_text_part(
            "customXml/item1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><root><account>9999888877776666</account></root>"#,
        );

        let error = assert_no_stale_business_values(&package, &stale_account_candidates(), &data())
            .unwrap_err();

        assert_eq!(error.code, "BUSINESS_PAYMENT_TEMPLATE_STALE_VALUE");
    }

    #[test]
    fn rejects_placeholder_hidden_in_custom_properties() {
        let package = package_with_text_part(
            "docProps/custom.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><Properties><property name="source"><value>${bankAccount}</value></property></Properties>"#,
        );

        let error =
            assert_no_stale_business_values(&package, &HashSet::new(), &data()).unwrap_err();

        assert_eq!(error.code, "BUSINESS_PAYMENT_TEMPLATE_STALE_VALUE");
    }

    #[test]
    fn allows_harmless_custom_xml() {
        let package = package_with_text_part(
            "customXml/item1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><root purpose="workflow">approved</root>"#,
        );

        assert_no_stale_business_values(&package, &stale_account_candidates(), &data()).unwrap();
    }

    #[test]
    fn rejects_malformed_custom_xml_fail_closed() {
        let package = package_with_text_part("customXml/item1.xml", b"<root>");

        let error =
            assert_no_stale_business_values(&package, &HashSet::new(), &data()).unwrap_err();

        assert_eq!(error.code, "BUSINESS_PAYMENT_TEMPLATE_INVALID");
    }

    #[test]
    fn formats_money_quantity_and_uppercase_rmb_deterministically() {
        assert_eq!(format_money(8_480_000), "84,800.00");
        assert_eq!(format_quantity(4_250), "4.25");
        assert_eq!(chinese_upper_money(7_990_000).unwrap(), "柒万玖仟玖佰元整");
    }

    #[test]
    fn accepts_a_settlement_table_adjacent_to_its_title() {
        let account_table = BodyChild {
            kind: BodyChildKind::Table,
            events: 10..20,
        };
        let settlement_title = BodyChild {
            kind: BodyChildKind::Paragraph,
            events: 20..30,
        };
        let settlement_table = BodyChild {
            kind: BodyChildKind::Table,
            events: 30..40,
        };

        validate_page_order(&account_table, &settlement_title, &settlement_table).unwrap();
    }

    #[test]
    fn includes_paragraphs_adjacent_to_section_boundaries() {
        let children = vec![
            BodyChild {
                kind: BodyChildKind::Paragraph,
                events: 10..20,
            },
            BodyChild {
                kind: BodyChildKind::Paragraph,
                events: 20..30,
            },
            BodyChild {
                kind: BodyChildKind::Table,
                events: 30..40,
            },
        ];

        let paragraphs = paragraphs_between(&children, 10, 30);
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].events, 10..20);
        assert_eq!(paragraphs[1].events, 20..30);
    }

    #[test]
    #[ignore = "requires Microsoft Word and the operator-supplied legacy template"]
    fn renders_the_real_legacy_template_without_old_business_values() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("真实需求")
            .join("瑞玺AI请款资料")
            .join("【空白验收模版】")
            .join("付款申请书+合同结算计算表.doc");
        assert!(source.is_file(), "real legacy template is missing");
        let temporary = tempfile::tempdir().unwrap();
        let normalized = temporary.path().join("normalized.docx");
        let rendered = temporary.path().join("rendered.docx");
        let normalization = crate::business_v1::legacy_doc_normalizer::normalize_legacy_doc(
            &source,
            PAYMENT_APPLICATION_LEGACY_DOC_SHA256,
            &normalized,
        )
        .unwrap();
        let source_bytes = fs::read(&normalized).unwrap();

        render_payment_application_template_from_bytes(
            &source_bytes,
            &normalization.output_sha256,
            &rendered,
            &data(),
        )
        .unwrap();

        let output = fs::read(&rendered).unwrap();
        let package = load_package(&output).unwrap();
        validate_safe_docx(&package).unwrap();
        verify_rendered_document(package_entry(&package, DOCUMENT_PATH).unwrap(), &data()).unwrap();
        assert!(
            !extract_word_text(package_entry(&package, DOCUMENT_PATH).unwrap())
                .unwrap()
                .contains("9999888877776666")
        );
    }
}

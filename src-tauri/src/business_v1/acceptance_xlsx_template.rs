use crate::protocol::HostError;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(crate) const CONTRACT_SETTLEMENT_TEMPLATE_SHA256: &str =
    "DD0B98DD568A117F0C5EDC871CB24915CAD5971DB0E377A04732FF912F0DB9A3";

const OUTPUT_SHEET_NAME: &str = "附件1最终结算书";
const WORKBOOK_PART: &str = "xl/workbook.xml";
const WORKBOOK_RELS_PART: &str = "xl/_rels/workbook.xml.rels";
const CONTENT_TYPES_PART: &str = "[Content_Types].xml";
const CORE_PROPERTIES_PART: &str = "docProps/core.xml";
const SHARED_STRINGS_PART: &str = "xl/sharedStrings.xml";
const MAX_XLSX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 1_024;
const MAX_ZIP_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ZIP_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ZIP_COMPRESSION_RATIO: u64 = 200;
static STAGED_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractSettlementTemplateData {
    pub project_title: String,
    pub contract_title: String,
    pub contract_number: String,
    pub customer_legal_name: String,
    pub supplier_legal_name: String,
    pub original_contract_amount_cents: i64,
    pub contract_adjustment_cents: i64,
    pub retention_rate_bps: Option<u32>,
    pub final_settlement_amount_cents: i64,
    pub final_settlement_amount_uppercase_cny: String,
}

#[derive(Debug, Clone)]
struct PackageEntry {
    name: String,
    bytes: Vec<u8>,
    compression: CompressionMethod,
}

#[derive(Debug)]
struct Package {
    entries: Vec<PackageEntry>,
    index: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RichRun {
    text: String,
    font: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CellSnapshot {
    cell_type: Option<String>,
    formula: Option<String>,
    value: Option<String>,
    inline_text: String,
    rich_runs: Vec<RichRun>,
}

#[derive(Debug, Default)]
struct SheetSnapshot {
    cells: BTreeMap<String, CellSnapshot>,
    merged_ranges: BTreeSet<String>,
    hidden_rows: BTreeSet<u32>,
    row_heights: BTreeMap<u32, String>,
}

pub(crate) fn clone_contract_settlement_template(
    source: &Path,
    expected_sha256: &str,
    destination: &Path,
    data: &ContractSettlementTemplateData,
) -> Result<(), HostError> {
    validate_destination(source, destination)?;
    let source_bytes = read_file_limited(source, MAX_XLSX_SOURCE_BYTES)?;
    clone_contract_settlement_template_from_bytes(&source_bytes, expected_sha256, destination, data)
}

pub(crate) fn clone_contract_settlement_template_from_bytes(
    source: &[u8],
    expected_sha256: &str,
    destination: &Path,
    data: &ContractSettlementTemplateData,
) -> Result<(), HostError> {
    if source.len() as u64 > MAX_XLSX_SOURCE_BYTES {
        return Err(package_limit_error(format!(
            "XLSX source exceeds {MAX_XLSX_SOURCE_BYTES} bytes"
        )));
    }
    validate_output_destination(destination)?;
    validate_input(data)?;
    validate_expected_hash(expected_sha256)?;

    let source_hash = sha256_bytes(source);
    if !source_hash.eq_ignore_ascii_case(expected_sha256) {
        return Err(template_error(
            "BUSINESS_XLSX_TEMPLATE_HASH_MISMATCH",
            format!(
                "contract settlement template SHA-256 mismatch: expected {expected_sha256}, got {source_hash}"
            ),
        ));
    }

    let mut package = Package::read(Cursor::new(source))?;
    package.validate_ooxml_xlsx()?;

    let sheet_part = package.resolve_output_sheet()?;
    let drawing_part = package.resolve_sheet_drawing(&sheet_part)?;

    let sheet_xml = package.required(&sheet_part)?.to_vec();
    validate_template_h16_empty(&sheet_xml)?;
    package.replace(&sheet_part, transform_output_sheet(&sheet_xml, data)?)?;

    let drawing_xml = package.required(&drawing_part)?.to_vec();
    package.replace(&drawing_part, transform_drawing(&drawing_xml)?)?;

    let workbook_xml = package.required(WORKBOOK_PART)?.to_vec();
    package.replace(WORKBOOK_PART, sanitize_workbook(&workbook_xml)?)?;

    if package.contains(CORE_PROPERTIES_PART) {
        let core_xml = package.required(CORE_PROPERTIES_PART)?.to_vec();
        package.replace(CORE_PROPERTIES_PART, sanitize_core_properties(&core_xml)?)?;
    }

    package.sanitize_shared_strings()?;

    validate_generated_package(&package, data)?;

    let destination_parent = destination.parent().ok_or_else(|| {
        template_error(
            "BUSINESS_XLSX_DESTINATION_INVALID",
            "destination must have an existing parent directory",
        )
    })?;
    let mut staged = StagedOutput::create(destination_parent, destination)?;
    package.write_to(&mut staged.file)?;
    staged
        .file
        .sync_all()
        .map_err(io_error("flush staged XLSX output"))?;

    let staged_file = File::open(&staged.path).map_err(io_error("reopen staged XLSX output"))?;
    let staged_package = Package::read(staged_file)?;
    validate_generated_package(&staged_package, data)?;

    persist_staged_without_overwrite(&mut staged, destination, |source, destination| {
        fs::hard_link(source, destination)
    })?;
    Ok(())
}

fn persist_staged_without_overwrite<F>(
    staged: &mut StagedOutput,
    destination: &Path,
    hard_link: F,
) -> Result<(), HostError>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    match hard_link(&staged.path, destination) {
        Ok(()) => return staged.remove(),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(destination_exists_error(error));
        }
        Err(_) => {}
    }

    let mut source = File::open(&staged.path).map_err(io_error("open staged XLSX fallback"))?;
    let mut destination_file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(destination_exists_error(error));
        }
        Err(error) => return Err(io_error("create XLSX output fallback")(error)),
    };

    let publish_result = std::io::copy(&mut source, &mut destination_file)
        .map_err(io_error("copy staged XLSX output"))
        .and_then(|_| {
            destination_file
                .sync_all()
                .map_err(io_error("flush XLSX output fallback"))
        });
    if let Err(error) = publish_result {
        drop(destination_file);
        let _ = fs::remove_file(destination);
        return Err(error);
    }

    staged.remove()
}

fn destination_exists_error(error: std::io::Error) -> HostError {
    template_error(
        "BUSINESS_XLSX_DESTINATION_EXISTS",
        format!("persist XLSX output without overwrite failed: {error}"),
    )
}

#[derive(Debug)]
struct StagedOutput {
    path: PathBuf,
    file: File,
    remove_on_drop: bool,
}

impl StagedOutput {
    fn create(parent: &Path, destination: &Path) -> Result<Self, HostError> {
        let destination_name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("contract-settlement.xlsx");
        for _ in 0..128 {
            let counter = STAGED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{destination_name}.{}.{}.tmp",
                std::process::id(),
                counter
            ));
            match fs::OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error("create staged XLSX output")(error)),
            }
        }
        Err(template_error(
            "BUSINESS_XLSX_IO_FAILED",
            "could not allocate a unique staged XLSX output path",
        ))
    }

    fn remove(&mut self) -> Result<(), HostError> {
        fs::remove_file(&self.path).map_err(io_error("remove staged XLSX output"))?;
        self.remove_on_drop = false;
        Ok(())
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Package {
    fn read<R: Read + Seek>(reader: R) -> Result<Self, HostError> {
        let mut archive = ZipArchive::new(reader).map_err(zip_error("open XLSX package"))?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(package_limit_error(format!(
                "XLSX package contains {} entries; maximum is {MAX_ZIP_ENTRIES}",
                archive.len()
            )));
        }
        let mut entries = Vec::with_capacity(archive.len());
        let mut index = BTreeMap::new();
        let mut case_folded_names = BTreeSet::new();
        let mut total_uncompressed = 0_u64;

        for entry_index in 0..archive.len() {
            let entry = archive
                .by_index(entry_index)
                .map_err(zip_error("read XLSX package entry"))?;
            let name = entry.name().to_string();
            validate_zip_entry_limits(
                &name,
                entry.size(),
                entry.compressed_size(),
                &mut total_uncompressed,
            )?;
            let validation_name = if entry.is_dir() {
                name.strip_suffix('/').unwrap_or(&name)
            } else {
                &name
            };
            validate_package_path(validation_name)?;
            if !case_folded_names.insert(name.to_ascii_lowercase()) {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_UNSAFE",
                    format!("duplicate XLSX package entry: {name}"),
                ));
            }
            if entry.is_dir() {
                continue;
            }
            let compression = entry.compression();
            let declared_size = entry.size();
            let mut bytes = Vec::with_capacity(declared_size.try_into().unwrap_or(0));
            entry
                .take(MAX_ZIP_ENTRY_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error("read XLSX package entry bytes"))?;
            if bytes.len() as u64 > MAX_ZIP_ENTRY_BYTES {
                return Err(package_limit_error(format!(
                    "XLSX package entry exceeds extraction limit: {name}"
                )));
            }
            if bytes.len() as u64 != declared_size {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_INVALID",
                    format!(
                        "XLSX package entry size mismatch for {name}: declared {declared_size}, read {}",
                        bytes.len()
                    ),
                ));
            }
            let position = entries.len();
            index.insert(name.clone(), position);
            entries.push(PackageEntry {
                name,
                bytes,
                compression,
            });
        }

        Ok(Self { entries, index })
    }

    fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    fn required(&self, name: &str) -> Result<&[u8], HostError> {
        self.index
            .get(name)
            .and_then(|index| self.entries.get(*index))
            .map(|entry| entry.bytes.as_slice())
            .ok_or_else(|| {
                template_error(
                    "BUSINESS_XLSX_PACKAGE_INVALID",
                    format!("required XLSX package part is missing: {name}"),
                )
            })
    }

    fn replace(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        let index = *self.index.get(name).ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_PACKAGE_INVALID",
                format!("cannot replace missing XLSX package part: {name}"),
            )
        })?;
        self.entries[index].bytes = bytes;
        Ok(())
    }

    fn remove(&mut self, name: &str) {
        if let Some(index) = self.index.remove(name) {
            self.entries.remove(index);
            self.rebuild_index();
        }
    }

    fn rebuild_index(&mut self) {
        self.index = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.name.clone(), index))
            .collect();
    }

    fn sanitize_shared_strings(&mut self) -> Result<(), HostError> {
        let worksheet_names: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| {
                entry.name.starts_with("xl/worksheets/")
                    && entry.name.ends_with(".xml")
                    && !entry.name.contains("/_rels/")
            })
            .map(|entry| entry.name.clone())
            .collect();
        let mut references = Vec::new();
        for worksheet_name in &worksheet_names {
            references.extend(shared_string_references(self.required(worksheet_name)?)?);
        }

        let relationship_target = relationship_target_by_type(
            self.required(WORKBOOK_RELS_PART)?,
            "sharedStrings",
            WORKBOOK_PART,
        )?;
        let shared_strings_part = relationship_target.clone().or_else(|| {
            self.contains(SHARED_STRINGS_PART)
                .then(|| SHARED_STRINGS_PART.to_string())
        });

        if references.is_empty() {
            if let Some(part) = shared_strings_part.as_deref() {
                self.remove(part);
            }
            let relationships =
                strip_relationship_type(self.required(WORKBOOK_RELS_PART)?, "sharedStrings")?;
            self.replace(WORKBOOK_RELS_PART, relationships)?;
            let part_name = relationship_target
                .as_deref()
                .unwrap_or(SHARED_STRINGS_PART);
            let content_types = strip_content_type_override(
                self.required(CONTENT_TYPES_PART)?,
                &format!("/{part_name}"),
            )?;
            self.replace(CONTENT_TYPES_PART, content_types)?;
            return Ok(());
        }

        let shared_strings_part = relationship_target.ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_PACKAGE_INVALID",
                "worksheets reference shared strings but workbook relationship is missing",
            )
        })?;
        let unique_references: BTreeSet<usize> = references.iter().copied().collect();
        let remapping: BTreeMap<usize, usize> = unique_references
            .iter()
            .enumerate()
            .map(|(new_index, old_index)| (*old_index, new_index))
            .collect();

        for worksheet_name in worksheet_names {
            let source = self.required(&worksheet_name)?.to_vec();
            self.replace(
                &worksheet_name,
                remap_shared_string_references(&source, &remapping)?,
            )?;
        }
        let source = self.required(&shared_strings_part)?.to_vec();
        self.replace(
            &shared_strings_part,
            compact_shared_strings(&source, &remapping, references.len())?,
        )?;
        Ok(())
    }

    fn write_to<W: Write + Seek>(&self, writer: W) -> Result<(), HostError> {
        let mut archive = ZipWriter::new(writer);
        for entry in &self.entries {
            let options = SimpleFileOptions::default().compression_method(entry.compression);
            archive
                .start_file(&entry.name, options)
                .map_err(zip_error("create XLSX package entry"))?;
            archive
                .write_all(&entry.bytes)
                .map_err(io_error("write XLSX package entry"))?;
        }
        archive.finish().map_err(zip_error("finish XLSX package"))?;
        Ok(())
    }

    fn validate_ooxml_xlsx(&self) -> Result<(), HostError> {
        for entry in &self.entries {
            let lower_name = entry.name.to_ascii_lowercase();
            if lower_name.starts_with("xl/externallinks/")
                || lower_name.contains("vbaproject")
                || lower_name.starts_with("xl/macrosheets/")
                || lower_name.starts_with("xl/dialogsheets/")
            {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_UNSAFE",
                    format!(
                        "unsupported active or external XLSX package part: {}",
                        entry.name
                    ),
                ));
            }
            if lower_name.ends_with(".rels") {
                reject_external_relationships(&entry.bytes, &entry.name)?;
            }
        }

        let content_types = self.required(CONTENT_TYPES_PART)?;
        let mut reader = xml_reader(content_types);
        let mut standard_workbook = false;
        loop {
            match reader
                .read_event()
                .map_err(xml_error("parse XLSX content types"))?
            {
                Event::Start(start) | Event::Empty(start)
                    if matches!(local_name(start.name().as_ref()), b"Override" | b"Default") =>
                {
                    let part_name = attribute_value(&reader, &start, b"PartName")?;
                    let content_type = attribute_value(&reader, &start, b"ContentType")?;
                    if content_type
                        .as_deref()
                        .is_some_and(|value| value.to_ascii_lowercase().contains("macroenabled"))
                    {
                        return Err(template_error(
                            "BUSINESS_XLSX_PACKAGE_UNSAFE",
                            "macro-enabled workbooks are not accepted",
                        ));
                    }
                    if part_name.as_deref() == Some("/xl/workbook.xml")
                        && content_type.as_deref()
                            == Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml")
                    {
                        standard_workbook = true;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if !standard_workbook || !self.contains(WORKBOOK_PART) || !self.contains(WORKBOOK_RELS_PART)
        {
            return Err(template_error(
                "BUSINESS_XLSX_PACKAGE_INVALID",
                "source is not a standard OOXML XLSX workbook",
            ));
        }
        Ok(())
    }

    fn resolve_output_sheet(&self) -> Result<String, HostError> {
        let workbook = self.required(WORKBOOK_PART)?;
        let mut reader = xml_reader(workbook);
        let mut relationship_id = None;
        loop {
            match reader
                .read_event()
                .map_err(xml_error("parse XLSX workbook"))?
            {
                Event::Start(start) | Event::Empty(start)
                    if local_name(start.name().as_ref()) == b"sheet" =>
                {
                    if attribute_value(&reader, &start, b"name")?.as_deref()
                        == Some(OUTPUT_SHEET_NAME)
                    {
                        relationship_id = attribute_value(&reader, &start, b"r:id")?;
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let relationship_id = relationship_id.ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                format!("workbook does not contain required sheet: {OUTPUT_SHEET_NAME}"),
            )
        })?;
        resolve_relationship_target(
            self.required(WORKBOOK_RELS_PART)?,
            &relationship_id,
            "worksheet",
            WORKBOOK_PART,
        )
    }

    fn resolve_sheet_drawing(&self, sheet_part: &str) -> Result<String, HostError> {
        let sheet = self.required(sheet_part)?;
        let mut reader = xml_reader(sheet);
        let mut relationship_id = None;
        loop {
            match reader
                .read_event()
                .map_err(xml_error("parse XLSX output sheet"))?
            {
                Event::Start(start) | Event::Empty(start)
                    if local_name(start.name().as_ref()) == b"drawing" =>
                {
                    relationship_id = attribute_value(&reader, &start, b"r:id")?;
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let relationship_id = relationship_id.ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                "output sheet does not reference a drawing",
            )
        })?;
        let relationships_part = relationships_part_for(sheet_part)?;
        resolve_relationship_target(
            self.required(&relationships_part)?,
            &relationship_id,
            "drawing",
            sheet_part,
        )
    }
}

fn shared_string_references(source: &[u8]) -> Result<Vec<usize>, HostError> {
    let mut reader = xml_reader(source);
    let mut references = Vec::new();
    let mut shared_string_cell = false;
    let mut shared_string_value = false;
    loop {
        match reader
            .read_event()
            .map_err(xml_error("parse worksheet shared string references"))?
        {
            Event::Start(start) if local_name(start.name().as_ref()) == b"c" => {
                shared_string_cell =
                    attribute_value(&reader, &start, b"t")?.as_deref() == Some("s");
            }
            Event::Start(start)
                if shared_string_cell && local_name(start.name().as_ref()) == b"v" =>
            {
                shared_string_value = true;
            }
            Event::Text(text) if shared_string_value => {
                let value = text
                    .decode()
                    .map_err(xml_encoding_error("decode shared string index"))?;
                references.push(parse_shared_string_index(value.trim())?);
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"v" => {
                shared_string_value = false;
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"c" => {
                shared_string_cell = false;
                shared_string_value = false;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(references)
}

fn remap_shared_string_references(
    source: &[u8],
    remapping: &BTreeMap<usize, usize>,
) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut shared_string_cell = false;
    let mut shared_string_value = false;
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("remap worksheet shared string references"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"c" => {
                shared_string_cell =
                    attribute_value(&reader, &start, b"t")?.as_deref() == Some("s");
                writer
                    .write_event(Event::Start(start.into_owned()))
                    .map_err(io_error("write worksheet shared string cell"))?;
            }
            Event::Start(start)
                if shared_string_cell && local_name(start.name().as_ref()) == b"v" =>
            {
                shared_string_value = true;
                writer
                    .write_event(Event::Start(start.into_owned()))
                    .map_err(io_error("write worksheet shared string value"))?;
            }
            Event::Text(text) if shared_string_value => {
                let value = text
                    .decode()
                    .map_err(xml_encoding_error("decode shared string index"))?;
                let old_index = parse_shared_string_index(value.trim())?;
                let new_index = remapping.get(&old_index).ok_or_else(|| {
                    template_error(
                        "BUSINESS_XLSX_PACKAGE_INVALID",
                        format!("shared string index is not mapped: {old_index}"),
                    )
                })?;
                writer
                    .write_event(Event::Text(BytesText::new(&new_index.to_string())))
                    .map_err(io_error("write remapped shared string index"))?;
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"v" => {
                shared_string_value = false;
                writer
                    .write_event(Event::End(end.into_owned()))
                    .map_err(io_error("finish worksheet shared string value"))?;
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"c" => {
                shared_string_cell = false;
                shared_string_value = false;
                writer
                    .write_event(Event::End(end.into_owned()))
                    .map_err(io_error("finish worksheet shared string cell"))?;
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write worksheet shared string remap"))?,
        }
    }
    Ok(writer.into_inner())
}

fn compact_shared_strings(
    source: &[u8],
    remapping: &BTreeMap<usize, usize>,
    reference_count: usize,
) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut item_index = 0_usize;
    let mut root_seen = false;
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("compact XLSX shared strings"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"sst" => {
                root_seen = true;
                writer
                    .write_event(Event::Start(shared_strings_root(
                        &start,
                        reference_count,
                        remapping.len(),
                    )?))
                    .map_err(io_error("write XLSX shared strings root"))?;
            }
            Event::Start(start) if local_name(start.name().as_ref()) == b"si" => {
                let events = capture_xml_element(
                    &mut reader,
                    Event::Start(start.into_owned()),
                    "shared string item",
                )?;
                if remapping.contains_key(&item_index) {
                    for event in events {
                        writer
                            .write_event(event)
                            .map_err(io_error("write retained XLSX shared string"))?;
                    }
                }
                item_index += 1;
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write compacted XLSX shared strings"))?,
        }
    }
    if !root_seen {
        return Err(template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            "sharedStrings.xml is missing the sst root",
        ));
    }
    if let Some(missing) = remapping.keys().find(|index| **index >= item_index) {
        return Err(template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!(
                "worksheet references shared string {missing}, but only {item_index} items exist"
            ),
        ));
    }
    Ok(writer.into_inner())
}

fn shared_strings_root(
    original: &BytesStart<'_>,
    count: usize,
    unique_count: usize,
) -> Result<BytesStart<'static>, HostError> {
    let mut updated = BytesStart::new("sst");
    for attribute in original.attributes().with_checks(false) {
        let attribute = attribute.map_err(attribute_error("read shared strings root"))?;
        if !matches!(attribute.key.as_ref(), b"count" | b"uniqueCount") {
            updated.push_attribute(attribute.to_owned());
        }
    }
    let count = count.to_string();
    let unique_count = unique_count.to_string();
    updated.push_attribute(("count", count.as_str()));
    updated.push_attribute(("uniqueCount", unique_count.as_str()));
    Ok(updated)
}

fn parse_shared_string_index(value: &str) -> Result<usize, HostError> {
    value.parse::<usize>().map_err(|_| {
        template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!("invalid shared string index: {value:?}"),
        )
    })
}

fn relationship_target_by_type(
    relationships: &[u8],
    expected_type_suffix: &str,
    source_part: &str,
) -> Result<Option<String>, HostError> {
    let mut reader = xml_reader(relationships);
    let mut target = None;
    loop {
        match reader
            .read_event()
            .map_err(xml_error("parse XLSX relationship type"))?
        {
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"Relationship" =>
            {
                let relationship_type = attribute_value(&reader, &start, b"Type")?;
                if !relationship_type
                    .as_deref()
                    .is_some_and(|value| value.ends_with(&format!("/{expected_type_suffix}")))
                {
                    continue;
                }
                if target.is_some() {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_INVALID",
                        format!("duplicate {expected_type_suffix} relationship"),
                    ));
                }
                if attribute_value(&reader, &start, b"TargetMode")?.is_some() {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_UNSAFE",
                        format!("{expected_type_suffix} relationship must be internal"),
                    ));
                }
                let relationship_target =
                    attribute_value(&reader, &start, b"Target")?.ok_or_else(|| {
                        template_error(
                            "BUSINESS_XLSX_PACKAGE_INVALID",
                            format!("{expected_type_suffix} relationship is missing Target"),
                        )
                    })?;
                target = Some(resolve_part_target(source_part, &relationship_target)?);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(target)
}

fn strip_relationship_type(source: &[u8], type_suffix: &str) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("strip XLSX relationship"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"Relationship" => {
                let remove = attribute_value(&reader, &start, b"Type")?
                    .as_deref()
                    .is_some_and(|value| value.ends_with(&format!("/{type_suffix}")));
                if remove {
                    skip_element(&mut reader, b"Relationship")?;
                } else {
                    writer
                        .write_event(Event::Start(start.into_owned()))
                        .map_err(io_error("write XLSX relationships"))?;
                }
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"Relationship" => {
                let remove = attribute_value(&reader, &start, b"Type")?
                    .as_deref()
                    .is_some_and(|value| value.ends_with(&format!("/{type_suffix}")));
                if !remove {
                    writer
                        .write_event(Event::Empty(start.into_owned()))
                        .map_err(io_error("write XLSX relationships"))?;
                }
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write XLSX relationships"))?,
        }
    }
    Ok(writer.into_inner())
}

fn strip_content_type_override(source: &[u8], part_name: &str) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("strip XLSX content type"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"Override" => {
                if attribute_value(&reader, &start, b"PartName")?.as_deref() == Some(part_name) {
                    skip_element(&mut reader, b"Override")?;
                } else {
                    writer
                        .write_event(Event::Start(start.into_owned()))
                        .map_err(io_error("write XLSX content types"))?;
                }
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"Override" => {
                if attribute_value(&reader, &start, b"PartName")?.as_deref() != Some(part_name) {
                    writer
                        .write_event(Event::Empty(start.into_owned()))
                        .map_err(io_error("write XLSX content types"))?;
                }
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write XLSX content types"))?,
        }
    }
    Ok(writer.into_inner())
}

fn validate_template_h16_empty(source: &[u8]) -> Result<(), HostError> {
    let mut reader = xml_reader(source);
    let mut found = false;
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("validate XLSX H16 mapping"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"c" => {
                if attribute_value(&reader, &start, b"r")?.as_deref() != Some("H16") {
                    continue;
                }
                if found {
                    return Err(template_error(
                        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                        "worksheet contains duplicate H16 cells",
                    ));
                }
                found = true;
                let events =
                    capture_xml_element(&mut reader, Event::Start(start.into_owned()), "H16 cell")?;
                if cell_events_have_value(&events)? {
                    return Err(template_error(
                        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                        "template mapping requires H16 to be empty",
                    ));
                }
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"c" => {
                if attribute_value(&reader, &start, b"r")?.as_deref() == Some("H16") {
                    if found {
                        return Err(template_error(
                            "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                            "worksheet contains duplicate H16 cells",
                        ));
                    }
                    found = true;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !found {
        return Err(template_error(
            "BUSINESS_XLSX_TEMPLATE_MISMATCH",
            "template mapping requires an empty H16 cell",
        ));
    }
    Ok(())
}

fn cell_events_have_value(events: &[Event<'static>]) -> Result<bool, HostError> {
    let mut value_element = false;
    for event in events.iter().skip(1) {
        match event {
            Event::Start(start) | Event::Empty(start)
                if matches!(local_name(start.name().as_ref()), b"f" | b"is") =>
            {
                return Ok(true);
            }
            Event::Start(start) if local_name(start.name().as_ref()) == b"v" => {
                value_element = true;
            }
            Event::Text(text) if value_element => {
                let value = text
                    .decode()
                    .map_err(xml_encoding_error("decode H16 value"))?;
                if !value.trim().is_empty() {
                    return Ok(true);
                }
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"v" => {
                value_element = false;
            }
            _ => {}
        }
    }
    Ok(false)
}

fn capture_xml_element(
    reader: &mut Reader<&[u8]>,
    first: Event<'static>,
    description: &str,
) -> Result<Vec<Event<'static>>, HostError> {
    let mut events = vec![first];
    let mut depth = 1_u32;
    while depth > 0 {
        let event = reader
            .read_event()
            .map_err(xml_error("capture XLSX XML element"))?;
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            Event::Eof => {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_INVALID",
                    format!("{description} ended unexpectedly"),
                ));
            }
            _ => {}
        }
        events.push(event.into_owned());
    }
    Ok(events)
}

fn transform_output_sheet(
    source: &[u8],
    data: &ContractSettlementTemplateData,
) -> Result<Vec<u8>, HostError> {
    let h18 = decimal_from_cents(data.final_settlement_amount_cents);
    let retention_cents =
        retention_amount_cents(data.final_settlement_amount_cents, data.retention_rate_bps)?;
    let replacements = BTreeMap::from([
        ("B1", CellReplacement::Inline(&data.customer_legal_name)),
        ("B2", CellReplacement::Inline(&data.contract_title)),
        ("E6", CellReplacement::Inline(&data.project_title)),
        (
            "E7",
            CellReplacement::FormulaString("B2", &data.contract_title),
        ),
        ("E8", CellReplacement::Inline(&data.contract_number)),
        ("E9", CellReplacement::Inline(&data.customer_legal_name)),
        ("E10", CellReplacement::Inline(&data.supplier_legal_name)),
        (
            "H14",
            CellReplacement::Number(decimal_from_cents(data.original_contract_amount_cents)),
        ),
        (
            "H15",
            CellReplacement::Number(decimal_from_cents(data.contract_adjustment_cents)),
        ),
        (
            "E21",
            data.retention_rate_bps
                .map(|value| CellReplacement::Number(decimal_from_bps(value)))
                .unwrap_or(CellReplacement::Blank),
        ),
        ("H18", CellReplacement::FormulaNumber("SUM(H14:H16)", h18)),
        (
            "H21",
            CellReplacement::FormulaNumber("H18*E21", decimal_from_cents(retention_cents)),
        ),
        ("C25", CellReplacement::RichConfirmation),
        (
            "C29",
            CellReplacement::FormulaString("+E9", &data.customer_legal_name),
        ),
        (
            "G29",
            CellReplacement::FormulaString("+E10", &data.supplier_legal_name),
        ),
    ]);

    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len() + 2048));
    let mut replaced = BTreeSet::new();
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("parse XLSX output sheet"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"c" => {
                let reference = attribute_value(&reader, &start, b"r")?.ok_or_else(|| {
                    template_error(
                        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                        "worksheet cell is missing its r attribute",
                    )
                })?;
                if let Some(replacement) = replacements.get(reference.as_str()) {
                    write_replacement_cell(&mut writer, &start, replacement, data)?;
                    skip_element(&mut reader, b"c")?;
                    replaced.insert(reference);
                } else {
                    writer
                        .write_event(Event::Start(start.into_owned()))
                        .map_err(io_error("write XLSX output sheet"))?;
                }
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"c" => {
                let reference = attribute_value(&reader, &start, b"r")?.ok_or_else(|| {
                    template_error(
                        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                        "worksheet cell is missing its r attribute",
                    )
                })?;
                if let Some(replacement) = replacements.get(reference.as_str()) {
                    write_replacement_cell(&mut writer, &start, replacement, data)?;
                    replaced.insert(reference);
                } else {
                    writer
                        .write_event(Event::Empty(start.into_owned()))
                        .map_err(io_error("write XLSX output sheet"))?;
                }
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write XLSX output sheet"))?,
        }
    }

    let missing: Vec<_> = replacements
        .keys()
        .filter(|reference| !replaced.contains(**reference))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(template_error(
            "BUSINESS_XLSX_TEMPLATE_MISMATCH",
            format!(
                "required worksheet cells are missing: {}",
                missing.join(", ")
            ),
        ));
    }
    Ok(writer.into_inner())
}

#[derive(Debug)]
enum CellReplacement<'a> {
    Blank,
    Inline(&'a str),
    Number(String),
    FormulaNumber(&'static str, String),
    FormulaString(&'static str, &'a str),
    RichConfirmation,
}

fn write_replacement_cell<W: Write>(
    writer: &mut Writer<W>,
    original: &BytesStart<'_>,
    replacement: &CellReplacement<'_>,
    data: &ContractSettlementTemplateData,
) -> Result<(), HostError> {
    let cell_type = match replacement {
        CellReplacement::Inline(_) | CellReplacement::RichConfirmation => Some("inlineStr"),
        CellReplacement::FormulaString(_, _) => Some("str"),
        _ => None,
    };
    let mut cell = BytesStart::new("c");
    for attribute in original.attributes().with_checks(false) {
        let attribute = attribute.map_err(attribute_error("read worksheet cell attribute"))?;
        if attribute.key.as_ref() != b"t" {
            cell.push_attribute(attribute);
        }
    }
    if let Some(cell_type) = cell_type {
        cell.push_attribute(("t", cell_type));
    }
    writer
        .write_event(Event::Start(cell))
        .map_err(io_error("write worksheet cell"))?;
    match replacement {
        CellReplacement::Blank => {}
        CellReplacement::Inline(value) => write_inline_string(writer, value)?,
        CellReplacement::Number(value) => write_text_element(writer, "v", value)?,
        CellReplacement::FormulaNumber(formula, value) => {
            write_text_element(writer, "f", formula)?;
            write_text_element(writer, "v", value)?;
        }
        CellReplacement::FormulaString(formula, value) => {
            write_text_element(writer, "f", formula)?;
            write_text_element(writer, "v", value)?;
        }
        CellReplacement::RichConfirmation => write_confirmation_rich_text(writer, data)?,
    }
    writer
        .write_event(Event::End(BytesEnd::new("c")))
        .map_err(io_error("finish worksheet cell"))?;
    Ok(())
}

fn write_inline_string<W: Write>(writer: &mut Writer<W>, value: &str) -> Result<(), HostError> {
    writer
        .write_event(Event::Start(BytesStart::new("is")))
        .map_err(io_error("write inline string"))?;
    write_text_element(writer, "t", value)?;
    writer
        .write_event(Event::End(BytesEnd::new("is")))
        .map_err(io_error("finish inline string"))?;
    Ok(())
}

fn write_confirmation_rich_text<W: Write>(
    writer: &mut Writer<W>,
    data: &ContractSettlementTemplateData,
) -> Result<(), HostError> {
    let small_amount = (data.final_settlement_amount_cents / 100).to_string();
    let runs = [
        ("兹特此同意确认", "华文细黑", "FF000000"),
        ("上述", "华文细黑", "FFFF0000"),
        ("合同最终结算书内所示之金额人民币：", "华文细黑", "FF000000"),
        (
            &format!("{}（", data.final_settlement_amount_uppercase_cny),
            "华文细黑",
            "FFFF0000",
        ),
        (&small_amount, "华文细黑", "FFFF0000"),
        ("RMB", "Arial", "FFFF0000"),
        ("元）", "华文细黑", "FFFF0000"),
        ("，按本合同结清所有应付款项", "华文细黑", "FF000000"),
        (",", "Arial", "FF000000"),
        (
            "承包方放弃一切对发包方的索赔。承包方仍需按合同约定完成质保期的所有工作并承担相应责任（如有）。",
            "华文细黑",
            "FF000000",
        ),
    ];

    writer
        .write_event(Event::Start(BytesStart::new("is")))
        .map_err(io_error("write confirmation rich text"))?;
    for (text, font, color) in runs {
        writer
            .write_event(Event::Start(BytesStart::new("r")))
            .map_err(io_error("write rich text run"))?;
        writer
            .write_event(Event::Start(BytesStart::new("rPr")))
            .map_err(io_error("write rich text properties"))?;
        write_value_element(writer, "rFont", font)?;
        write_value_element(writer, "sz", "10")?;
        let mut color_element = BytesStart::new("color");
        color_element.push_attribute(("rgb", color));
        writer
            .write_event(Event::Empty(color_element))
            .map_err(io_error("write rich text color"))?;
        writer
            .write_event(Event::End(BytesEnd::new("rPr")))
            .map_err(io_error("finish rich text properties"))?;
        write_text_element(writer, "t", text)?;
        writer
            .write_event(Event::End(BytesEnd::new("r")))
            .map_err(io_error("finish rich text run"))?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("is")))
        .map_err(io_error("finish confirmation rich text"))?;
    Ok(())
}

fn write_value_element<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: &str,
) -> Result<(), HostError> {
    let mut element = BytesStart::new(name);
    element.push_attribute(("val", value));
    writer
        .write_event(Event::Empty(element))
        .map_err(io_error("write XML value element"))?;
    Ok(())
}

fn write_text_element<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: &str,
) -> Result<(), HostError> {
    writer
        .write_event(Event::Start(BytesStart::new(name)))
        .map_err(io_error("write XML text element"))?;
    writer
        .write_event(Event::Text(BytesText::new(value)))
        .map_err(io_error("write XML text"))?;
    writer
        .write_event(Event::End(BytesEnd::new(name)))
        .map_err(io_error("finish XML text element"))?;
    Ok(())
}

fn transform_drawing(source: &[u8]) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut removed = BTreeSet::new();
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("parse XLSX drawing"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"twoCellAnchor" => {
                let (events, names) =
                    capture_element(&mut reader, Event::Start(start.into_owned()))?;
                let removable: Vec<_> = names
                    .iter()
                    .filter_map(|name| removable_oval_number(name))
                    .collect();
                if removable.is_empty() {
                    for event in events {
                        writer
                            .write_event(event)
                            .map_err(io_error("write XLSX drawing"))?;
                    }
                } else {
                    for number in removable {
                        removed.insert(number);
                    }
                }
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write XLSX drawing"))?,
        }
    }
    let expected = BTreeSet::from([6_u8, 7, 8]);
    if removed != expected {
        return Err(template_error(
            "BUSINESS_XLSX_TEMPLATE_MISMATCH",
            format!("drawing must contain exactly Oval 6, Oval 7 and Oval 8; removed {removed:?}"),
        ));
    }
    Ok(writer.into_inner())
}

fn capture_element(
    reader: &mut Reader<&[u8]>,
    first: Event<'static>,
) -> Result<(Vec<Event<'static>>, Vec<String>), HostError> {
    let mut events = vec![first];
    let mut names = Vec::new();
    let mut depth = 1_u32;
    while depth > 0 {
        let event = reader
            .read_event()
            .map_err(xml_error("parse XLSX drawing anchor"))?;
        match &event {
            Event::Start(start) => {
                if local_name(start.name().as_ref()) == b"cNvPr" {
                    if let Some(name) = attribute_value(reader, start, b"name")? {
                        names.push(name);
                    }
                }
                depth += 1;
            }
            Event::Empty(start) => {
                if local_name(start.name().as_ref()) == b"cNvPr" {
                    if let Some(name) = attribute_value(reader, start, b"name")? {
                        names.push(name);
                    }
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_INVALID",
                    "drawing anchor ended unexpectedly",
                ));
            }
            _ => {}
        }
        events.push(event.into_owned());
    }
    Ok((events, names))
}

fn removable_oval_number(name: &str) -> Option<u8> {
    let normalized = name.trim();
    let (prefix, number) = normalized.rsplit_once(' ')?;
    if !matches!(prefix, "Oval" | "椭圆") {
        return None;
    }
    match number.parse::<u8>().ok()? {
        value @ 6..=8 => Some(value),
        _ => None,
    }
}

fn sanitize_workbook(source: &[u8]) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut calc_pr_seen = false;
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("parse XLSX workbook for sanitization"))?;
        match event {
            Event::Start(start) if local_name(start.name().as_ref()) == b"AlternateContent" => {
                skip_element(&mut reader, b"AlternateContent")?;
            }
            Event::Start(start) if local_name(start.name().as_ref()) == b"calcPr" => {
                calc_pr_seen = true;
                let updated = calc_pr_element(&start)?;
                writer
                    .write_event(Event::Start(updated))
                    .map_err(io_error("write XLSX calculation settings"))?;
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"calcPr" => {
                calc_pr_seen = true;
                let updated = calc_pr_element(&start)?;
                writer
                    .write_event(Event::Empty(updated))
                    .map_err(io_error("write XLSX calculation settings"))?;
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"workbook" => {
                if !calc_pr_seen {
                    writer
                        .write_event(Event::Empty(calc_pr_element(&BytesStart::new("calcPr"))?))
                        .map_err(io_error("add XLSX calculation settings"))?;
                }
                writer
                    .write_event(Event::End(end.into_owned()))
                    .map_err(io_error("finish XLSX workbook"))?;
            }
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write sanitized XLSX workbook"))?,
        }
    }
    Ok(writer.into_inner())
}

fn calc_pr_element(original: &BytesStart<'_>) -> Result<BytesStart<'static>, HostError> {
    let mut updated = BytesStart::new("calcPr");
    for attribute in original.attributes().with_checks(false) {
        let attribute = attribute.map_err(attribute_error("read calculation setting"))?;
        if !matches!(
            attribute.key.as_ref(),
            b"calcMode" | b"fullCalcOnLoad" | b"forceFullCalc"
        ) {
            updated.push_attribute(attribute.to_owned());
        }
    }
    updated.push_attribute(("calcMode", "auto"));
    updated.push_attribute(("fullCalcOnLoad", "1"));
    updated.push_attribute(("forceFullCalc", "1"));
    Ok(updated)
}

fn sanitize_core_properties(source: &[u8]) -> Result<Vec<u8>, HostError> {
    let mut reader = xml_reader(source);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    loop {
        let event = reader
            .read_event()
            .map_err(xml_error("parse XLSX core properties"))?;
        match event {
            Event::Start(start)
                if matches!(
                    local_name(start.name().as_ref()),
                    b"creator" | b"lastModifiedBy" | b"lastPrinted"
                ) =>
            {
                let name = local_name(start.name().as_ref()).to_vec();
                skip_element(&mut reader, &name)?;
            }
            Event::Empty(start)
                if matches!(
                    local_name(start.name().as_ref()),
                    b"creator" | b"lastModifiedBy" | b"lastPrinted"
                ) => {}
            Event::Eof => break,
            other => writer
                .write_event(other.into_owned())
                .map_err(io_error("write sanitized XLSX core properties"))?,
        }
    }
    Ok(writer.into_inner())
}

fn validate_generated_package(
    package: &Package,
    data: &ContractSettlementTemplateData,
) -> Result<(), HostError> {
    package.validate_ooxml_xlsx()?;
    let sheet_part = package.resolve_output_sheet()?;
    let drawing_part = package.resolve_sheet_drawing(&sheet_part)?;
    let workbook = package.required(WORKBOOK_PART)?;
    validate_workbook(workbook)?;

    let sheet = parse_sheet(package.required(&sheet_part)?)?;
    let expected_merges = BTreeSet::from([
        "B2:I2".to_string(),
        "C17:E17".to_string(),
        "C25:H25".to_string(),
    ]);
    if sheet.merged_ranges != expected_merges {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!("merged ranges changed: {:?}", sheet.merged_ranges),
        ));
    }
    if !sheet.hidden_rows.contains(&22) || !sheet.hidden_rows.contains(&23) {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            "rows 22 and 23 must remain hidden",
        ));
    }
    if sheet.row_heights.get(&25).map(String::as_str) != Some("43.5") {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            "row 25 height must remain 43.5",
        ));
    }

    assert_cell_value(&sheet, "B1", &data.customer_legal_name)?;
    assert_cell_value(&sheet, "B2", &data.contract_title)?;
    assert_cell_value(&sheet, "E6", &data.project_title)?;
    assert_cell_value(&sheet, "E8", &data.contract_number)?;
    assert_cell_value(&sheet, "E9", &data.customer_legal_name)?;
    assert_cell_value(&sheet, "E10", &data.supplier_legal_name)?;
    assert_cell_value(
        &sheet,
        "H14",
        &decimal_from_cents(data.original_contract_amount_cents),
    )?;
    assert_cell_value(
        &sheet,
        "H15",
        &decimal_from_cents(data.contract_adjustment_cents),
    )?;
    let expected_retention_rate = data.retention_rate_bps.map(decimal_from_bps);
    let e21 = required_cell(&sheet, "E21")?;
    if e21.value != expected_retention_rate {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!("E21 retention rate mismatch: {:?}", e21.value),
        ));
    }

    assert_formula(&sheet, "E7", "B2", &data.contract_title)?;
    assert_formula(
        &sheet,
        "H18",
        "SUM(H14:H16)",
        &decimal_from_cents(data.final_settlement_amount_cents),
    )?;
    assert_formula(
        &sheet,
        "H21",
        "H18*E21",
        &decimal_from_cents(retention_amount_cents(
            data.final_settlement_amount_cents,
            data.retention_rate_bps,
        )?),
    )?;
    assert_formula(&sheet, "C29", "+E9", &data.customer_legal_name)?;
    assert_formula(&sheet, "G29", "+E10", &data.supplier_legal_name)?;

    let confirmation = required_cell(&sheet, "C25")?;
    if confirmation.cell_type.as_deref() != Some("inlineStr")
        || !confirmation
            .inline_text
            .contains(&data.final_settlement_amount_uppercase_cny)
        || !confirmation.inline_text.contains(&format!(
            "{}RMB元",
            data.final_settlement_amount_cents / 100
        ))
        || confirmation.rich_runs.len() < 8
        || !confirmation.rich_runs.iter().any(|run| {
            run.text == "RMB"
                && run.font.as_deref() == Some("Arial")
                && is_red(run.color.as_deref())
        })
        || !confirmation.rich_runs.iter().any(|run| {
            run.text == "上述"
                && run.font.as_deref() == Some("华文细黑")
                && is_red(run.color.as_deref())
        })
        || !confirmation.rich_runs.iter().any(|run| {
            run.text == "兹特此同意确认"
                && run.font.as_deref() == Some("华文细黑")
                && is_black(run.color.as_deref())
        })
    {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            "C25 confirmation rich text does not satisfy the red/black and Arial RMB contract",
        ));
    }

    let shape_names = drawing_shape_names(package.required(&drawing_part)?)?;
    for line in ["Line 1", "Line 2", "Line 3", "Line 4"] {
        if !shape_names.contains(line) {
            return Err(template_error(
                "BUSINESS_XLSX_OUTPUT_INVALID",
                format!("required signature shape is missing: {line}"),
            ));
        }
    }
    if shape_names
        .iter()
        .any(|name| removable_oval_number(name).is_some())
    {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            "instructional stamp ovals remain in the output drawing",
        ));
    }

    if package.contains(CORE_PROPERTIES_PART) {
        validate_core_properties(package.required(CORE_PROPERTIES_PART)?)?;
    }
    Ok(())
}

fn validate_workbook(source: &[u8]) -> Result<(), HostError> {
    let mut reader = xml_reader(source);
    let mut full_calc = false;
    let mut print_area = None;
    let mut current_defined_name = false;
    loop {
        match reader
            .read_event()
            .map_err(xml_error("validate XLSX workbook"))?
        {
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"absPath" =>
            {
                return Err(template_error(
                    "BUSINESS_XLSX_OUTPUT_INVALID",
                    "x15ac:absPath was not removed from workbook metadata",
                ));
            }
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"calcPr" =>
            {
                full_calc = attribute_value(&reader, &start, b"calcMode")?.as_deref()
                    == Some("auto")
                    && attribute_value(&reader, &start, b"fullCalcOnLoad")?.as_deref() == Some("1")
                    && attribute_value(&reader, &start, b"forceFullCalc")?.as_deref() == Some("1");
            }
            Event::Start(start) if local_name(start.name().as_ref()) == b"definedName" => {
                current_defined_name = attribute_value(&reader, &start, b"name")?.as_deref()
                    == Some("_xlnm.Print_Area");
            }
            Event::Text(text) if current_defined_name => {
                print_area = Some(
                    text.decode()
                        .map_err(xml_encoding_error("decode XLSX print area"))?
                        .into_owned(),
                );
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"definedName" => {
                current_defined_name = false;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !full_calc {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            "workbook full recalculation flags are missing",
        ));
    }
    if print_area.as_deref() != Some("附件1最终结算书!$A$1:$I$34") {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!("print area changed: {print_area:?}"),
        ));
    }
    Ok(())
}

fn parse_sheet(source: &[u8]) -> Result<SheetSnapshot, HostError> {
    let mut reader = xml_reader(source);
    let mut snapshot = SheetSnapshot::default();
    let mut current_reference: Option<String> = None;
    let mut current_cell = CellSnapshot::default();
    let mut current_text_element: Option<Vec<u8>> = None;
    let mut current_run: Option<RichRun> = None;
    loop {
        match reader
            .read_event()
            .map_err(xml_error("validate XLSX output sheet"))?
        {
            Event::Start(start) if local_name(start.name().as_ref()) == b"c" => {
                current_reference = attribute_value(&reader, &start, b"r")?;
                current_cell = CellSnapshot {
                    cell_type: attribute_value(&reader, &start, b"t")?,
                    ..CellSnapshot::default()
                };
            }
            Event::Start(start)
                if current_reference.is_some()
                    && matches!(local_name(start.name().as_ref()), b"f" | b"v" | b"t") =>
            {
                current_text_element = Some(local_name(start.name().as_ref()).to_vec());
            }
            Event::Start(start)
                if current_reference.is_some() && local_name(start.name().as_ref()) == b"r" =>
            {
                current_run = Some(RichRun::default());
            }
            Event::Start(start) | Event::Empty(start)
                if current_run.is_some() && local_name(start.name().as_ref()) == b"rFont" =>
            {
                current_run.as_mut().expect("checked above").font =
                    attribute_value(&reader, &start, b"val")?;
            }
            Event::Start(start) | Event::Empty(start)
                if current_run.is_some() && local_name(start.name().as_ref()) == b"color" =>
            {
                current_run.as_mut().expect("checked above").color =
                    attribute_value(&reader, &start, b"rgb")?
                        .or(attribute_value(&reader, &start, b"indexed")?);
            }
            Event::Text(text) if current_reference.is_some() => {
                let value = text
                    .decode()
                    .map_err(xml_encoding_error("decode XLSX cell text"))?
                    .into_owned();
                match current_text_element.as_deref() {
                    Some(b"f") => current_cell
                        .formula
                        .get_or_insert_with(String::new)
                        .push_str(&value),
                    Some(b"v") => current_cell
                        .value
                        .get_or_insert_with(String::new)
                        .push_str(&value),
                    Some(b"t") => {
                        current_cell.inline_text.push_str(&value);
                        if let Some(run) = current_run.as_mut() {
                            run.text.push_str(&value);
                        }
                    }
                    _ => {}
                }
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"r" => {
                if let Some(run) = current_run.take() {
                    current_cell.rich_runs.push(run);
                }
            }
            Event::End(end) if matches!(local_name(end.name().as_ref()), b"f" | b"v" | b"t") => {
                current_text_element = None;
            }
            Event::End(end) if local_name(end.name().as_ref()) == b"c" => {
                if let Some(reference) = current_reference.take() {
                    snapshot
                        .cells
                        .insert(reference, std::mem::take(&mut current_cell));
                }
            }
            Event::Empty(start) if local_name(start.name().as_ref()) == b"mergeCell" => {
                if let Some(reference) = attribute_value(&reader, &start, b"ref")? {
                    snapshot.merged_ranges.insert(reference);
                }
            }
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"row" =>
            {
                if let Some(number) = attribute_value(&reader, &start, b"r")?
                    .and_then(|value| value.parse::<u32>().ok())
                {
                    if attribute_value(&reader, &start, b"hidden")?.as_deref() == Some("1") {
                        snapshot.hidden_rows.insert(number);
                    }
                    if let Some(height) = attribute_value(&reader, &start, b"ht")? {
                        snapshot.row_heights.insert(number, height);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(snapshot)
}

fn assert_cell_value(
    sheet: &SheetSnapshot,
    reference: &str,
    expected: &str,
) -> Result<(), HostError> {
    let cell = required_cell(sheet, reference)?;
    let actual = if cell.cell_type.as_deref() == Some("inlineStr") {
        Some(cell.inline_text.as_str())
    } else {
        cell.value.as_deref()
    };
    if actual != Some(expected) {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!("{reference} value mismatch: expected {expected:?}, got {actual:?}"),
        ));
    }
    Ok(())
}

fn assert_formula(
    sheet: &SheetSnapshot,
    reference: &str,
    expected_formula: &str,
    expected_value: &str,
) -> Result<(), HostError> {
    let cell = required_cell(sheet, reference)?;
    if cell.formula.as_deref() != Some(expected_formula)
        || cell.value.as_deref() != Some(expected_value)
    {
        return Err(template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!(
                "{reference} formula/cache mismatch: formula={:?}, value={:?}",
                cell.formula, cell.value
            ),
        ));
    }
    Ok(())
}

fn required_cell<'a>(
    sheet: &'a SheetSnapshot,
    reference: &str,
) -> Result<&'a CellSnapshot, HostError> {
    sheet.cells.get(reference).ok_or_else(|| {
        template_error(
            "BUSINESS_XLSX_OUTPUT_INVALID",
            format!("required output cell is missing: {reference}"),
        )
    })
}

fn drawing_shape_names(source: &[u8]) -> Result<BTreeSet<String>, HostError> {
    let mut reader = xml_reader(source);
    let mut names = BTreeSet::new();
    loop {
        match reader
            .read_event()
            .map_err(xml_error("validate XLSX drawing"))?
        {
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"cNvPr" =>
            {
                if let Some(name) = attribute_value(&reader, &start, b"name")? {
                    names.insert(name);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(names)
}

fn validate_core_properties(source: &[u8]) -> Result<(), HostError> {
    let mut reader = xml_reader(source);
    loop {
        match reader
            .read_event()
            .map_err(xml_error("validate XLSX core properties"))?
        {
            Event::Start(start) | Event::Empty(start)
                if matches!(
                    local_name(start.name().as_ref()),
                    b"creator" | b"lastModifiedBy" | b"lastPrinted"
                ) =>
            {
                return Err(template_error(
                    "BUSINESS_XLSX_OUTPUT_INVALID",
                    "personal author or last-print metadata remains in XLSX output",
                ));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

fn validate_input(data: &ContractSettlementTemplateData) -> Result<(), HostError> {
    for (label, value) in [
        ("project_title", &data.project_title),
        ("contract_title", &data.contract_title),
        ("contract_number", &data.contract_number),
        ("customer_legal_name", &data.customer_legal_name),
        ("supplier_legal_name", &data.supplier_legal_name),
    ] {
        if value.trim().is_empty() {
            return Err(template_error(
                "BUSINESS_XLSX_INPUT_INVALID",
                format!("{label} must not be empty"),
            ));
        }
        if value.chars().any(|character| !is_xml_character(character)) {
            return Err(template_error(
                "BUSINESS_XLSX_INPUT_INVALID",
                format!("{label} contains a character that XML cannot represent"),
            ));
        }
    }
    if data.original_contract_amount_cents < 0 || data.final_settlement_amount_cents < 0 {
        return Err(template_error(
            "BUSINESS_XLSX_INPUT_INVALID",
            "contract and final settlement amounts must not be negative",
        ));
    }
    let calculated_final = data
        .original_contract_amount_cents
        .checked_add(data.contract_adjustment_cents)
        .ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_INPUT_INVALID",
                "contract settlement amount overflowed i64 cents",
            )
        })?;
    if calculated_final != data.final_settlement_amount_cents {
        return Err(template_error(
            "BUSINESS_XLSX_AMOUNT_MISMATCH",
            format!(
                "final settlement cents must equal original plus adjustment: expected {calculated_final}, got {}",
                data.final_settlement_amount_cents
            ),
        ));
    }
    if data.final_settlement_amount_cents % 100 != 0 {
        return Err(template_error(
            "BUSINESS_XLSX_FRACTIONAL_CNY_UNSUPPORTED",
            "C25 small-amount wording for jiao/fen is not approved; non-whole-yuan settlement is rejected",
        ));
    }
    if data.retention_rate_bps.is_some_and(|value| value > 10_000) {
        return Err(template_error(
            "BUSINESS_XLSX_INPUT_INVALID",
            "retention_rate_bps must be between 0 and 10000",
        ));
    }
    let generated_uppercase = uppercase_cny(data.final_settlement_amount_cents)?;
    if data.final_settlement_amount_uppercase_cny.trim() != generated_uppercase {
        return Err(template_error(
            "BUSINESS_XLSX_AMOUNT_MISMATCH",
            format!(
                "uppercase CNY does not match final settlement cents: expected {generated_uppercase}"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn uppercase_cny(cents: i64) -> Result<String, HostError> {
    if cents < 0 || cents % 100 != 0 {
        return Err(template_error(
            "BUSINESS_XLSX_FRACTIONAL_CNY_UNSUPPORTED",
            "uppercase CNY generation requires a non-negative whole-yuan amount",
        ));
    }
    let yuan = cents / 100;
    if yuan == 0 {
        return Ok("零元整".to_string());
    }
    const GROUP_UNITS: [&str; 4] = ["", "万", "亿", "万亿"];
    let mut groups = Vec::new();
    let mut remaining = yuan;
    while remaining > 0 {
        groups.push((remaining % 10_000) as u16);
        remaining /= 10_000;
    }
    if groups.len() > GROUP_UNITS.len() {
        return Err(template_error(
            "BUSINESS_XLSX_INPUT_INVALID",
            "settlement amount is too large for the approved uppercase CNY format",
        ));
    }

    let mut output = String::new();
    let mut zero_pending = false;
    for group_index in (0..groups.len()).rev() {
        let group = groups[group_index];
        if group == 0 {
            if !output.is_empty() {
                zero_pending = true;
            }
            continue;
        }
        if !output.is_empty() && (zero_pending || group < 1000) {
            output.push('零');
        }
        output.push_str(&uppercase_group(group));
        output.push_str(GROUP_UNITS[group_index]);
        zero_pending = false;
    }
    output.push_str("元整");
    Ok(output)
}

fn uppercase_group(group: u16) -> String {
    const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    const UNITS: [&str; 4] = ["仟", "佰", "拾", ""];
    let divisors = [1000_u16, 100, 10, 1];
    let mut output = String::new();
    let mut zero_pending = false;
    for (index, divisor) in divisors.into_iter().enumerate() {
        let digit = (group / divisor) % 10;
        if digit == 0 {
            if !output.is_empty() && !group.is_multiple_of(divisor) {
                zero_pending = true;
            }
            continue;
        }
        if zero_pending {
            output.push('零');
            zero_pending = false;
        }
        output.push_str(DIGITS[digit as usize]);
        output.push_str(UNITS[index]);
    }
    output
}

fn retention_amount_cents(final_cents: i64, rate_bps: Option<u32>) -> Result<i64, HostError> {
    let Some(rate_bps) = rate_bps else {
        return Ok(0);
    };
    let numerator = i128::from(final_cents)
        .checked_mul(i128::from(rate_bps))
        .ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_INPUT_INVALID",
                "retention amount overflowed during calculation",
            )
        })?;
    let rounded = (numerator + 5_000) / 10_000;
    i64::try_from(rounded).map_err(|_| {
        template_error(
            "BUSINESS_XLSX_INPUT_INVALID",
            "retention amount does not fit i64 cents",
        )
    })
}

fn decimal_from_cents(cents: i64) -> String {
    let value = i128::from(cents);
    let sign = if value < 0 { "-" } else { "" };
    let absolute = value.abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

fn decimal_from_bps(bps: u32) -> String {
    if bps == 10_000 {
        return "1".to_string();
    }
    let mut fraction = format!("{bps:04}");
    while fraction.ends_with('0') {
        fraction.pop();
    }
    if fraction.is_empty() {
        "0".to_string()
    } else {
        format!("0.{fraction}")
    }
}

fn validate_destination(source: &Path, destination: &Path) -> Result<(), HostError> {
    let canonical_source =
        fs::canonicalize(source).map_err(io_error("resolve XLSX template path"))?;
    let destination_parent = destination.parent().ok_or_else(|| {
        template_error(
            "BUSINESS_XLSX_DESTINATION_INVALID",
            "destination must have a parent directory",
        )
    })?;
    let canonical_parent = fs::canonicalize(destination_parent)
        .map_err(io_error("resolve XLSX destination parent"))?;
    let destination_name = destination.file_name().ok_or_else(|| {
        template_error(
            "BUSINESS_XLSX_DESTINATION_INVALID",
            "destination must name an XLSX file",
        )
    })?;
    let canonical_destination = canonical_parent.join(destination_name);
    if paths_equal(&canonical_source, &canonical_destination) {
        return Err(template_error(
            "BUSINESS_XLSX_DESTINATION_INVALID",
            "source and destination paths must differ",
        ));
    }
    validate_output_destination(destination)
}

fn validate_output_destination(destination: &Path) -> Result<(), HostError> {
    let destination_parent = destination.parent().ok_or_else(|| {
        template_error(
            "BUSINESS_XLSX_DESTINATION_INVALID",
            "destination must have a parent directory",
        )
    })?;
    fs::canonicalize(destination_parent).map_err(io_error("resolve XLSX destination parent"))?;
    if destination.exists() {
        return Err(template_error(
            "BUSINESS_XLSX_DESTINATION_EXISTS",
            format!("destination already exists: {}", destination.display()),
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
        return Err(template_error(
            "BUSINESS_XLSX_TEMPLATE_HASH_INVALID",
            "expected template SHA-256 must contain exactly 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn read_file_limited(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, HostError> {
    let file = File::open(path).map_err(io_error("open XLSX template"))?;
    let mut bytes = Vec::new();
    file.take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error("read XLSX template"))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(package_limit_error(format!(
            "XLSX source exceeds {maximum_bytes} bytes"
        )));
    }
    Ok(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:X}", digest.finalize())
}

fn sha256_file(path: &Path) -> Result<String, HostError> {
    Ok(sha256_bytes(&read_file_limited(
        path,
        MAX_XLSX_SOURCE_BYTES,
    )?))
}

fn validate_zip_entry_limits(
    name: &str,
    uncompressed_size: u64,
    compressed_size: u64,
    total_uncompressed: &mut u64,
) -> Result<(), HostError> {
    if uncompressed_size > MAX_ZIP_ENTRY_BYTES {
        return Err(package_limit_error(format!(
            "XLSX package entry {name} declares {uncompressed_size} bytes; maximum is {MAX_ZIP_ENTRY_BYTES}"
        )));
    }
    if uncompressed_size > 0
        && (compressed_size == 0
            || uncompressed_size > compressed_size.saturating_mul(MAX_ZIP_COMPRESSION_RATIO))
    {
        return Err(package_limit_error(format!(
            "XLSX package entry {name} exceeds compression ratio {MAX_ZIP_COMPRESSION_RATIO}:1"
        )));
    }
    *total_uncompressed = total_uncompressed
        .checked_add(uncompressed_size)
        .ok_or_else(|| package_limit_error("XLSX package total size overflow"))?;
    if *total_uncompressed > MAX_ZIP_TOTAL_BYTES {
        return Err(package_limit_error(format!(
            "XLSX package declares more than {MAX_ZIP_TOTAL_BYTES} uncompressed bytes"
        )));
    }
    Ok(())
}

fn package_limit_error(message: impl Into<String>) -> HostError {
    template_error("BUSINESS_XLSX_PACKAGE_LIMIT_EXCEEDED", message)
}

fn validate_package_path(name: &str) -> Result<(), HostError> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('/')
        || name
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || Path::new(name)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(template_error(
            "BUSINESS_XLSX_PACKAGE_UNSAFE",
            format!("unsafe XLSX package entry path: {name:?}"),
        ));
    }
    Ok(())
}

fn reject_external_relationships(source: &[u8], part_name: &str) -> Result<(), HostError> {
    let mut reader = xml_reader(source);
    loop {
        match reader
            .read_event()
            .map_err(xml_error("parse XLSX relationships"))?
        {
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"Relationship" =>
            {
                if attribute_value(&reader, &start, b"TargetMode")?
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("External"))
                {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_UNSAFE",
                        format!("external relationship is not allowed in {part_name}"),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

fn resolve_relationship_target(
    relationships: &[u8],
    relationship_id: &str,
    expected_type_suffix: &str,
    source_part: &str,
) -> Result<String, HostError> {
    let mut reader = xml_reader(relationships);
    loop {
        match reader
            .read_event()
            .map_err(xml_error("parse XLSX relationship target"))?
        {
            Event::Start(start) | Event::Empty(start)
                if local_name(start.name().as_ref()) == b"Relationship" =>
            {
                if attribute_value(&reader, &start, b"Id")?.as_deref() != Some(relationship_id) {
                    continue;
                }
                if attribute_value(&reader, &start, b"TargetMode")?.is_some() {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_UNSAFE",
                        "output relationship target must be internal",
                    ));
                }
                let relationship_type =
                    attribute_value(&reader, &start, b"Type")?.ok_or_else(|| {
                        template_error(
                            "BUSINESS_XLSX_PACKAGE_INVALID",
                            "relationship is missing Type",
                        )
                    })?;
                if !relationship_type.ends_with(&format!("/{expected_type_suffix}")) {
                    return Err(template_error(
                        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
                        format!("relationship {relationship_id} is not a {expected_type_suffix}"),
                    ));
                }
                let target = attribute_value(&reader, &start, b"Target")?.ok_or_else(|| {
                    template_error(
                        "BUSINESS_XLSX_PACKAGE_INVALID",
                        "relationship is missing Target",
                    )
                })?;
                return resolve_part_target(source_part, &target);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Err(template_error(
        "BUSINESS_XLSX_TEMPLATE_MISMATCH",
        format!("relationship was not found: {relationship_id}"),
    ))
}

fn resolve_part_target(source_part: &str, target: &str) -> Result<String, HostError> {
    if target.is_empty()
        || target.contains('\\')
        || target.contains(':')
        || target.contains('?')
        || target.contains('#')
        || target.starts_with('/')
    {
        return Err(template_error(
            "BUSINESS_XLSX_PACKAGE_UNSAFE",
            format!("unsafe relationship target: {target:?}"),
        ));
    }
    let mut components: Vec<String> = source_part
        .split('/')
        .take(source_part.split('/').count().saturating_sub(1))
        .map(str::to_string)
        .collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_UNSAFE",
                        format!("relationship target escapes package root: {target}"),
                    ));
                }
            }
            _ => components.push(part.to_string()),
        }
    }
    let resolved = components.join("/");
    validate_package_path(&resolved)?;
    Ok(resolved)
}

fn relationships_part_for(part: &str) -> Result<String, HostError> {
    let path = Path::new(part);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            template_error(
                "BUSINESS_XLSX_PACKAGE_INVALID",
                format!("invalid package part path: {part}"),
            )
        })?;
    let parent = path.parent().and_then(Path::to_str).unwrap_or_default();
    Ok(format!("{parent}/_rels/{file_name}.rels"))
}

fn skip_element(reader: &mut Reader<&[u8]>, expected_local_name: &[u8]) -> Result<(), HostError> {
    let mut depth = 1_u32;
    while depth > 0 {
        match reader
            .read_event()
            .map_err(xml_error("skip XLSX XML element"))?
        {
            Event::Start(_) => depth += 1,
            Event::End(end) => {
                depth -= 1;
                if depth == 0 && local_name(end.name().as_ref()) != expected_local_name {
                    return Err(template_error(
                        "BUSINESS_XLSX_PACKAGE_INVALID",
                        "unexpected XML element boundary",
                    ));
                }
            }
            Event::Eof => {
                return Err(template_error(
                    "BUSINESS_XLSX_PACKAGE_INVALID",
                    "XML element ended unexpectedly",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn attribute_value(
    reader: &Reader<&[u8]>,
    start: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, HostError> {
    for attribute in start.attributes().with_checks(false) {
        let attribute = attribute.map_err(attribute_error("read XLSX XML attribute"))?;
        if attribute.key.as_ref() == name {
            return attribute
                .decode_and_unescape_value(reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(xml_error("decode XLSX XML attribute"));
        }
    }
    Ok(None)
}

fn xml_reader(source: &[u8]) -> Reader<&[u8]> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    reader
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn is_red(color: Option<&str>) -> bool {
    matches!(color, Some("FFFF0000") | Some("FF0000") | Some("10"))
}

fn is_black(color: Option<&str>) -> bool {
    matches!(color, Some("FF000000") | Some("000000") | Some("8"))
}

fn is_xml_character(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&value)
        || ('\u{E000}'..='\u{FFFD}').contains(&value)
        || ('\u{10000}'..='\u{10FFFF}').contains(&value)
}

fn template_error(code: &'static str, message: impl Into<String>) -> HostError {
    HostError::new(code, message, false)
}

fn io_error(action: &'static str) -> impl FnOnce(std::io::Error) -> HostError {
    move |error| {
        HostError::new(
            "BUSINESS_XLSX_IO_FAILED",
            format!("{action} failed: {error}"),
            true,
        )
    }
}

fn zip_error(action: &'static str) -> impl FnOnce(zip::result::ZipError) -> HostError {
    move |error| {
        template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!("{action} failed: {error}"),
        )
    }
}

fn xml_error(action: &'static str) -> impl FnOnce(quick_xml::Error) -> HostError {
    move |error| {
        template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!("{action} failed: {error}"),
        )
    }
}

fn xml_encoding_error(
    action: &'static str,
) -> impl FnOnce(quick_xml::encoding::EncodingError) -> HostError {
    move |error| {
        template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!("{action} failed: {error}"),
        )
    }
}

fn attribute_error(
    action: &'static str,
) -> impl FnOnce(quick_xml::events::attributes::AttrError) -> HostError {
    move |error| {
        template_error(
            "BUSINESS_XLSX_PACKAGE_INVALID",
            format!("{action} failed: {error}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use tempfile::TempDir;

    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/not-sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/drawings/drawing9.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;
    const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice><x15ac:absPath xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac" url="C:\secret\"/></mc:Choice></mc:AlternateContent><sheets><sheet name="Other" sheetId="1" r:id="rId1"/><sheet name="附件1最终结算书" sheetId="2" r:id="rId9"/></sheets><definedNames><definedName name="_xlnm.Print_Area" localSheetId="1">附件1最终结算书!$A$1:$I$34</definedName></definedNames><calcPr calcId="1"/></workbook>"#;
    const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/other.xml"/><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/not-sheet1.xml"/></Relationships>"#;
    const SHEET_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdDraw" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing9.xml"/></Relationships>"#;
    const CORE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Person</dc:creator><cp:lastModifiedBy>Other</cp:lastModifiedBy><cp:lastPrinted>2026-01-01T00:00:00Z</cp:lastPrinted></cp:coreProperties>"#;

    fn fixture_data() -> ContractSettlementTemplateData {
        ContractSettlementTemplateData {
            project_title: "白鹅潭瑞玺".to_string(),
            contract_title: "营销视频全案服务落地合同".to_string(),
            contract_number: "CRCGF-YX2026-127".to_string(),
            customer_legal_name: "广州市润川房地产开发有限公司".to_string(),
            supplier_legal_name: "广州华邦互娱科技有限公司".to_string(),
            original_contract_amount_cents: 9_752_000,
            contract_adjustment_cents: 0,
            retention_rate_bps: Some(500),
            final_settlement_amount_cents: 9_752_000,
            final_settlement_amount_uppercase_cny: "玖万柒仟伍佰贰拾元整".to_string(),
        }
    }

    fn fixture_sheet() -> String {
        let cells = [
            "<c r=\"B1\" s=\"1\"/>",
            "<c r=\"B2\" s=\"2\"/>",
            "<c r=\"E6\" s=\"3\"/>",
            "<c r=\"E7\" s=\"3\"><f>B2</f><v>old</v></c>",
            "<c r=\"E8\" s=\"3\"/>",
            "<c r=\"E9\" s=\"3\"/>",
            "<c r=\"E10\" s=\"3\"/>",
            "<c r=\"H14\" s=\"4\"/>",
            "<c r=\"H15\" s=\"4\"><v>0</v></c>",
            "<c r=\"H16\" s=\"4\"/>",
            "<c r=\"H18\" s=\"5\"><f>SUM(H14:H16)</f><v>0</v></c>",
            "<c r=\"E21\" s=\"6\"/>",
            "<c r=\"H21\" s=\"4\"><f>H18*E21</f><v>0</v></c>",
            "<c r=\"C25\" s=\"7\"/>",
            "<c r=\"C29\" s=\"3\"><f>+E9</f><v>old</v></c>",
            "<c r=\"G29\" s=\"3\"><f>+E10</f><v>old</v></c>",
        ]
        .join("");
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData><row r="1" ht="21">{}</row><row r="22" ht="21" hidden="1"/><row r="23" ht="21" hidden="1"/><row r="25" ht="43.5"/></sheetData><mergeCells count="3"><mergeCell ref="C17:E17"/><mergeCell ref="C25:H25"/><mergeCell ref="B2:I2"/></mergeCells><drawing r:id="rIdDraw"/></worksheet>"#,
            cells
        )
    }

    fn fixture_drawing(include_ovals: bool, include_lines: bool) -> String {
        let mut names = Vec::new();
        if include_lines {
            names.extend(["Line 1", "Line 2", "Line 3", "Line 4"]);
        }
        if include_ovals {
            names.extend(["Oval 6", "Oval 7", "Oval 8"]);
        }
        let anchors = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                format!(
                    "<xdr:twoCellAnchor><xdr:sp><xdr:nvSpPr><xdr:cNvPr id=\"{}\" name=\"{}\"/></xdr:nvSpPr></xdr:sp><xdr:clientData/></xdr:twoCellAnchor>",
                    index + 1,
                    name
                )
            })
            .collect::<String>();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing">{anchors}</xdr:wsDr>"#
        )
    }

    fn write_fixture(
        path: &Path,
        include_ovals: bool,
        include_lines: bool,
        extra_entry: Option<(&str, &[u8])>,
    ) -> String {
        let sheet = fixture_sheet();
        write_fixture_with_parts(
            path,
            include_ovals,
            include_lines,
            &sheet,
            CONTENT_TYPES,
            WORKBOOK_RELS,
            &[],
            extra_entry,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fixture_with_parts(
        path: &Path,
        include_ovals: bool,
        include_lines: bool,
        sheet: &str,
        content_types: &str,
        workbook_rels: &str,
        additional_entries: &[(&str, &[u8])],
        extra_entry: Option<(&str, &[u8])>,
    ) -> String {
        let file = File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let drawing = fixture_drawing(include_ovals, include_lines);
        let entries = [
            (CONTENT_TYPES_PART, content_types.as_bytes()),
            (WORKBOOK_PART, WORKBOOK.as_bytes()),
            (WORKBOOK_RELS_PART, workbook_rels.as_bytes()),
            ("xl/worksheets/not-sheet1.xml", sheet.as_bytes()),
            (
                "xl/worksheets/_rels/not-sheet1.xml.rels",
                SHEET_RELS.as_bytes(),
            ),
            ("xl/drawings/drawing9.xml", drawing.as_bytes()),
            (CORE_PROPERTIES_PART, CORE.as_bytes()),
        ];
        for (name, bytes) in entries {
            archive.start_file(name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        for (name, bytes) in additional_entries {
            archive.start_file(*name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        if let Some((name, bytes)) = extra_entry {
            archive.start_file(name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        sha256_file(path).unwrap()
    }

    fn read_output(path: &Path) -> Package {
        Package::read(File::open(path).unwrap()).unwrap()
    }

    #[test]
    fn clones_template_by_relationship_and_preserves_contract_structure() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let destination = temp.path().join("result.xlsx");
        let hash = write_fixture(&source, true, true, None);
        let data = fixture_data();

        clone_contract_settlement_template(&source, &hash, &destination, &data).unwrap();

        let package = read_output(&destination);
        validate_generated_package(&package, &data).unwrap();
        let sheet = parse_sheet(package.required("xl/worksheets/not-sheet1.xml").unwrap()).unwrap();
        assert_eq!(sheet.cells["H14"].value.as_deref(), Some("97520.00"));
        assert_eq!(sheet.cells["H18"].formula.as_deref(), Some("SUM(H14:H16)"));
        assert_eq!(sheet.cells["H18"].value.as_deref(), Some("97520.00"));
        assert_eq!(sheet.cells["H21"].value.as_deref(), Some("4876.00"));
        assert_eq!(sheet.merged_ranges.len(), 3);
        assert!(sheet.hidden_rows.is_superset(&BTreeSet::from([22, 23])));
        assert!(sheet.cells["C25"].rich_runs.len() >= 8);

        let drawing =
            drawing_shape_names(package.required("xl/drawings/drawing9.xml").unwrap()).unwrap();
        assert_eq!(
            drawing,
            BTreeSet::from([
                "Line 1".to_string(),
                "Line 2".to_string(),
                "Line 3".to_string(),
                "Line 4".to_string()
            ])
        );
        let workbook =
            String::from_utf8(package.required(WORKBOOK_PART).unwrap().to_vec()).unwrap();
        assert!(!workbook.contains("absPath"));
        assert!(workbook.contains("fullCalcOnLoad=\"1\""));
        let core =
            String::from_utf8(package.required(CORE_PROPERTIES_PART).unwrap().to_vec()).unwrap();
        assert!(!core.contains("creator"));
        assert!(!core.contains("lastModifiedBy"));
        assert!(!core.contains("lastPrinted"));
    }

    #[test]
    fn clones_from_the_same_bytes_used_for_hash_validation() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let destination = temp.path().join("from-bytes.xlsx");
        write_fixture(&source, true, true, None);
        let source = read_file_limited(&source, MAX_XLSX_SOURCE_BYTES).unwrap();
        let data = fixture_data();

        clone_contract_settlement_template_from_bytes(
            &source,
            &sha256_bytes(&source),
            &destination,
            &data,
        )
        .unwrap();

        validate_generated_package(&read_output(&destination), &data).unwrap();
    }

    #[test]
    fn rejects_hash_mismatch_without_output() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let destination = temp.path().join("result.xlsx");
        write_fixture(&source, true, true, None);

        let error = clone_contract_settlement_template(
            &source,
            &"0".repeat(64),
            &destination,
            &fixture_data(),
        )
        .unwrap_err();

        assert_eq!(error.code, "BUSINESS_XLSX_TEMPLATE_HASH_MISMATCH");
        assert!(!destination.exists());
    }

    #[test]
    fn removes_unused_shared_strings_relationship_and_content_type() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("shared-unused.xlsx");
        let destination = temp.path().join("shared-unused-output.xlsx");
        let content_types = CONTENT_TYPES.replace(
            "</Types>",
            r#"<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#,
        );
        let workbook_rels = WORKBOOK_RELS.replace(
            "</Relationships>",
            r#"<Relationship Id="rIdShared" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#,
        );
        let shared_strings = br#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2"><si><t>OLD CUSTOMER SECRET</t></si><si><t>OLD AMOUNT 123456.78</t></si></sst>"#;
        let sheet = fixture_sheet();
        let hash = write_fixture_with_parts(
            &source,
            true,
            true,
            &sheet,
            &content_types,
            &workbook_rels,
            &[(SHARED_STRINGS_PART, shared_strings)],
            None,
        );

        clone_contract_settlement_template(&source, &hash, &destination, &fixture_data()).unwrap();

        let package = read_output(&destination);
        assert!(!package.contains(SHARED_STRINGS_PART));
        assert!(
            !String::from_utf8_lossy(package.required(WORKBOOK_RELS_PART).unwrap())
                .contains("sharedStrings")
        );
        assert!(
            !String::from_utf8_lossy(package.required(CONTENT_TYPES_PART).unwrap())
                .contains("sharedStrings")
        );
        assert!(package.entries.iter().all(|entry| {
            !String::from_utf8_lossy(&entry.bytes).contains("OLD CUSTOMER SECRET")
                && !String::from_utf8_lossy(&entry.bytes).contains("OLD AMOUNT 123456.78")
        }));
    }

    #[test]
    fn compacts_referenced_shared_strings_and_remaps_indexes() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("shared-referenced.xlsx");
        let destination = temp.path().join("shared-referenced-output.xlsx");
        let content_types = CONTENT_TYPES.replace(
            "</Types>",
            r#"<Override PartName="/xl/worksheets/other.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#,
        );
        let workbook_rels = WORKBOOK_RELS.replace(
            "</Relationships>",
            r#"<Relationship Id="rIdShared" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#,
        );
        let other_sheet = br#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>1</v></c></row></sheetData></worksheet>"#;
        let shared_strings = br#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2"><si><t>OLD CUSTOMER SECRET</t></si><si><t>SAFE RETAINED LABEL</t></si></sst>"#;
        let sheet = fixture_sheet();
        let hash = write_fixture_with_parts(
            &source,
            true,
            true,
            &sheet,
            &content_types,
            &workbook_rels,
            &[
                ("xl/worksheets/other.xml", other_sheet),
                (SHARED_STRINGS_PART, shared_strings),
            ],
            None,
        );

        clone_contract_settlement_template(&source, &hash, &destination, &fixture_data()).unwrap();

        let package = read_output(&destination);
        let compacted = String::from_utf8_lossy(package.required(SHARED_STRINGS_PART).unwrap());
        assert!(compacted.contains("SAFE RETAINED LABEL"));
        assert!(!compacted.contains("OLD CUSTOMER SECRET"));
        assert!(compacted.contains("count=\"1\""));
        assert!(compacted.contains("uniqueCount=\"1\""));
        let other = String::from_utf8_lossy(package.required("xl/worksheets/other.xml").unwrap());
        assert!(other.contains("<v>0</v>"));
    }

    #[test]
    fn rejects_nonempty_h16_before_generation() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("nonempty-h16.xlsx");
        let destination = temp.path().join("nonempty-h16-output.xlsx");
        let sheet = fixture_sheet().replace(
            "<c r=\"H16\" s=\"4\"/>",
            "<c r=\"H16\" s=\"4\"><v>1</v></c>",
        );
        let hash = write_fixture_with_parts(
            &source,
            true,
            true,
            &sheet,
            CONTENT_TYPES,
            WORKBOOK_RELS,
            &[],
            None,
        );

        let error =
            clone_contract_settlement_template(&source, &hash, &destination, &fixture_data())
                .unwrap_err();

        assert_eq!(error.code, "BUSINESS_XLSX_TEMPLATE_MISMATCH");
        assert!(error.message.contains("H16"));
        assert!(!destination.exists());
    }

    #[test]
    fn rejects_non_xlsx_zip_and_macro_or_external_parts() {
        let temp = TempDir::new().unwrap();
        let plain_zip = temp.path().join("plain.zip");
        {
            let mut archive = ZipWriter::new(File::create(&plain_zip).unwrap());
            archive
                .start_file("file.txt", SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"not xlsx").unwrap();
            archive.finish().unwrap();
        }
        let destination = temp.path().join("plain-output.xlsx");
        let error = clone_contract_settlement_template(
            &plain_zip,
            &sha256_file(&plain_zip).unwrap(),
            &destination,
            &fixture_data(),
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_INVALID");
        assert!(!destination.exists());

        let macro_source = temp.path().join("macro.xlsx");
        let macro_hash = write_fixture(
            &macro_source,
            true,
            true,
            Some(("xl/vbaProject.bin", b"macro")),
        );
        let macro_destination = temp.path().join("macro-output.xlsx");
        let error = clone_contract_settlement_template(
            &macro_source,
            &macro_hash,
            &macro_destination,
            &fixture_data(),
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_UNSAFE");
        assert!(!macro_destination.exists());

        let external_source = temp.path().join("external.xlsx");
        let external_relationship = br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdExternal" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid/" TargetMode="External"/></Relationships>"#;
        let external_hash = write_fixture(
            &external_source,
            true,
            true,
            Some(("xl/drawings/_rels/drawing9.xml.rels", external_relationship)),
        );
        let external_destination = temp.path().join("external-output.xlsx");
        let error = clone_contract_settlement_template(
            &external_source,
            &external_hash,
            &external_destination,
            &fixture_data(),
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_UNSAFE");
        assert!(!external_destination.exists());
    }

    #[test]
    fn rejects_amount_and_uppercase_mismatch_and_fractional_amount() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let hash = write_fixture(&source, true, true, None);

        let mut amount_mismatch = fixture_data();
        amount_mismatch.final_settlement_amount_cents += 100;
        let error = clone_contract_settlement_template(
            &source,
            &hash,
            &temp.path().join("amount.xlsx"),
            &amount_mismatch,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_AMOUNT_MISMATCH");

        let mut uppercase_mismatch = fixture_data();
        uppercase_mismatch.final_settlement_amount_uppercase_cny = "壹元整".to_string();
        let error = clone_contract_settlement_template(
            &source,
            &hash,
            &temp.path().join("uppercase.xlsx"),
            &uppercase_mismatch,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_AMOUNT_MISMATCH");

        let mut fractional = fixture_data();
        fractional.original_contract_amount_cents += 50;
        fractional.final_settlement_amount_cents += 50;
        let error = clone_contract_settlement_template(
            &source,
            &hash,
            &temp.path().join("fractional.xlsx"),
            &fractional,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_FRACTIONAL_CNY_UNSUPPORTED");
    }

    #[test]
    fn rejects_same_path_and_existing_destination() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let hash = write_fixture(&source, true, true, None);
        let error = clone_contract_settlement_template(&source, &hash, &source, &fixture_data())
            .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_DESTINATION_INVALID");

        let destination = temp.path().join("existing.xlsx");
        fs::write(&destination, b"existing").unwrap();
        let error =
            clone_contract_settlement_template(&source, &hash, &destination, &fixture_data())
                .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_DESTINATION_EXISTS");
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
    }

    #[test]
    fn structural_failure_is_atomic() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let destination = temp.path().join("result.xlsx");
        let hash = write_fixture(&source, false, true, None);

        let error =
            clone_contract_settlement_template(&source, &hash, &destination, &fixture_data())
                .unwrap_err();

        assert_eq!(error.code, "BUSINESS_XLSX_TEMPLATE_MISMATCH");
        assert!(!destination.exists());
        assert!(temp
            .path()
            .read_dir()
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().starts_with(".tmp")));

        let missing_lines_source = temp.path().join("missing-lines.xlsx");
        let missing_lines_destination = temp.path().join("missing-lines-output.xlsx");
        let hash = write_fixture(&missing_lines_source, true, false, None);
        let error = clone_contract_settlement_template(
            &missing_lines_source,
            &hash,
            &missing_lines_destination,
            &fixture_data(),
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_OUTPUT_INVALID");
        assert!(!missing_lines_destination.exists());
    }

    #[test]
    fn package_reader_rejects_path_escape_entry() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut archive = ZipWriter::new(&mut bytes);
            archive
                .start_file("../escape.xml", SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"escape").unwrap();
            archive.finish().unwrap();
        }
        bytes.set_position(0);
        let error = Package::read(bytes).unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_UNSAFE");
    }

    #[test]
    fn package_reader_enforces_entry_count_and_compression_ratio_limits() {
        let mut too_many = Cursor::new(Vec::new());
        {
            let mut archive = ZipWriter::new(&mut too_many);
            for index in 0..=MAX_ZIP_ENTRIES {
                archive
                    .start_file(
                        format!("entry-{index}.xml"),
                        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
                    )
                    .unwrap();
            }
            archive.finish().unwrap();
        }
        too_many.set_position(0);
        let error = Package::read(too_many).unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_LIMIT_EXCEEDED");

        let mut high_ratio = Cursor::new(Vec::new());
        {
            let mut archive = ZipWriter::new(&mut high_ratio);
            archive
                .start_file(
                    "high-ratio.xml",
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .unwrap();
            archive.write_all(&vec![b'A'; 512 * 1024]).unwrap();
            archive.finish().unwrap();
        }
        high_ratio.set_position(0);
        let error = Package::read(high_ratio).unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_LIMIT_EXCEEDED");
        assert!(error.message.contains("compression ratio"));
    }

    #[test]
    fn zip_limit_validation_rejects_single_and_total_expansion() {
        let mut total = 0;
        let error = validate_zip_entry_limits(
            "oversized.xml",
            MAX_ZIP_ENTRY_BYTES + 1,
            MAX_ZIP_ENTRY_BYTES + 1,
            &mut total,
        )
        .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_LIMIT_EXCEEDED");

        let mut total = MAX_ZIP_TOTAL_BYTES;
        let error = validate_zip_entry_limits("extra.xml", 1, 1, &mut total).unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_PACKAGE_LIMIT_EXCEEDED");
    }

    #[test]
    fn publish_fallback_does_not_overwrite_and_cleans_staging() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("fallback.xlsx");
        let mut staged = StagedOutput::create(temp.path(), &destination).unwrap();
        let staged_path = staged.path.clone();
        staged.file.write_all(b"fallback payload").unwrap();
        staged.file.sync_all().unwrap();

        persist_staged_without_overwrite(&mut staged, &destination, |_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "hard links unsupported",
            ))
        })
        .unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"fallback payload");
        assert!(!staged_path.exists());

        let second_destination = temp.path().join("existing-fallback.xlsx");
        fs::write(&second_destination, b"existing").unwrap();
        let mut second_staged = StagedOutput::create(temp.path(), &second_destination).unwrap();
        second_staged.file.write_all(b"replacement").unwrap();
        second_staged.file.sync_all().unwrap();
        let error =
            persist_staged_without_overwrite(&mut second_staged, &second_destination, |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "hard links unsupported",
                ))
            })
            .unwrap_err();
        assert_eq!(error.code, "BUSINESS_XLSX_DESTINATION_EXISTS");
        assert_eq!(fs::read(&second_destination).unwrap(), b"existing");
    }

    #[test]
    fn uppercase_cny_and_decimal_formatting_are_exact() {
        assert_eq!(uppercase_cny(0).unwrap(), "零元整");
        assert_eq!(uppercase_cny(9_752_000).unwrap(), "玖万柒仟伍佰贰拾元整");
        assert_eq!(uppercase_cny(100_010_000).unwrap(), "壹佰万零壹佰元整");
        assert_eq!(decimal_from_cents(-50), "-0.50");
        assert_eq!(decimal_from_bps(500), "0.05");
        assert_eq!(decimal_from_bps(10_000), "1");
    }

    #[test]
    fn output_is_a_readable_zip_after_atomic_persist() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.xlsx");
        let destination = temp.path().join("result.xlsx");
        let hash = write_fixture(&source, true, true, None);
        clone_contract_settlement_template(&source, &hash, &destination, &fixture_data()).unwrap();

        let mut archive = ZipArchive::new(File::open(destination).unwrap()).unwrap();
        let mut workbook = String::new();
        archive
            .by_name(WORKBOOK_PART)
            .unwrap()
            .read_to_string(&mut workbook)
            .unwrap();
        assert!(workbook.contains(OUTPUT_SHEET_NAME));
    }

    #[test]
    #[ignore = "requires the registered real contract settlement template"]
    fn real_template_read_only_regression_when_present() {
        const TEMPLATE_PATH_ENV: &str = "BSAIGC_CONTRACT_SETTLEMENT_TEMPLATE";
        let configured_source = std::env::var_os(TEMPLATE_PATH_ENV).map(PathBuf::from);
        let source = configured_source.clone().unwrap_or_else(|| {
            PathBuf::from(
                "tests/fixtures/synthetic/business-v1/templates/synthetic-contract-settlement.xlsx",
            )
        });
        if !source.is_file() {
            assert!(
                configured_source.is_none(),
                "configured template path from {TEMPLATE_PATH_ENV} does not exist: {}",
                source.display()
            );
            eprintln!(
                "real template is unavailable; set {TEMPLATE_PATH_ENV} to execute this ignored regression"
            );
            return;
        }
        assert_eq!(
            sha256_file(&source).unwrap(),
            CONTRACT_SETTLEMENT_TEMPLATE_SHA256
        );
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("real-template-regression.xlsx");
        let mut data = fixture_data();
        data.retention_rate_bps = None;

        let source = read_file_limited(&source, MAX_XLSX_SOURCE_BYTES).unwrap();
        clone_contract_settlement_template_from_bytes(
            &source,
            CONTRACT_SETTLEMENT_TEMPLATE_SHA256,
            &destination,
            &data,
        )
        .unwrap();

        validate_generated_package(&read_output(&destination), &data).unwrap();
    }
}

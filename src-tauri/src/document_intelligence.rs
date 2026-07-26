use crate::protocol::{
    ContractReviewFailure, ContractReviewStage, DocumentBlockKind, DocumentBlockRecord,
    DocumentExtractionRecord, DocumentExtractionStatus, DocumentPageRecord, DocumentTableRecord,
    EvidenceBoundingBox, HostError, OcrProvenance, ParserProvenance,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

pub const TEXT_PDF_PARSER_NAME: &str = "pdf-extract";
pub const TEXT_PDF_PARSER_VERSION: &str = "0.12.0";
pub const DOCX_PARSER_NAME: &str = "ooxml-docx";
pub const DOCX_PARSER_VERSION: &str = "1.0.0";
const DOCX_MIME_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const MIN_SEARCHABLE_TEXT_CHARS: usize = 24;
const DETERMINISTIC_UUID_DOMAIN: &str = "bsaigc.document-intelligence.uuid.v1";
const MAX_OCR_PAGES: u32 = 120;
const MAX_OCR_PIXELS_PER_PAGE: u64 = 8_000_000;
const MAX_OCR_TOTAL_PIXELS: u64 = 120_000_000;
const MAX_OCR_TEXT_CHARS: usize = 2_000_000;
const MAX_OCR_BLOCKS: usize = 50_000;
const MAX_OCR_RENDER_DIMENSION: u32 = 2_600;
const PREFERRED_OCR_RENDER_SCALE: f64 = 2.0;

#[derive(Debug, Clone, PartialEq)]
struct LocalOcrLine {
    text: String,
    bbox: Option<EvidenceBoundingBox>,
}

#[derive(Debug, Clone, PartialEq)]
struct LocalOcrPage {
    width: u32,
    height: u32,
    lines: Vec<LocalOcrLine>,
}

#[derive(Debug, Clone, PartialEq)]
struct LocalOcrOutput {
    engine: String,
    version: String,
    language: String,
    pages: Vec<LocalOcrPage>,
}

pub trait DocumentParser: Send + Sync {
    fn parser_name(&self) -> &'static str;
    fn parser_version(&self) -> &'static str;
    fn supports(&self, mime_type: &str, source_path: &Path) -> bool;
    fn extract(
        &self,
        review_id: &str,
        source_asset_id: &str,
        source_asset_sha256: &str,
        source_path: &Path,
        now: i64,
    ) -> Result<DocumentExtractionRecord, HostError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TextPdfParser;

impl DocumentParser for TextPdfParser {
    fn parser_name(&self) -> &'static str {
        TEXT_PDF_PARSER_NAME
    }

    fn parser_version(&self) -> &'static str {
        TEXT_PDF_PARSER_VERSION
    }

    fn supports(&self, mime_type: &str, source_path: &Path) -> bool {
        supports_extension_or_mime(source_path, "pdf", mime_type, &["application/pdf"])
    }

    fn extract(
        &self,
        review_id: &str,
        source_asset_id: &str,
        source_asset_sha256: &str,
        source_path: &Path,
        now: i64,
    ) -> Result<DocumentExtractionRecord, HostError> {
        validate_source(review_id, source_asset_id, source_asset_sha256, source_path)?;
        let text = pdf_extract::extract_text(source_path).map_err(|error| {
            HostError::new(
                "DOCUMENT_EXTRACTION_FAILED",
                format!("unable to extract searchable PDF text: {error}"),
                true,
            )
        })?;
        Ok(build_extraction_from_text(
            review_id,
            source_asset_id,
            source_asset_sha256,
            self.parser_name(),
            self.parser_version(),
            &text,
            now,
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DocxParser;

impl DocumentParser for DocxParser {
    fn parser_name(&self) -> &'static str {
        DOCX_PARSER_NAME
    }

    fn parser_version(&self) -> &'static str {
        DOCX_PARSER_VERSION
    }

    fn supports(&self, mime_type: &str, source_path: &Path) -> bool {
        supports_extension_or_mime(source_path, "docx", mime_type, &[DOCX_MIME_TYPE])
    }

    fn extract(
        &self,
        review_id: &str,
        source_asset_id: &str,
        source_asset_sha256: &str,
        source_path: &Path,
        now: i64,
    ) -> Result<DocumentExtractionRecord, HostError> {
        validate_source(review_id, source_asset_id, source_asset_sha256, source_path)?;
        let source = fs::File::open(source_path).map_err(|error| {
            HostError::new(
                "DOCUMENT_SOURCE_UNAVAILABLE",
                format!("unable to open DOCX source: {error}"),
                true,
            )
        })?;
        let mut archive = zip::ZipArchive::new(source).map_err(|error| {
            HostError::new(
                "DOCX_PACKAGE_INVALID",
                format!("unable to open DOCX package: {error}"),
                false,
            )
        })?;
        let mut document_xml = String::new();
        {
            let mut entry = archive.by_name("word/document.xml").map_err(|error| {
                HostError::new(
                    "DOCX_DOCUMENT_XML_MISSING",
                    format!("DOCX package does not contain word/document.xml: {error}"),
                    false,
                )
            })?;
            entry.read_to_string(&mut document_xml).map_err(|error| {
                HostError::new(
                    "DOCX_DOCUMENT_XML_INVALID",
                    format!("unable to read word/document.xml: {error}"),
                    false,
                )
            })?;
        }
        let flow = parse_docx_document_xml(&document_xml)?;
        Ok(build_docx_extraction(
            review_id,
            source_asset_id,
            source_asset_sha256,
            self.parser_name(),
            self.parser_version(),
            flow,
            now,
        ))
    }
}

#[derive(Default)]
pub struct DocumentIntelligence {
    parsers: Vec<Box<dyn DocumentParser>>,
}

impl DocumentIntelligence {
    pub fn with_defaults() -> Self {
        Self {
            parsers: vec![Box::new(TextPdfParser), Box::new(DocxParser)],
        }
    }

    pub fn extract(
        &self,
        review_id: &str,
        source_asset_id: &str,
        source_asset_sha256: &str,
        mime_type: &str,
        source_path: &Path,
        now: i64,
    ) -> Result<DocumentExtractionRecord, HostError> {
        self.extract_with_cancel(
            review_id,
            source_asset_id,
            source_asset_sha256,
            mime_type,
            source_path,
            now,
            || Ok(false),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn extract_with_cancel<F>(
        &self,
        review_id: &str,
        source_asset_id: &str,
        source_asset_sha256: &str,
        mime_type: &str,
        source_path: &Path,
        now: i64,
        mut is_cancelled: F,
    ) -> Result<DocumentExtractionRecord, HostError>
    where
        F: FnMut() -> Result<bool, HostError>,
    {
        if check_cancelled(&mut is_cancelled)? {
            return Err(document_extraction_cancelled(
                "document extraction was cancelled before parsing started",
            ));
        }
        let parser = self
            .parsers
            .iter()
            .find(|parser| parser.supports(mime_type, source_path))
            .ok_or_else(|| {
                HostError::new(
                    "DOCUMENT_FORMAT_UNSUPPORTED",
                    "no registered parser supports this contract document",
                    false,
                )
            })?;
        let mut extraction = parser.extract(
            review_id,
            source_asset_id,
            source_asset_sha256,
            source_path,
            now,
        )?;
        if check_cancelled(&mut is_cancelled)? {
            mark_extraction_cancelled(&mut extraction, now);
            return Ok(extraction);
        }
        if extraction.status != DocumentExtractionStatus::AwaitingOcr {
            return Ok(extraction);
        }

        if check_cancelled(&mut is_cancelled)? {
            mark_extraction_cancelled(&mut extraction, now);
            return Ok(extraction);
        }

        let output = match run_local_pdf_ocr(source_path, &mut is_cancelled) {
            Ok(output) => output,
            Err(error) if error.code == "DOCUMENT_EXTRACTION_CANCELLED" => {
                mark_extraction_cancelled(&mut extraction, now);
                return Ok(extraction);
            }
            Err(error) => return Err(error),
        };
        if check_cancelled(&mut is_cancelled)? {
            mark_extraction_cancelled(&mut extraction, now);
            return Ok(extraction);
        }
        match apply_local_ocr_output(&mut extraction, output, now, &mut is_cancelled) {
            Ok(()) => Ok(extraction),
            Err(error) if error.code == "DOCUMENT_EXTRACTION_CANCELLED" => {
                mark_extraction_cancelled(&mut extraction, now);
                Ok(extraction)
            }
            Err(error) => Err(error),
        }
    }
}

fn check_cancelled<F>(is_cancelled: &mut F) -> Result<bool, HostError>
where
    F: FnMut() -> Result<bool, HostError>,
{
    is_cancelled()
}

fn document_extraction_cancelled(message: impl Into<String>) -> HostError {
    HostError::new("DOCUMENT_EXTRACTION_CANCELLED", message, false)
}

fn ensure_extraction_not_cancelled<F>(is_cancelled: &mut F) -> Result<(), HostError>
where
    F: FnMut() -> Result<bool, HostError>,
{
    if check_cancelled(is_cancelled)? {
        Err(document_extraction_cancelled(
            "document extraction was cancelled while applying local OCR output",
        ))
    } else {
        Ok(())
    }
}

fn mark_extraction_cancelled(extraction: &mut DocumentExtractionRecord, now: i64) {
    extraction.status = DocumentExtractionStatus::Cancelled;
    extraction.ocr = None;
    extraction.completed_at = Some(now);
    extraction.failure = Some(ContractReviewFailure {
        code: "DOCUMENT_EXTRACTION_CANCELLED".to_string(),
        message: "document extraction was cancelled before local OCR completed".to_string(),
        retryable: false,
        stage: ContractReviewStage::Extracting,
    });
}

fn apply_local_ocr_output<F>(
    extraction: &mut DocumentExtractionRecord,
    output: LocalOcrOutput,
    now: i64,
    is_cancelled: &mut F,
) -> Result<(), HostError>
where
    F: FnMut() -> Result<bool, HostError>,
{
    ensure_extraction_not_cancelled(is_cancelled)?;
    let page_count = u32::try_from(output.pages.len()).map_err(|_| {
        HostError::new(
            "OCR_PAGE_LIMIT_EXCEEDED",
            "local OCR page count cannot be represented safely",
            false,
        )
    })?;
    if page_count == 0 {
        return Err(HostError::new(
            "OCR_NO_PAGES",
            "local OCR could not find any PDF pages to process",
            false,
        ));
    }
    if page_count > MAX_OCR_PAGES {
        return Err(HostError::new(
            "OCR_PAGE_LIMIT_EXCEEDED",
            format!(
                "local OCR is limited to {MAX_OCR_PAGES} pages; document contains {page_count}"
            ),
            false,
        ));
    }

    let mut total_pixels = 0_u64;
    let mut total_text_chars = 0_usize;
    let mut total_blocks = 0_usize;
    let mut pages = Vec::with_capacity(output.pages.len());
    let mut blocks = Vec::new();
    let mut full_text_pages = Vec::with_capacity(output.pages.len());

    for (page_index, output_page) in output.pages.into_iter().enumerate() {
        ensure_extraction_not_cancelled(is_cancelled)?;
        if output_page.width == 0 || output_page.height == 0 {
            return Err(HostError::new(
                "OCR_IMAGE_DECODE_FAILED",
                format!(
                    "local OCR produced an empty bitmap for page {}",
                    page_index + 1
                ),
                true,
            ));
        }
        let page_pixels = u64::from(output_page.width)
            .checked_mul(u64::from(output_page.height))
            .ok_or_else(|| {
                HostError::new(
                    "OCR_PIXEL_LIMIT_EXCEEDED",
                    format!("page {} pixel dimensions overflowed", page_index + 1),
                    false,
                )
            })?;
        if page_pixels > MAX_OCR_PIXELS_PER_PAGE {
            return Err(HostError::new(
                "OCR_PIXEL_LIMIT_EXCEEDED",
                format!(
                    "page {} contains {page_pixels} rendered pixels; limit is {MAX_OCR_PIXELS_PER_PAGE}",
                    page_index + 1
                ),
                false,
            ));
        }
        total_pixels = total_pixels.checked_add(page_pixels).ok_or_else(|| {
            HostError::new(
                "OCR_PIXEL_LIMIT_EXCEEDED",
                "local OCR total pixel count overflowed",
                false,
            )
        })?;
        if total_pixels > MAX_OCR_TOTAL_PIXELS {
            return Err(HostError::new(
                "OCR_PIXEL_LIMIT_EXCEEDED",
                format!(
                    "document requires {total_pixels} rendered pixels; limit is {MAX_OCR_TOTAL_PIXELS}"
                ),
                false,
            ));
        }

        let page_index_i64 = page_index as i64;
        let page_index_key = page_index_i64.to_string();
        let page_id =
            deterministic_uuid("page", &[extraction.id.as_str(), page_index_key.as_str()]);
        let mut page_text = String::new();
        let mut page_blocks = Vec::new();
        for output_line in output_page.lines {
            ensure_extraction_not_cancelled(is_cancelled)?;
            let line_text = normalize_ocr_line(&output_line.text);
            if line_text.is_empty() {
                continue;
            }
            if !page_text.is_empty() {
                page_text.push('\n');
            }
            let char_start = page_text.chars().count();
            page_text.push_str(&line_text);
            let char_end = page_text.chars().count();
            total_text_chars = total_text_chars
                .checked_add(line_text.chars().count())
                .ok_or_else(|| {
                    HostError::new(
                        "OCR_TEXT_LIMIT_EXCEEDED",
                        "local OCR text size overflowed",
                        false,
                    )
                })?;
            if total_text_chars > MAX_OCR_TEXT_CHARS {
                return Err(HostError::new(
                    "OCR_TEXT_LIMIT_EXCEEDED",
                    format!("local OCR text exceeds the {MAX_OCR_TEXT_CHARS} character limit"),
                    false,
                ));
            }
            total_blocks += 1;
            if total_blocks > MAX_OCR_BLOCKS {
                return Err(HostError::new(
                    "OCR_BLOCK_LIMIT_EXCEEDED",
                    format!("local OCR exceeds the {MAX_OCR_BLOCKS} block limit"),
                    false,
                ));
            }
            let order_index = page_blocks.len() as i64;
            let order_index_key = order_index.to_string();
            page_blocks.push(DocumentBlockRecord {
                id: deterministic_uuid(
                    "block",
                    &[
                        extraction.id.as_str(),
                        page_id.as_str(),
                        order_index_key.as_str(),
                    ],
                ),
                extraction_id: extraction.id.clone(),
                page_id: page_id.clone(),
                page_index: page_index_i64,
                order_index,
                kind: infer_block_kind(&line_text),
                text: line_text,
                char_start: char_start as i64,
                char_end: char_end as i64,
                bbox: output_line
                    .bbox
                    .and_then(|bbox| clamp_ocr_bbox(bbox, output_page.width, output_page.height)),
            });
        }

        pages.push(DocumentPageRecord {
            id: page_id,
            extraction_id: extraction.id.clone(),
            page_index: page_index_i64,
            text: page_text.clone(),
            text_sha256: sha256_text(&page_text),
            width: Some(f64::from(output_page.width)),
            height: Some(f64::from(output_page.height)),
            preview_asset_id: None,
        });
        blocks.extend(page_blocks);
        full_text_pages.push(page_text);
    }

    if total_text_chars == 0 || blocks.is_empty() {
        return Err(HostError::new(
            "OCR_NO_TEXT_RECOGNIZED",
            "Windows local OCR completed but did not recognize any text; retry after checking scan quality or installed OCR languages",
            true,
        ));
    }

    ensure_extraction_not_cancelled(is_cancelled)?;
    let content = full_text_pages.join("\u{000c}");
    extraction.parser.mode = "windowsPdfRenderOcr".to_string();
    extraction.ocr = Some(OcrProvenance {
        engine: output.engine,
        version: output.version,
        language: output.language,
    });
    extraction.status = DocumentExtractionStatus::Completed;
    extraction.page_count = pages.len() as i64;
    extraction.content_sha256 = Some(sha256_text(&content));
    extraction.pages = pages;
    extraction.blocks = blocks;
    extraction.tables.clear();
    extraction.completed_at = Some(now);
    extraction.failure = None;
    Ok(())
}

fn normalize_ocr_line(value: &str) -> String {
    value
        .replace(['\r', '\n', '\u{0000}'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn clamp_ocr_bbox(
    bbox: EvidenceBoundingBox,
    page_width: u32,
    page_height: u32,
) -> Option<EvidenceBoundingBox> {
    if !bbox.x.is_finite()
        || !bbox.y.is_finite()
        || !bbox.width.is_finite()
        || !bbox.height.is_finite()
        || bbox.width <= 0.0
        || bbox.height <= 0.0
    {
        return None;
    }
    let max_x = f64::from(page_width);
    let max_y = f64::from(page_height);
    let left = bbox.x.max(0.0).min(max_x);
    let top = bbox.y.max(0.0).min(max_y);
    let right = (bbox.x + bbox.width).max(0.0).min(max_x);
    let bottom = (bbox.y + bbox.height).max(0.0).min(max_y);
    if right <= left || bottom <= top {
        None
    } else {
        Some(EvidenceBoundingBox {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }
}

#[cfg(not(windows))]
fn run_local_pdf_ocr<F>(
    _source_path: &Path,
    is_cancelled: &mut F,
) -> Result<LocalOcrOutput, HostError>
where
    F: FnMut() -> Result<bool, HostError>,
{
    if check_cancelled(is_cancelled)? {
        return Err(HostError::new(
            "DOCUMENT_EXTRACTION_CANCELLED",
            "document extraction was cancelled before local OCR started",
            false,
        ));
    }
    Err(HostError::new(
        "LOCAL_OCR_UNSUPPORTED",
        "scanned PDF OCR is only available in the Windows desktop host",
        false,
    ))
}

#[cfg(windows)]
fn run_local_pdf_ocr<F>(
    source_path: &Path,
    is_cancelled: &mut F,
) -> Result<LocalOcrOutput, HostError>
where
    F: FnMut() -> Result<bool, HostError>,
{
    windows_ocr::run(source_path, is_cancelled)
}

#[cfg(windows)]
mod windows_ocr {
    use super::*;
    use std::path::PathBuf;
    use windows::core::{Error as WindowsError, HSTRING};
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, SoftwareBitmap,
    };
    use windows::Media::Ocr::{OcrEngine, OcrLine};
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::InMemoryRandomAccessStream;
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
    use windows::UI::Color;

    struct WinRtApartmentGuard {
        uninitialize: bool,
    }

    impl Drop for WinRtApartmentGuard {
        fn drop(&mut self) {
            if self.uninitialize {
                unsafe { RoUninitialize() };
            }
        }
    }

    pub(super) fn run<F>(
        source_path: &Path,
        is_cancelled: &mut F,
    ) -> Result<LocalOcrOutput, HostError>
    where
        F: FnMut() -> Result<bool, HostError>,
    {
        ensure_not_cancelled(is_cancelled)?;
        let _apartment = initialize_winrt()?;
        let absolute_path = absolute_source_path(source_path)?;
        let path_string = absolute_path.to_string_lossy().to_string();
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path_string))
            .map_err(|error| {
                windows_error(
                    "OCR_PDF_OPEN_FAILED",
                    "unable to start opening the scanned PDF",
                    error,
                    true,
                )
            })?
            .get()
            .map_err(|error| {
                windows_error(
                    "OCR_PDF_OPEN_FAILED",
                    "unable to open the scanned PDF",
                    error,
                    true,
                )
            })?;
        ensure_not_cancelled(is_cancelled)?;
        let document = PdfDocument::LoadFromFileAsync(&file)
            .map_err(|error| {
                windows_error(
                    "OCR_PDF_OPEN_FAILED",
                    "unable to start loading the scanned PDF",
                    error,
                    true,
                )
            })?
            .get()
            .map_err(|error| {
                windows_error(
                    "OCR_PDF_OPEN_FAILED",
                    "unable to load the scanned PDF",
                    error,
                    true,
                )
            })?;
        let page_count = document.PageCount().map_err(|error| {
            windows_error(
                "OCR_PDF_OPEN_FAILED",
                "unable to read the scanned PDF page count",
                error,
                true,
            )
        })?;
        if page_count == 0 {
            return Err(HostError::new(
                "OCR_NO_PAGES",
                "scanned PDF does not contain any pages",
                false,
            ));
        }
        if page_count > MAX_OCR_PAGES {
            return Err(HostError::new(
                "OCR_PAGE_LIMIT_EXCEEDED",
                format!(
                    "local OCR is limited to {MAX_OCR_PAGES} pages; document contains {page_count}"
                ),
                false,
            ));
        }

        let (engine, language) = create_ocr_engine()?;
        let engine_max_dimension = OcrEngine::MaxImageDimension().map_err(|error| {
            windows_error(
                "OCR_ENGINE_UNAVAILABLE",
                "unable to query the Windows OCR image limit",
                error,
                true,
            )
        })?;
        let render_max_dimension = engine_max_dimension.min(MAX_OCR_RENDER_DIMENSION);
        if render_max_dimension == 0 {
            return Err(HostError::new(
                "OCR_ENGINE_UNAVAILABLE",
                "Windows OCR reported an invalid maximum image dimension",
                true,
            ));
        }

        let mut pages = Vec::with_capacity(page_count as usize);
        let mut total_pixels = 0_u64;
        let mut recognized_chars = 0_usize;
        let mut recognized_blocks = 0_usize;
        for page_index in 0..page_count {
            ensure_not_cancelled(is_cancelled)?;
            let page = document.GetPage(page_index).map_err(|error| {
                windows_error(
                    "OCR_PDF_RENDER_FAILED",
                    &format!("unable to access scanned PDF page {}", page_index + 1),
                    error,
                    true,
                )
            })?;
            let page_size = page.Size().map_err(|error| {
                windows_error(
                    "OCR_PDF_RENDER_FAILED",
                    &format!("unable to read scanned PDF page {} size", page_index + 1),
                    error,
                    true,
                )
            })?;
            let (requested_width, requested_height) =
                render_dimensions(page_size.Width, page_size.Height, render_max_dimension)?;
            let requested_pixels = u64::from(requested_width) * u64::from(requested_height);
            total_pixels = total_pixels.checked_add(requested_pixels).ok_or_else(|| {
                HostError::new(
                    "OCR_PIXEL_LIMIT_EXCEEDED",
                    "local OCR total pixel count overflowed",
                    false,
                )
            })?;
            if total_pixels > MAX_OCR_TOTAL_PIXELS {
                return Err(HostError::new(
                    "OCR_PIXEL_LIMIT_EXCEEDED",
                    format!("document requires more than {MAX_OCR_TOTAL_PIXELS} rendered pixels"),
                    false,
                ));
            }

            let render_options = PdfPageRenderOptions::new().map_err(|error| {
                windows_error(
                    "OCR_PDF_RENDER_FAILED",
                    "unable to create PDF render options",
                    error,
                    true,
                )
            })?;
            render_options
                .SetDestinationWidth(requested_width)
                .and_then(|_| render_options.SetDestinationHeight(requested_height))
                .and_then(|_| {
                    render_options.SetBackgroundColor(Color {
                        A: 255,
                        R: 255,
                        G: 255,
                        B: 255,
                    })
                })
                .map_err(|error| {
                    windows_error(
                        "OCR_PDF_RENDER_FAILED",
                        &format!(
                            "unable to configure scanned PDF page {} render",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?;
            let stream = InMemoryRandomAccessStream::new().map_err(|error| {
                windows_error(
                    "OCR_PDF_RENDER_FAILED",
                    "unable to allocate an in-memory PDF render stream",
                    error,
                    true,
                )
            })?;
            page.RenderWithOptionsToStreamAsync(&stream, &render_options)
                .map_err(|error| {
                    windows_error(
                        "OCR_PDF_RENDER_FAILED",
                        &format!(
                            "unable to start rendering scanned PDF page {}",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?
                .get()
                .map_err(|error| {
                    windows_error(
                        "OCR_PDF_RENDER_FAILED",
                        &format!("unable to render scanned PDF page {}", page_index + 1),
                        error,
                        true,
                    )
                })?;
            ensure_not_cancelled(is_cancelled)?;
            stream.Seek(0).map_err(|error| {
                windows_error(
                    "OCR_IMAGE_DECODE_FAILED",
                    "unable to rewind the rendered PDF image",
                    error,
                    true,
                )
            })?;
            let decoder = BitmapDecoder::CreateAsync(&stream)
                .map_err(|error| {
                    windows_error(
                        "OCR_IMAGE_DECODE_FAILED",
                        &format!(
                            "unable to start decoding scanned PDF page {}",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?
                .get()
                .map_err(|error| {
                    windows_error(
                        "OCR_IMAGE_DECODE_FAILED",
                        &format!("unable to decode scanned PDF page {}", page_index + 1),
                        error,
                        true,
                    )
                })?;
            let bitmap = decoder
                .GetSoftwareBitmapConvertedAsync(
                    BitmapPixelFormat::Bgra8,
                    BitmapAlphaMode::Premultiplied,
                )
                .map_err(|error| {
                    windows_error(
                        "OCR_IMAGE_DECODE_FAILED",
                        &format!(
                            "unable to convert scanned PDF page {} bitmap",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?
                .get()
                .map_err(|error| {
                    windows_error(
                        "OCR_IMAGE_DECODE_FAILED",
                        &format!(
                            "unable to convert scanned PDF page {} bitmap",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?;
            ensure_not_cancelled(is_cancelled)?;
            let (bitmap_width, bitmap_height) = bitmap_dimensions(&bitmap, page_index)?;
            let result = engine
                .RecognizeAsync(&bitmap)
                .map_err(|error| {
                    windows_error(
                        "OCR_RECOGNITION_FAILED",
                        &format!(
                            "unable to start OCR for scanned PDF page {}",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?
                .get()
                .map_err(|error| {
                    windows_error(
                        "OCR_RECOGNITION_FAILED",
                        &format!("unable to OCR scanned PDF page {}", page_index + 1),
                        error,
                        true,
                    )
                })?;
            ensure_not_cancelled(is_cancelled)?;
            let lines = collect_lines(
                &result.Lines().map_err(|error| {
                    windows_error(
                        "OCR_RECOGNITION_FAILED",
                        &format!(
                            "unable to read OCR lines for scanned PDF page {}",
                            page_index + 1
                        ),
                        error,
                        true,
                    )
                })?,
                page_index,
                is_cancelled,
            )?;
            for line in &lines {
                ensure_not_cancelled(is_cancelled)?;
                let normalized = normalize_ocr_line(&line.text);
                if !normalized.is_empty() {
                    recognized_chars = recognized_chars
                        .checked_add(normalized.chars().count())
                        .ok_or_else(|| {
                            HostError::new(
                                "OCR_TEXT_LIMIT_EXCEEDED",
                                "local OCR text size overflowed",
                                false,
                            )
                        })?;
                    recognized_blocks += 1;
                }
            }
            if recognized_chars > MAX_OCR_TEXT_CHARS {
                return Err(HostError::new(
                    "OCR_TEXT_LIMIT_EXCEEDED",
                    format!("local OCR text exceeds the {MAX_OCR_TEXT_CHARS} character limit"),
                    false,
                ));
            }
            if recognized_blocks > MAX_OCR_BLOCKS {
                return Err(HostError::new(
                    "OCR_BLOCK_LIMIT_EXCEEDED",
                    format!("local OCR exceeds the {MAX_OCR_BLOCKS} block limit"),
                    false,
                ));
            }
            pages.push(LocalOcrPage {
                width: bitmap_width,
                height: bitmap_height,
                lines,
            });
        }

        Ok(LocalOcrOutput {
            engine: "Windows.Media.Ocr".to_string(),
            version: "system".to_string(),
            language,
            pages,
        })
    }

    fn initialize_winrt() -> Result<WinRtApartmentGuard, HostError> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(WinRtApartmentGuard { uninitialize: true }),
            Err(error) if error.code() == RPC_E_CHANGED_MODE => Ok(WinRtApartmentGuard {
                uninitialize: false,
            }),
            Err(error) => Err(windows_error(
                "OCR_PLATFORM_INITIALIZATION_FAILED",
                "unable to initialize the Windows Runtime for local OCR",
                error,
                true,
            )),
        }
    }

    fn absolute_source_path(source_path: &Path) -> Result<PathBuf, HostError> {
        if source_path.is_absolute() {
            return Ok(source_path.to_path_buf());
        }
        std::env::current_dir()
            .map(|current| current.join(source_path))
            .map_err(|error| {
                HostError::new(
                    "OCR_PDF_OPEN_FAILED",
                    format!("unable to resolve scanned PDF path: {error}"),
                    true,
                )
            })
    }

    fn create_ocr_engine() -> Result<(OcrEngine, String), HostError> {
        if let Ok(language) = Language::CreateLanguage(&HSTRING::from("zh-Hans")) {
            if OcrEngine::IsLanguageSupported(&language).unwrap_or(false) {
                if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
                    let language_tag =
                        recognizer_language_tag(&engine).unwrap_or_else(|_| "zh-Hans".to_string());
                    return Ok((engine, language_tag));
                }
            }
        }

        let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|error| {
            windows_error(
                "OCR_ENGINE_UNAVAILABLE",
                "Windows OCR has no installed engine for zh-Hans or the current user languages",
                error,
                true,
            )
        })?;
        let language = recognizer_language_tag(&engine).map_err(|error| {
            windows_error(
                "OCR_ENGINE_UNAVAILABLE",
                "unable to determine the Windows OCR recognizer language",
                error,
                true,
            )
        })?;
        Ok((engine, language))
    }

    fn recognizer_language_tag(engine: &OcrEngine) -> windows::core::Result<String> {
        Ok(engine.RecognizerLanguage()?.LanguageTag()?.to_string())
    }

    fn render_dimensions(
        source_width: f32,
        source_height: f32,
        max_dimension: u32,
    ) -> Result<(u32, u32), HostError> {
        let width = f64::from(source_width);
        let height = f64::from(source_height);
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(HostError::new(
                "OCR_PDF_RENDER_FAILED",
                "scanned PDF page reports invalid dimensions",
                true,
            ));
        }
        let max_dimension = f64::from(max_dimension);
        let dimension_scale = (max_dimension / width).min(max_dimension / height);
        let pixel_scale = (MAX_OCR_PIXELS_PER_PAGE as f64 / (width * height)).sqrt();
        let scale = PREFERRED_OCR_RENDER_SCALE
            .min(dimension_scale)
            .min(pixel_scale);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(HostError::new(
                "OCR_PIXEL_LIMIT_EXCEEDED",
                "unable to fit the PDF page inside the local OCR pixel budget",
                false,
            ));
        }
        let rendered_width = (width * scale).floor().max(1.0) as u32;
        let rendered_height = (height * scale).floor().max(1.0) as u32;
        let pixels = u64::from(rendered_width) * u64::from(rendered_height);
        if pixels > MAX_OCR_PIXELS_PER_PAGE
            || rendered_width > max_dimension as u32
            || rendered_height > max_dimension as u32
        {
            return Err(HostError::new(
                "OCR_PIXEL_LIMIT_EXCEEDED",
                "unable to fit the PDF page inside the local OCR render limits",
                false,
            ));
        }
        Ok((rendered_width, rendered_height))
    }

    fn bitmap_dimensions(
        bitmap: &SoftwareBitmap,
        page_index: u32,
    ) -> Result<(u32, u32), HostError> {
        let width = bitmap.PixelWidth().map_err(|error| {
            windows_error(
                "OCR_IMAGE_DECODE_FAILED",
                &format!("unable to read page {} bitmap width", page_index + 1),
                error,
                true,
            )
        })?;
        let height = bitmap.PixelHeight().map_err(|error| {
            windows_error(
                "OCR_IMAGE_DECODE_FAILED",
                &format!("unable to read page {} bitmap height", page_index + 1),
                error,
                true,
            )
        })?;
        if width <= 0 || height <= 0 {
            return Err(HostError::new(
                "OCR_IMAGE_DECODE_FAILED",
                format!("page {} rendered to an empty bitmap", page_index + 1),
                true,
            ));
        }
        Ok((width as u32, height as u32))
    }

    fn collect_lines<F>(
        lines: &windows_collections::IVectorView<OcrLine>,
        page_index: u32,
        is_cancelled: &mut F,
    ) -> Result<Vec<LocalOcrLine>, HostError>
    where
        F: FnMut() -> Result<bool, HostError>,
    {
        let count = lines.Size().map_err(|error| {
            windows_error(
                "OCR_RECOGNITION_FAILED",
                &format!("unable to count OCR lines for page {}", page_index + 1),
                error,
                true,
            )
        })?;
        let mut output = Vec::with_capacity(count as usize);
        for index in 0..count {
            ensure_not_cancelled(is_cancelled)?;
            let line = lines.GetAt(index).map_err(|error| {
                windows_error(
                    "OCR_RECOGNITION_FAILED",
                    &format!(
                        "unable to access OCR line {} on page {}",
                        index + 1,
                        page_index + 1
                    ),
                    error,
                    true,
                )
            })?;
            let text = line.Text().map_err(|error| {
                windows_error(
                    "OCR_RECOGNITION_FAILED",
                    &format!(
                        "unable to read OCR line {} on page {}",
                        index + 1,
                        page_index + 1
                    ),
                    error,
                    true,
                )
            })?;
            output.push(LocalOcrLine {
                text: text.to_string(),
                bbox: line_bounding_box(&line, page_index, index, is_cancelled)?,
            });
        }
        Ok(output)
    }

    fn line_bounding_box<F>(
        line: &OcrLine,
        page_index: u32,
        line_index: u32,
        is_cancelled: &mut F,
    ) -> Result<Option<EvidenceBoundingBox>, HostError>
    where
        F: FnMut() -> Result<bool, HostError>,
    {
        let words = line.Words().map_err(|error| {
            windows_error(
                "OCR_RECOGNITION_FAILED",
                &format!(
                    "unable to read OCR words for line {} on page {}",
                    line_index + 1,
                    page_index + 1
                ),
                error,
                true,
            )
        })?;
        let count = words.Size().map_err(|error| {
            windows_error(
                "OCR_RECOGNITION_FAILED",
                &format!(
                    "unable to count OCR words for line {} on page {}",
                    line_index + 1,
                    page_index + 1
                ),
                error,
                true,
            )
        })?;
        let mut left = f64::INFINITY;
        let mut top = f64::INFINITY;
        let mut right = f64::NEG_INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for word_index in 0..count {
            ensure_not_cancelled(is_cancelled)?;
            let word = words.GetAt(word_index).map_err(|error| {
                windows_error(
                    "OCR_RECOGNITION_FAILED",
                    &format!(
                        "unable to access OCR word {} on page {}",
                        word_index + 1,
                        page_index + 1
                    ),
                    error,
                    true,
                )
            })?;
            let rect = word.BoundingRect().map_err(|error| {
                windows_error(
                    "OCR_RECOGNITION_FAILED",
                    &format!("unable to read OCR word bounds on page {}", page_index + 1),
                    error,
                    true,
                )
            })?;
            let x = f64::from(rect.X);
            let y = f64::from(rect.Y);
            let width = f64::from(rect.Width);
            let height = f64::from(rect.Height);
            if x.is_finite()
                && y.is_finite()
                && width.is_finite()
                && height.is_finite()
                && x >= 0.0
                && y >= 0.0
                && width > 0.0
                && height > 0.0
            {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + width);
                bottom = bottom.max(y + height);
            }
        }
        if !left.is_finite() || !top.is_finite() || right <= left || bottom <= top {
            return Ok(None);
        }
        Ok(Some(EvidenceBoundingBox {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }))
    }

    fn ensure_not_cancelled<F>(is_cancelled: &mut F) -> Result<(), HostError>
    where
        F: FnMut() -> Result<bool, HostError>,
    {
        if check_cancelled(is_cancelled)? {
            return Err(HostError::new(
                "DOCUMENT_EXTRACTION_CANCELLED",
                "document extraction was cancelled during local OCR",
                false,
            ));
        }
        Ok(())
    }

    fn windows_error(
        code: &'static str,
        context: &str,
        error: WindowsError,
        retryable: bool,
    ) -> HostError {
        HostError::new(
            code,
            format!(
                "{context}: {} (HRESULT 0x{:08X})",
                error.message(),
                error.code().0 as u32
            ),
            retryable,
        )
    }
}

fn deterministic_uuid(kind: &str, components: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hash_uuid_component(&mut hasher, DETERMINISTIC_UUID_DOMAIN);
    hash_uuid_component(&mut hasher, kind);
    for component in components {
        hash_uuid_component(&mut hasher, component);
    }

    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn hash_uuid_component(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

pub fn build_extraction_from_text(
    review_id: &str,
    source_asset_id: &str,
    source_asset_sha256: &str,
    parser_name: &str,
    parser_version: &str,
    text: &str,
    now: i64,
) -> DocumentExtractionRecord {
    let extraction_id = deterministic_uuid(
        "extraction",
        &[
            review_id,
            source_asset_id,
            source_asset_sha256,
            parser_name,
            parser_version,
        ],
    );
    let normalized = normalize_text(text);
    let needs_ocr = normalized
        .chars()
        .filter(|value| !value.is_whitespace())
        .count()
        < MIN_SEARCHABLE_TEXT_CHARS;
    let raw_pages = split_pages(&normalized);
    let mut pages = Vec::with_capacity(raw_pages.len());
    let mut blocks = Vec::new();

    for (page_index, page_text) in raw_pages.iter().enumerate() {
        let page_index = page_index as i64;
        let page_index_key = page_index.to_string();
        let page_id =
            deterministic_uuid("page", &[extraction_id.as_str(), page_index_key.as_str()]);
        pages.push(DocumentPageRecord {
            id: page_id.clone(),
            extraction_id: extraction_id.clone(),
            page_index,
            text: page_text.clone(),
            text_sha256: sha256_text(page_text),
            width: None,
            height: None,
            preview_asset_id: None,
        });
        blocks.extend(build_blocks(
            &extraction_id,
            &page_id,
            page_index,
            page_text,
        ));
    }

    DocumentExtractionRecord {
        id: extraction_id,
        review_id: review_id.to_string(),
        source_asset_id: source_asset_id.to_string(),
        source_asset_sha256: source_asset_sha256.to_string(),
        parser: ParserProvenance {
            name: parser_name.to_string(),
            version: parser_version.to_string(),
            mode: "searchableText".to_string(),
        },
        ocr: None::<OcrProvenance>,
        status: if needs_ocr {
            DocumentExtractionStatus::AwaitingOcr
        } else {
            DocumentExtractionStatus::Completed
        },
        page_count: pages.len() as i64,
        content_sha256: (!normalized.is_empty()).then(|| sha256_text(&normalized)),
        snapshot_asset_id: None,
        pages,
        blocks,
        tables: Vec::<DocumentTableRecord>::new(),
        created_at: now,
        completed_at: (!needs_ocr).then_some(now),
        failure: needs_ocr.then(|| ContractReviewFailure {
            code: "OCR_REQUIRED".to_string(),
            message: "PDF does not contain enough searchable text; OCR is required".to_string(),
            retryable: true,
            stage: ContractReviewStage::AwaitingOcr,
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocxFlowItem {
    Block {
        kind: DocumentBlockKind,
        text: String,
    },
    Table {
        rows: Vec<Vec<String>>,
    },
}

#[derive(Default)]
struct DocxParagraph {
    text: String,
    style: Option<String>,
    numbered: bool,
}

#[derive(Default)]
struct DocxTable {
    rows: Vec<Vec<String>>,
    row: Option<Vec<String>>,
    cell: Option<String>,
}

#[derive(Default)]
struct DocxState {
    flow: Vec<DocxFlowItem>,
    paragraph: Option<DocxParagraph>,
    table: Option<DocxTable>,
    table_depth: usize,
    text_depth: usize,
    saw_document: bool,
    saw_body: bool,
}

impl DocxState {
    fn start(&mut self, name: &str, attributes: &[(String, String)]) {
        match local_name(name) {
            "document" => self.saw_document = true,
            "body" => self.saw_body = true,
            "tbl" => {
                self.table_depth += 1;
                if self.table_depth == 1 {
                    self.table = Some(DocxTable::default());
                }
            }
            "tr" if self.table_depth == 1 => {
                if let Some(table) = self.table.as_mut() {
                    finish_cell(table);
                    finish_row(table);
                    table.row = Some(Vec::new());
                }
            }
            "tc" if self.table_depth == 1 => {
                if let Some(table) = self.table.as_mut() {
                    finish_cell(table);
                    table.row.get_or_insert_with(Vec::new);
                    table.cell = Some(String::new());
                }
            }
            "p" => self.paragraph = Some(DocxParagraph::default()),
            "pStyle" => {
                if let Some(paragraph) = self.paragraph.as_mut() {
                    paragraph.style = attribute_value(attributes, "val").map(str::to_string);
                }
            }
            "numPr" => {
                if let Some(paragraph) = self.paragraph.as_mut() {
                    paragraph.numbered = true;
                }
            }
            "t" => self.text_depth += 1,
            "tab" => self.inline("\t"),
            "br" | "cr" => self.inline("\n"),
            "noBreakHyphen" => self.inline("-"),
            "softHyphen" => self.inline("\u{00ad}"),
            _ => {}
        }
    }

    fn end(&mut self, name: &str) {
        match local_name(name) {
            "t" => self.text_depth = self.text_depth.saturating_sub(1),
            "p" => self.finish_paragraph(),
            "tc" if self.table_depth == 1 => {
                if let Some(table) = self.table.as_mut() {
                    finish_cell(table);
                }
            }
            "tr" if self.table_depth == 1 => {
                if let Some(table) = self.table.as_mut() {
                    finish_cell(table);
                    finish_row(table);
                }
            }
            "tbl" => {
                if self.table_depth == 1 {
                    if let Some(mut table) = self.table.take() {
                        finish_cell(&mut table);
                        finish_row(&mut table);
                        self.flow.push(DocxFlowItem::Table { rows: table.rows });
                    }
                }
                self.table_depth = self.table_depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn text(&mut self, value: &str) {
        if self.text_depth > 0 {
            self.inline(value);
        }
    }

    fn inline(&mut self, value: &str) {
        if let Some(paragraph) = self.paragraph.as_mut() {
            paragraph.text.push_str(value);
        }
    }

    fn finish_paragraph(&mut self) {
        let Some(paragraph) = self.paragraph.take() else {
            return;
        };
        let text = normalize_docx_text(&paragraph.text);
        if self.table_depth > 0 {
            if let Some(cell) = self.table.as_mut().and_then(|table| table.cell.as_mut()) {
                if !text.is_empty() {
                    if !cell.is_empty() {
                        cell.push('\n');
                    }
                    cell.push_str(&text);
                }
            }
        } else if !text.is_empty() {
            self.flow.push(DocxFlowItem::Block {
                kind: docx_kind(paragraph.style.as_deref(), paragraph.numbered),
                text,
            });
        }
    }
}

fn parse_docx_document_xml(xml: &str) -> Result<Vec<DocxFlowItem>, HostError> {
    let mut state = DocxState::default();
    let mut stack = Vec::<String>::new();
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let Some(relative_open) = xml[cursor..].find('<') else {
            state.text(&decode_xml_entities(&xml[cursor..])?);
            break;
        };
        let open = cursor + relative_open;
        if open > cursor {
            state.text(&decode_xml_entities(&xml[cursor..open])?);
        }
        if xml[open..].starts_with("<!--") {
            let end = xml[open + 4..]
                .find("-->")
                .ok_or_else(|| docx_xml_error("unterminated XML comment"))?;
            cursor = open + 4 + end + 3;
            continue;
        }
        if xml[open..].starts_with("<![CDATA[") {
            let start = open + 9;
            let end = xml[start..]
                .find("]]>")
                .ok_or_else(|| docx_xml_error("unterminated CDATA section"))?;
            state.text(&xml[start..start + end]);
            cursor = start + end + 3;
            continue;
        }
        if xml[open..].starts_with("<?") {
            let end = xml[open + 2..]
                .find("?>")
                .ok_or_else(|| docx_xml_error("unterminated processing instruction"))?;
            cursor = open + 2 + end + 2;
            continue;
        }
        if xml[open..].starts_with("<!") {
            return Err(docx_xml_error("unsupported XML declaration"));
        }
        let close = find_tag_end(xml, open + 1)?;
        let tag = parse_tag(&xml[open + 1..close])?;
        if tag.is_end {
            let opened = stack
                .pop()
                .ok_or_else(|| docx_xml_error("unexpected closing XML element"))?;
            if opened != tag.name {
                return Err(docx_xml_error(format!(
                    "mismatched XML element: expected </{opened}> but found </{}>",
                    tag.name
                )));
            }
            state.end(&tag.name);
        } else {
            state.start(&tag.name, &tag.attributes);
            if tag.self_closing {
                state.end(&tag.name);
            } else {
                stack.push(tag.name);
            }
        }
        cursor = close + 1;
    }
    if !stack.is_empty() {
        return Err(docx_xml_error("unterminated XML element"));
    }
    if !state.saw_document || !state.saw_body {
        return Err(docx_xml_error(
            "word/document.xml is missing the document body",
        ));
    }
    Ok(state.flow)
}

struct XmlTag {
    name: String,
    attributes: Vec<(String, String)>,
    is_end: bool,
    self_closing: bool,
}

fn parse_tag(raw: &str) -> Result<XmlTag, HostError> {
    let mut value = raw.trim();
    if value.is_empty() {
        return Err(docx_xml_error("empty XML tag"));
    }
    let is_end = value.starts_with('/');
    if is_end {
        value = value[1..].trim_start();
    }
    let self_closing = !is_end && value.ends_with('/');
    if self_closing {
        value = value[..value.len() - 1].trim_end();
    }
    let name_end = value.find(char::is_whitespace).unwrap_or(value.len());
    let name = value[..name_end].trim();
    if name.is_empty() {
        return Err(docx_xml_error("XML tag has no name"));
    }
    if is_end && !value[name_end..].trim().is_empty() {
        return Err(docx_xml_error("closing XML tag contains attributes"));
    }
    Ok(XmlTag {
        name: name.to_string(),
        attributes: if is_end {
            Vec::new()
        } else {
            parse_attributes(&value[name_end..])?
        },
        is_end,
        self_closing,
    })
}

fn parse_attributes(raw: &str) -> Result<Vec<(String, String)>, HostError> {
    let bytes = raw.as_bytes();
    let mut result = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        let start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'='
        {
            cursor += 1;
        }
        let name = raw[start..cursor].trim();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if name.is_empty() || cursor >= bytes.len() || bytes[cursor] != b'=' {
            return Err(docx_xml_error("malformed XML attribute"));
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || !matches!(bytes[cursor], b'\'' | b'"') {
            return Err(docx_xml_error("XML attribute value must be quoted"));
        }
        let quote = bytes[cursor];
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != quote {
            cursor += 1;
        }
        if cursor == bytes.len() {
            return Err(docx_xml_error("unterminated XML attribute"));
        }
        result.push((name.to_string(), decode_xml_entities(&raw[start..cursor])?));
        cursor += 1;
    }
    Ok(result)
}

fn find_tag_end(xml: &str, start: usize) -> Result<usize, HostError> {
    let bytes = xml.as_bytes();
    let mut quote = None;
    for (cursor, value) in bytes.iter().copied().enumerate().skip(start) {
        match (value, quote) {
            (b'\'' | b'"', None) => quote = Some(value),
            (value, Some(active)) if value == active => quote = None,
            (b'>', None) => return Ok(cursor),
            _ => {}
        }
    }
    Err(docx_xml_error("unterminated XML tag"))
}

fn decode_xml_entities(value: &str) -> Result<String, HostError> {
    let mut decoded = String::with_capacity(value.len());
    let mut cursor = 0usize;
    while let Some(relative) = value[cursor..].find('&') {
        let start = cursor + relative;
        decoded.push_str(&value[cursor..start]);
        let end = value[start + 1..]
            .find(';')
            .map(|offset| start + 1 + offset)
            .ok_or_else(|| docx_xml_error("unterminated XML entity"))?;
        let entity = &value[start + 1..end];
        match entity {
            "amp" => decoded.push('&'),
            "lt" => decoded.push('<'),
            "gt" => decoded.push('>'),
            "quot" => decoded.push('"'),
            "apos" => decoded.push('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                let code = u32::from_str_radix(&entity[2..], 16)
                    .map_err(|_| docx_xml_error("invalid hexadecimal XML entity"))?;
                decoded.push(
                    char::from_u32(code)
                        .ok_or_else(|| docx_xml_error("invalid XML character entity"))?,
                );
            }
            _ if entity.starts_with('#') => {
                let code = entity[1..]
                    .parse::<u32>()
                    .map_err(|_| docx_xml_error("invalid decimal XML entity"))?;
                decoded.push(
                    char::from_u32(code)
                        .ok_or_else(|| docx_xml_error("invalid XML character entity"))?,
                );
            }
            _ => return Err(docx_xml_error(format!("unsupported XML entity &{entity};"))),
        }
        cursor = end + 1;
    }
    decoded.push_str(&value[cursor..]);
    Ok(decoded)
}

fn build_docx_extraction(
    review_id: &str,
    source_asset_id: &str,
    source_asset_sha256: &str,
    parser_name: &str,
    parser_version: &str,
    flow: Vec<DocxFlowItem>,
    now: i64,
) -> DocumentExtractionRecord {
    let extraction_id = deterministic_uuid(
        "extraction",
        &[
            review_id,
            source_asset_id,
            source_asset_sha256,
            parser_name,
            parser_version,
        ],
    );
    let page_id = deterministic_uuid("page", &[extraction_id.as_str(), "0"]);
    let mut page_text = String::new();
    let mut blocks = Vec::new();
    let mut tables = Vec::new();
    for (order_index, item) in flow.into_iter().enumerate() {
        let order_index = order_index as i64;
        let order_index_key = order_index.to_string();
        let (kind, text, rows) = match item {
            DocxFlowItem::Block { kind, text } => (kind, text, None),
            DocxFlowItem::Table { rows } => {
                (DocumentBlockKind::Table, table_markdown(&rows), Some(rows))
            }
        };
        if !page_text.is_empty() {
            page_text.push_str("\n\n");
        }
        let char_start = page_text.len() as i64;
        page_text.push_str(&text);
        let char_end = page_text.len() as i64;
        blocks.push(DocumentBlockRecord {
            id: deterministic_uuid(
                "block",
                &[
                    extraction_id.as_str(),
                    page_id.as_str(),
                    order_index_key.as_str(),
                ],
            ),
            extraction_id: extraction_id.clone(),
            page_id: page_id.clone(),
            page_index: 0,
            order_index,
            kind,
            text: text.clone(),
            char_start,
            char_end,
            bbox: None,
        });
        if let Some(rows) = rows {
            let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
            tables.push(DocumentTableRecord {
                id: deterministic_uuid(
                    "table",
                    &[
                        extraction_id.as_str(),
                        page_id.as_str(),
                        order_index_key.as_str(),
                    ],
                ),
                extraction_id: extraction_id.clone(),
                page_id: page_id.clone(),
                page_index: 0,
                order_index,
                markdown: text,
                data: serde_json::json!({ "columnCount": column_count, "rows": rows }),
                bbox: None,
            });
        }
    }
    let content_sha256 = sha256_text(&page_text);
    DocumentExtractionRecord {
        id: extraction_id.clone(),
        review_id: review_id.to_string(),
        source_asset_id: source_asset_id.to_string(),
        source_asset_sha256: source_asset_sha256.to_string(),
        parser: ParserProvenance {
            name: parser_name.to_string(),
            version: parser_version.to_string(),
            mode: "ooxmlLogicalFlow".to_string(),
        },
        ocr: None,
        status: DocumentExtractionStatus::Completed,
        page_count: 1,
        content_sha256: Some(content_sha256.clone()),
        snapshot_asset_id: None,
        pages: vec![DocumentPageRecord {
            id: page_id,
            extraction_id,
            page_index: 0,
            text: page_text,
            text_sha256: content_sha256,
            width: None,
            height: None,
            preview_asset_id: None,
        }],
        blocks,
        tables,
        created_at: now,
        completed_at: Some(now),
        failure: None,
    }
}

fn finish_cell(table: &mut DocxTable) {
    if let Some(cell) = table.cell.take() {
        table
            .row
            .get_or_insert_with(Vec::new)
            .push(normalize_docx_text(&cell));
    }
}

fn finish_row(table: &mut DocxTable) {
    if let Some(row) = table.row.take() {
        table.rows.push(row);
    }
}

fn table_markdown(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return String::new();
    }
    let rows = rows
        .iter()
        .map(|row| {
            (0..columns)
                .map(|index| markdown_cell(row.get(index).map(String::as_str).unwrap_or("")))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut lines = vec![format!("| {} |", rows[0].join(" | "))];
    lines.push(format!(
        "| {} |",
        std::iter::repeat_n("---", columns)
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    lines.extend(
        rows.iter()
            .skip(1)
            .map(|row| format!("| {} |", row.join(" | "))),
    );
    lines.join("\n")
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
}

fn docx_kind(style: Option<&str>, numbered: bool) -> DocumentBlockKind {
    let style = style.unwrap_or_default();
    let lower = style.to_lowercase();
    if numbered
        || lower.contains("list")
        || lower.contains("bullet")
        || lower.contains("number")
        || style.contains("列表")
    {
        DocumentBlockKind::ListItem
    } else if lower.contains("heading")
        || lower == "title"
        || lower == "subtitle"
        || style.starts_with("标题")
    {
        DocumentBlockKind::Heading
    } else {
        DocumentBlockKind::Paragraph
    }
}

fn normalize_docx_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{0000}', "")
        .trim()
        .to_string()
}

fn attribute_value<'a>(attributes: &'a [(String, String)], wanted: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(name, _)| local_name(name) == wanted)
        .map(|(_, value)| value.as_str())
}

fn local_name(name: &str) -> &str {
    name.rsplit_once(':')
        .map(|(_, local)| local)
        .unwrap_or(name)
}

fn docx_xml_error(message: impl Into<String>) -> HostError {
    HostError::new("DOCX_DOCUMENT_XML_INVALID", message, false)
}

fn supports_extension_or_mime(
    source_path: &Path,
    expected_extension: &str,
    mime_type: &str,
    supported_mime_types: &[&str],
) -> bool {
    match source_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => {
            extension.eq_ignore_ascii_case(expected_extension)
        }
        _ => supported_mime_types
            .iter()
            .any(|supported| mime_type.eq_ignore_ascii_case(supported)),
    }
}

fn validate_source(
    review_id: &str,
    source_asset_id: &str,
    source_asset_sha256: &str,
    source_path: &Path,
) -> Result<(), HostError> {
    if review_id.trim().is_empty() || source_asset_id.trim().is_empty() {
        return Err(HostError::validation(
            "reviewId and sourceAssetId are required for extraction",
        ));
    }
    if source_asset_sha256.len() != 64
        || !source_asset_sha256
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err(HostError::validation(
            "sourceAssetSha256 must be a 64-character hex digest",
        ));
    }
    let metadata = fs::metadata(source_path).map_err(|error| {
        HostError::new(
            "DOCUMENT_SOURCE_UNAVAILABLE",
            format!("contract source is unavailable: {error}"),
            true,
        )
    })?;
    if !metadata.is_file() {
        return Err(HostError::validation("contract source must be a file"));
    }
    Ok(())
}

fn normalize_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{0000}', "")
        .trim()
        .to_string()
}

fn split_pages(text: &str) -> Vec<String> {
    let pages = text
        .split('\u{000c}')
        .map(str::trim)
        .filter(|page| !page.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if pages.is_empty() {
        vec![String::new()]
    } else {
        pages
    }
}

fn build_blocks(
    extraction_id: &str,
    page_id: &str,
    page_index: i64,
    page_text: &str,
) -> Vec<DocumentBlockRecord> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    for (order_index, paragraph) in page_text
        .split("\n\n")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .enumerate()
    {
        let relative = page_text[cursor..].find(paragraph).unwrap_or(0);
        let start = cursor + relative;
        let end = start + paragraph.len();
        let kind = infer_block_kind(paragraph);
        let order_index = order_index as i64;
        let order_index_key = order_index.to_string();
        blocks.push(DocumentBlockRecord {
            id: deterministic_uuid("block", &[extraction_id, page_id, &order_index_key]),
            extraction_id: extraction_id.to_string(),
            page_id: page_id.to_string(),
            page_index,
            order_index,
            kind,
            text: paragraph.to_string(),
            char_start: start as i64,
            char_end: end as i64,
            bbox: None,
        });
        cursor = end.min(page_text.len());
    }
    if blocks.is_empty() && !page_text.is_empty() {
        blocks.push(DocumentBlockRecord {
            id: deterministic_uuid("block", &[extraction_id, page_id, "0"]),
            extraction_id: extraction_id.to_string(),
            page_id: page_id.to_string(),
            page_index,
            order_index: 0,
            kind: DocumentBlockKind::Paragraph,
            text: page_text.to_string(),
            char_start: 0,
            char_end: page_text.len() as i64,
            bbox: None,
        });
    }
    blocks
}

fn infer_block_kind(value: &str) -> DocumentBlockKind {
    let trimmed = value.trim();
    if trimmed.len() <= 40
        && (trimmed.ends_with(':')
            || trimmed.ends_with('：')
            || trimmed.contains("合同")
            || trimmed.starts_with('第'))
    {
        DocumentBlockKind::Heading
    } else if trimmed.starts_with(['-', '•', '·', '(']) {
        DocumentBlockKind::ListItem
    } else {
        DocumentBlockKind::Paragraph
    }
}

pub fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn hash() -> String {
        "a".repeat(64)
    }

    fn extraction_ids(extraction: &DocumentExtractionRecord) -> Vec<String> {
        let mut ids = vec![extraction.id.clone()];
        ids.extend(extraction.pages.iter().map(|page| page.id.clone()));
        ids.extend(extraction.blocks.iter().map(|block| block.id.clone()));
        ids.extend(extraction.tables.iter().map(|table| table.id.clone()));
        ids
    }

    fn assert_rfc4122_version_5(ids: &[String]) {
        for id in ids {
            let uuid = Uuid::parse_str(id).unwrap();
            assert_eq!(uuid.as_bytes()[6] >> 4, 5);
            assert_eq!(uuid.as_bytes()[8] & 0xc0, 0x80);
        }
    }

    fn write_test_pdf(path: &Path, text: Option<&str>) {
        let content = text.map_or_else(
            || "q Q".to_string(),
            |value| {
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('(', "\\(")
                    .replace(')', "\\)");
                format!("BT /F1 12 Tf 72 720 Td ({escaped}) Tj ET")
            },
        );
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                objects.len() + 1,
                xref
            )
            .as_bytes(),
        );
        fs::write(path, pdf).unwrap();
    }

    fn write_test_docx(path: &Path, document_xml: &str) {
        let file = fs::File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            // Real Microsoft Word DOCX packages normally use Deflate. Keeping the
            // fixture compressed prevents a Stored-only build from passing tests
            // while rejecting ordinary customer documents at runtime.
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        archive.start_file("word/document.xml", options).unwrap();
        archive.write_all(document_xml.as_bytes()).unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn searchable_pdf_still_extracts_text() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("contract.pdf");
        write_test_pdf(
            &path,
            Some("Contract payment acceptance delivery and liability terms are searchable."),
        );
        let extraction = DocumentIntelligence::with_defaults()
            .extract(
                "review-1",
                "asset-1",
                &hash(),
                "application/octet-stream",
                &path,
                10,
            )
            .unwrap();
        assert_eq!(extraction.parser.name, TEXT_PDF_PARSER_NAME);
        assert_eq!(extraction.status, DocumentExtractionStatus::Completed);
        assert_eq!(extraction.page_count, 1);
        assert!(!extraction.blocks.is_empty());
        assert!(extraction.content_sha256.is_some());
    }

    #[test]
    fn scanned_pdf_requests_ocr_without_faking_ocr_output() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("scan.pdf");
        write_test_pdf(&path, None);
        let extraction = TextPdfParser
            .extract("review-1", "asset-1", &hash(), &path, 10)
            .unwrap();
        assert_eq!(extraction.status, DocumentExtractionStatus::AwaitingOcr);
        assert_eq!(extraction.failure.as_ref().unwrap().code, "OCR_REQUIRED");
        assert!(extraction.ocr.is_none());
        assert!(extraction.completed_at.is_none());
    }

    #[test]
    fn local_ocr_preserves_chinese_character_offsets_zero_based_pages_and_clamped_bboxes() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("scan.pdf");
        write_test_pdf(&path, None);
        let mut extraction = TextPdfParser
            .extract("review-ocr", "asset-ocr", &hash(), &path, 10)
            .unwrap();
        let output = LocalOcrOutput {
            engine: "Windows.Media.Ocr".to_string(),
            version: "1".to_string(),
            language: "zh-Hans".to_string(),
            pages: vec![
                LocalOcrPage {
                    width: 100,
                    height: 80,
                    lines: vec![
                        LocalOcrLine {
                            text: "合同审查".to_string(),
                            bbox: Some(EvidenceBoundingBox {
                                x: -10.0,
                                y: 5.0,
                                width: 50.0,
                                height: 100.0,
                            }),
                        },
                        LocalOcrLine {
                            text: "金额：86000元".to_string(),
                            bbox: Some(EvidenceBoundingBox {
                                x: f64::NAN,
                                y: 0.0,
                                width: 20.0,
                                height: 10.0,
                            }),
                        },
                    ],
                },
                LocalOcrPage {
                    width: 120,
                    height: 90,
                    lines: vec![LocalOcrLine {
                        text: "验收标准".to_string(),
                        bbox: Some(EvidenceBoundingBox {
                            x: 10.0,
                            y: 10.0,
                            width: 0.0,
                            height: 20.0,
                        }),
                    }],
                },
            ],
        };

        apply_local_ocr_output(&mut extraction, output, 20, &mut || Ok(false)).unwrap();

        assert_eq!(extraction.status, DocumentExtractionStatus::Completed);
        assert_eq!(extraction.page_count, 2);
        assert_eq!(
            extraction
                .pages
                .iter()
                .map(|page| page.page_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            extraction
                .blocks
                .iter()
                .map(|block| block.page_index)
                .collect::<Vec<_>>(),
            vec![0, 0, 1]
        );
        assert_eq!(extraction.blocks[0].char_start, 0);
        assert_eq!(extraction.blocks[0].char_end, 4);
        assert_eq!(extraction.blocks[1].char_start, 5);
        assert_eq!(extraction.blocks[1].char_end, 14);
        for block in &extraction.blocks {
            let page = &extraction.pages[block.page_index as usize];
            let recovered = page
                .text
                .chars()
                .skip(block.char_start as usize)
                .take((block.char_end - block.char_start) as usize)
                .collect::<String>();
            assert_eq!(recovered, block.text);
        }
        assert_eq!(
            extraction.blocks[0].bbox,
            Some(EvidenceBoundingBox {
                x: 0.0,
                y: 5.0,
                width: 40.0,
                height: 75.0,
            })
        );
        assert!(extraction.blocks[1].bbox.is_none());
        assert!(extraction.blocks[2].bbox.is_none());
        assert_eq!(extraction.parser.mode, "windowsPdfRenderOcr");
        let ocr = extraction.ocr.as_ref().unwrap();
        assert_eq!(ocr.engine, "Windows.Media.Ocr");
        assert_eq!(ocr.version, "1");
        assert_eq!(ocr.language, "zh-Hans");
        assert_eq!(
            extraction.content_sha256,
            Some(sha256_text("合同审查\n金额：86000元\u{000c}验收标准"))
        );
        assert_eq!(extraction.completed_at, Some(20));
        assert!(extraction.failure.is_none());
    }

    #[test]
    fn local_ocr_cancellation_does_not_commit_partial_output() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("scan.pdf");
        write_test_pdf(&path, None);
        let mut extraction = TextPdfParser
            .extract("review-cancel", "asset-cancel", &hash(), &path, 10)
            .unwrap();
        let before = extraction.clone();
        let output = LocalOcrOutput {
            engine: "Windows.Media.Ocr".to_string(),
            version: "1".to_string(),
            language: "zh-Hans".to_string(),
            pages: vec![LocalOcrPage {
                width: 100,
                height: 80,
                lines: vec![LocalOcrLine {
                    text: "合同审查".to_string(),
                    bbox: None,
                }],
            }],
        };
        let mut checks = 0;

        let error = apply_local_ocr_output(&mut extraction, output, 20, &mut || {
            checks += 1;
            Ok(checks >= 3)
        })
        .unwrap_err();

        assert_eq!(checks, 3);
        assert_eq!(error.code, "DOCUMENT_EXTRACTION_CANCELLED");
        assert!(!error.retryable);
        assert_eq!(extraction, before);
    }

    #[test]
    fn scanned_pdf_can_cancel_after_parsing_before_windows_ocr_starts() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("scan.pdf");
        write_test_pdf(&path, None);
        let mut checks = 0;

        let extraction = DocumentIntelligence::with_defaults()
            .extract_with_cancel(
                "review-cancel-before-ocr",
                "asset-cancel-before-ocr",
                &hash(),
                "application/pdf",
                &path,
                20,
                || {
                    checks += 1;
                    Ok(checks >= 2)
                },
            )
            .unwrap();

        assert_eq!(checks, 2);
        assert_eq!(extraction.status, DocumentExtractionStatus::Cancelled);
        assert_eq!(
            extraction.failure.as_ref().unwrap().code,
            "DOCUMENT_EXTRACTION_CANCELLED"
        );
        assert!(extraction.ocr.is_none());
        assert_eq!(extraction.completed_at, Some(20));
    }

    #[test]
    fn docx_extracts_headings_lists_paragraphs_and_tables() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("contract.DOCX");
        write_test_docx(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>服务合同</w:t></w:r></w:p>
    <w:p><w:r><w:t>甲方应于验收完成后十个工作日内支付合同价款。</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>乙方提供成片</w:t></w:r></w:p>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>节点</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>比例</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>验收</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>50%</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
  </w:body>
</w:document>"#,
        );
        let extraction = DocumentIntelligence::with_defaults()
            .extract("review-2", "asset-2", &hash(), "application/pdf", &path, 20)
            .unwrap();
        assert_eq!(extraction.parser.name, DOCX_PARSER_NAME);
        assert_eq!(extraction.parser.mode, "ooxmlLogicalFlow");
        assert_eq!(extraction.status, DocumentExtractionStatus::Completed);
        assert_eq!(extraction.page_count, 1);
        assert_eq!(extraction.blocks.len(), 4);
        assert_eq!(extraction.blocks[0].kind, DocumentBlockKind::Heading);
        assert_eq!(extraction.blocks[1].kind, DocumentBlockKind::Paragraph);
        assert_eq!(extraction.blocks[2].kind, DocumentBlockKind::ListItem);
        assert_eq!(extraction.blocks[3].kind, DocumentBlockKind::Table);
        assert_eq!(extraction.tables.len(), 1);
        assert!(extraction.tables[0].markdown.contains("| 节点 | 比例 |"));
        assert_eq!(extraction.tables[0].data["columnCount"], 2);
        assert_eq!(extraction.tables[0].data["rows"][1][0], "验收");
        assert_eq!(
            extraction.content_sha256,
            Some(sha256_text(&extraction.pages[0].text))
        );
        for (index, block) in extraction.blocks.iter().enumerate() {
            assert_eq!(block.order_index, index as i64);
            assert_eq!(
                &extraction.pages[0].text[block.char_start as usize..block.char_end as usize],
                block.text
            );
        }
    }

    #[test]
    fn repeated_text_extraction_has_stable_ids_and_chinese_content() {
        let text = "合同标题\n\n甲方应当在验收完成后十个工作日内支付全部合同价款。\n\n- 逾期付款应承担违约责任。";
        let source_sha256 = sha256_text(text);
        let first = build_extraction_from_text(
            "review-stable",
            "asset-stable",
            &source_sha256,
            TEXT_PDF_PARSER_NAME,
            TEXT_PDF_PARSER_VERSION,
            text,
            100,
        );
        let retry = build_extraction_from_text(
            "review-stable",
            "asset-stable",
            &source_sha256,
            TEXT_PDF_PARSER_NAME,
            TEXT_PDF_PARSER_VERSION,
            text,
            200,
        );

        assert_eq!(first.id, retry.id);
        assert_eq!(first.pages, retry.pages);
        assert_eq!(first.blocks, retry.blocks);
        assert_eq!(first.tables, retry.tables);
        assert_eq!(first.content_sha256, retry.content_sha256);
        assert_eq!(first.pages[0].text, normalize_text(text));
        assert!(first.pages[0].text.contains("甲方"));
        assert_ne!(first.created_at, retry.created_at);
        assert_rfc4122_version_5(&extraction_ids(&first));

        let other_review = build_extraction_from_text(
            "review-other",
            "asset-stable",
            &source_sha256,
            TEXT_PDF_PARSER_NAME,
            TEXT_PDF_PARSER_VERSION,
            text,
            300,
        );
        let changed_text = "合同标题\n\n甲方应当在验收完成后十五个工作日内支付全部合同价款。\n\n- 逾期付款应承担违约责任。";
        let other_content = build_extraction_from_text(
            "review-stable",
            "asset-stable",
            &sha256_text(changed_text),
            TEXT_PDF_PARSER_NAME,
            TEXT_PDF_PARSER_VERSION,
            changed_text,
            400,
        );
        let stable_ids = extraction_ids(&first);
        let other_review_ids = extraction_ids(&other_review);
        let other_content_ids = extraction_ids(&other_content);
        assert_eq!(stable_ids.len(), other_review_ids.len());
        assert_eq!(stable_ids.len(), other_content_ids.len());
        assert!(stable_ids
            .iter()
            .zip(&other_review_ids)
            .all(|(stable, changed)| stable != changed));
        assert!(stable_ids
            .iter()
            .zip(&other_content_ids)
            .all(|(stable, changed)| stable != changed));
    }

    #[test]
    fn repeated_docx_extraction_has_stable_table_ids_and_content() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("stable.docx");
        write_test_docx(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>付款安排</w:t></w:r></w:p>
    <w:p><w:r><w:t>双方确认以下付款节点和比例。</w:t></w:r></w:p>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>节点</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>比例</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>验收</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>50%</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
  </w:body>
</w:document>"#,
        );
        let source_sha256 = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        let intelligence = DocumentIntelligence::with_defaults();
        let first = intelligence
            .extract(
                "review-docx",
                "asset-docx",
                &source_sha256,
                DOCX_MIME_TYPE,
                &path,
                500,
            )
            .unwrap();
        let retry = intelligence
            .extract(
                "review-docx",
                "asset-docx",
                &source_sha256,
                DOCX_MIME_TYPE,
                &path,
                600,
            )
            .unwrap();

        assert_eq!(first.id, retry.id);
        assert_eq!(first.pages, retry.pages);
        assert_eq!(first.blocks, retry.blocks);
        assert_eq!(first.tables, retry.tables);
        assert_eq!(first.content_sha256, retry.content_sha256);
        assert_eq!(first.tables.len(), 1);
        assert!(first.tables[0].markdown.contains("验收"));
        assert_rfc4122_version_5(&extraction_ids(&first));
    }

    #[test]
    fn damaged_docx_returns_structured_error() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("damaged.docx");
        fs::write(&path, b"not-a-zip-package").unwrap();
        let error = DocumentIntelligence::with_defaults()
            .extract("review-3", "asset-3", &hash(), DOCX_MIME_TYPE, &path, 30)
            .unwrap_err();
        assert_eq!(error.code, "DOCX_PACKAGE_INVALID");
        assert!(!error.retryable);
    }

    #[test]
    fn unknown_format_is_not_routed_from_misleading_mime() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("contract.rtf");
        fs::write(&path, b"plain text").unwrap();
        let error = DocumentIntelligence::with_defaults()
            .extract("review-4", "asset-4", &hash(), DOCX_MIME_TYPE, &path, 40)
            .unwrap_err();
        assert_eq!(error.code, "DOCUMENT_FORMAT_UNSUPPORTED");
    }

    #[test]
    fn malformed_docx_xml_is_rejected() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("malformed.docx");
        write_test_docx(
            &path,
            r#"<w:document xmlns:w="urn:test"><w:body><w:p><w:r><w:t>broken</w:t></w:r></w:body></w:document>"#,
        );
        let error = DocumentIntelligence::with_defaults()
            .extract("review-5", "asset-5", &hash(), DOCX_MIME_TYPE, &path, 50)
            .unwrap_err();
        assert_eq!(error.code, "DOCX_DOCUMENT_XML_INVALID");
    }
}

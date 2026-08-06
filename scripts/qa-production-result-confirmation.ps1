[CmdletBinding()]
param(
    [string]$TemplatePath,
    [string]$HistoricalPdfPath,
    [string]$ArtifactDocxPath,
    [string]$ArtifactPdfPath,
    [string]$WordPdfOutputPath,
    [string]$ExpectedTemplateSha256 = "7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF",
    [string]$ExpectedHistoricalPdfSha256 = "BD56B931D02863B9DBC515D764D704F6F61B6D29CB1F0EF9CB86C2AA0A8D5546",
    [string]$ExpectedArtifactDocxSha256,
    [int]$ExpectedHistoricalPages = 22,
    [int]$ExpectedArtifactPages = 0,
    [switch]$SkipWordExport,
    [switch]$RequireWord,
    [switch]$OverwriteWordPdf,
    [switch]$KeepTemporaryWordPdf
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:Failures = [System.Collections.Generic.List[string]]::new()
$script:Warnings = [System.Collections.Generic.List[string]]::new()
$script:PassCount = 0

function Write-Pass {
    param([Parameter(Mandatory = $true)][string]$Message)
    $script:PassCount++
    Write-Host "[PASS] $Message" -ForegroundColor Green
}

function Write-Failure {
    param([Parameter(Mandatory = $true)][string]$Message)
    $script:Failures.Add($Message)
    Write-Host "[FAIL] $Message" -ForegroundColor Red
}

function Write-QaWarning {
    param([Parameter(Mandatory = $true)][string]$Message)
    $script:Warnings.Add($Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Assert-Qa {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if ($Condition) { Write-Pass $Message } else { Write-Failure $Message }
}

function Resolve-QaPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path)
}

function Get-Sha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
}

function Test-ExpectedHash {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Expected,
        [Parameter(Mandatory = $true)][string]$Label
    )
    $actual = Get-Sha256 $Path
    Write-Host "[INFO] $Label SHA-256: $actual"
    Assert-Qa ($actual -eq $Expected.ToUpperInvariant()) "$Label SHA-256 与冻结基线一致"
}

function Read-ZipXml {
    param([Parameter(Mandatory = $true)]$Entry)
    $stream = $Entry.Open()
    $reader = $null
    try {
        $settings = [System.Xml.XmlReaderSettings]::new()
        $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
        $settings.XmlResolver = $null
        $reader = [System.Xml.XmlReader]::Create($stream, $settings)
        $document = [System.Xml.XmlDocument]::new()
        $document.PreserveWhitespace = $true
        $document.XmlResolver = $null
        $document.Load($reader)
        return $document
    } finally {
        if ($null -ne $reader) { $reader.Dispose() }
        $stream.Dispose()
    }
}

function Convert-RelationshipPathToSourcePart {
    param([Parameter(Mandatory = $true)][string]$RelationshipPath)
    $normalized = $RelationshipPath.Replace("\", "/")
    if ($normalized -eq "_rels/.rels") { return "" }
    $marker = "/_rels/"
    $markerIndex = $normalized.LastIndexOf($marker, [System.StringComparison]::Ordinal)
    if ($markerIndex -lt 0 -or -not $normalized.EndsWith(".rels", [System.StringComparison]::OrdinalIgnoreCase)) { return $null }
    $parent = $normalized.Substring(0, $markerIndex)
    $fileName = $normalized.Substring($markerIndex + $marker.Length)
    $sourceName = $fileName.Substring(0, $fileName.Length - ".rels".Length)
    if ([string]::IsNullOrEmpty($parent)) { return $sourceName }
    return "$parent/$sourceName"
}

function Resolve-PackageTarget {
    param(
        [AllowEmptyString()][string]$SourcePart,
        [Parameter(Mandatory = $true)][string]$Target
    )
    $cleanTarget = [System.Uri]::UnescapeDataString($Target.Replace("\", "/")).TrimStart("/")
    $baseDirectory = ""
    if (-not [string]::IsNullOrEmpty($SourcePart)) {
        $slashIndex = $SourcePart.LastIndexOf("/", [System.StringComparison]::Ordinal)
        if ($slashIndex -ge 0) { $baseDirectory = $SourcePart.Substring(0, $slashIndex) }
    }
    $combined = if ([string]::IsNullOrEmpty($baseDirectory)) { $cleanTarget } else { "$baseDirectory/$cleanTarget" }
    $segments = [System.Collections.Generic.List[string]]::new()
    foreach ($segment in $combined.Split("/")) {
        if ([string]::IsNullOrEmpty($segment) -or $segment -eq ".") { continue }
        if ($segment -eq "..") {
            if ($segments.Count -eq 0) { return $null }
            $segments.RemoveAt($segments.Count - 1)
        } else {
            $segments.Add($segment)
        }
    }
    return [string]::Join("/", $segments)
}

function Test-DocxPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $requiredEntries = @("[Content_Types].xml", "_rels/.rels", "word/document.xml", "word/_rels/document.xml.rels")
    $zip = $null
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($Path)
        $entryMap = @{}
        foreach ($entry in $zip.Entries) {
            $name = $entry.FullName.Replace("\", "/")
            if ($entryMap.ContainsKey($name)) { Write-Failure "$Label ZIP 存在重复条目：$name" } else { $entryMap[$name] = $entry }
        }
        Assert-Qa ($zip.Entries.Count -gt 0) "$Label ZIP 可读取且非空"
        foreach ($requiredEntry in $requiredEntries) {
            Assert-Qa ($entryMap.ContainsKey($requiredEntry)) "$Label 包含必需条目 $requiredEntry"
        }

        $macroEntries = @($entryMap.Keys | Where-Object { $_ -match "(?i)(^|/)vba(Project\.bin|Data\.xml)$" })
        Assert-Qa ($macroEntries.Count -eq 0) "$Label 不包含 VBA 宏载荷"

        $xmlDocuments = @{}
        $xmlFailures = [System.Collections.Generic.List[string]]::new()
        foreach ($entryName in @($entryMap.Keys | Where-Object { $_ -match "(?i)\.(xml|rels)$" } | Sort-Object)) {
            try { $xmlDocuments[$entryName] = Read-ZipXml $entryMap[$entryName] } catch { $xmlFailures.Add("$entryName ($($_.Exception.Message))") }
        }
        Assert-Qa ($xmlFailures.Count -eq 0) "$Label 的全部 XML/RELS 均可安全解析"
        foreach ($failure in $xmlFailures) { Write-Host "       $failure" -ForegroundColor Red }

        $externalRelationships = [System.Collections.Generic.List[string]]::new()
        $macroRelationships = [System.Collections.Generic.List[string]]::new()
        $duplicateRelationshipIds = [System.Collections.Generic.List[string]]::new()
        $duplicateImageTargets = [System.Collections.Generic.List[string]]::new()
        $missingImageTargets = [System.Collections.Generic.List[string]]::new()
        $imageRelationshipCount = 0
        foreach ($relationshipPath in @($xmlDocuments.Keys | Where-Object { $_ -match "(?i)\.rels$" })) {
            $relationships = @($xmlDocuments[$relationshipPath].SelectNodes("//*[local-name()='Relationship']"))
            $ids = @{}
            $imageTargets = @{}
            $sourcePart = Convert-RelationshipPathToSourcePart $relationshipPath
            foreach ($relationship in $relationships) {
                $id = $relationship.GetAttribute("Id")
                $type = $relationship.GetAttribute("Type")
                $target = $relationship.GetAttribute("Target")
                $targetMode = $relationship.GetAttribute("TargetMode")
                if ($ids.ContainsKey($id)) { $duplicateRelationshipIds.Add("$relationshipPath::$id") } else { $ids[$id] = $true }
                if ($targetMode -eq "External") { $externalRelationships.Add("$relationshipPath::$id -> $target") }
                if ($type -match "(?i)(vbaProject|vbaData|customUI)") { $macroRelationships.Add("$relationshipPath::$id -> $type") }
                if ($type -match "(?i)/image$") {
                    $imageRelationshipCount++
                    if ($imageTargets.ContainsKey($target)) { $duplicateImageTargets.Add("$relationshipPath::$target") } else { $imageTargets[$target] = $true }
                    if ($targetMode -ne "External") {
                        $resolvedTarget = Resolve-PackageTarget $sourcePart $target
                        if ($null -eq $resolvedTarget -or -not $entryMap.ContainsKey($resolvedTarget)) { $missingImageTargets.Add("$relationshipPath::$id -> $target") }
                    }
                }
            }
        }
        Assert-Qa ($externalRelationships.Count -eq 0) "$Label 不包含 External relationship 外链"
        Assert-Qa ($macroRelationships.Count -eq 0) "$Label 不包含宏相关 relationship"
        Assert-Qa ($duplicateRelationshipIds.Count -eq 0) "$Label 各 relationship part 内的关系 ID 唯一"
        Assert-Qa ($duplicateImageTargets.Count -eq 0) "$Label 各 relationship part 内的图片目标唯一"
        Assert-Qa ($missingImageTargets.Count -eq 0) "$Label 的图片关系均指向包内真实文件"

        $visibleText = [System.Text.StringBuilder]::new()
        $highlightCount = 0
        $yellowShadingCount = 0
        $revisionCount = 0
        $externalFieldCount = 0
        $docPrIds = [System.Collections.Generic.List[string]]::new()
        $cNvPrIds = [System.Collections.Generic.List[string]]::new()
        foreach ($xmlPath in $xmlDocuments.Keys) {
            $document = $xmlDocuments[$xmlPath]
            foreach ($textNode in @($document.SelectNodes("//*[local-name()='t' or local-name()='instrText']"))) {
                [void]$visibleText.Append($textNode.InnerText)
                [void]$visibleText.Append(" ")
                if ($textNode.LocalName -eq "instrText" -and $textNode.InnerText -match '(?i)(HYPERLINK|INCLUDEPICTURE)\s+["'']?(https?|ftp|file):') { $externalFieldCount++ }
            }
            $highlightCount += $document.SelectNodes("//*[local-name()='highlight']").Count
            foreach ($shading in @($document.SelectNodes("//*[local-name()='shd']"))) {
                $fill = $shading.GetAttribute("fill", "http://schemas.openxmlformats.org/wordprocessingml/2006/main")
                if ($fill -match "(?i)^(yellow|FFFF00)$") { $yellowShadingCount++ }
            }
            $revisionCount += $document.SelectNodes("//*[local-name()='ins' or local-name()='del' or local-name()='moveFrom' or local-name()='moveTo']").Count
            foreach ($node in @($document.SelectNodes("//*[local-name()='docPr']"))) { $docPrIds.Add($node.GetAttribute("id")) }
            foreach ($node in @($document.SelectNodes("//*[local-name()='cNvPr']"))) { $cNvPrIds.Add($node.GetAttribute("id")) }
        }

        $placeholderPatterns = @(
            "\{\{[^{}\r\n]{1,200}\}\}", "\$\{[^{}\r\n]{1,200}\}", "<<[^<>\r\n]{1,200}>>",
            "\[\[[^\[\]\r\n]{1,200}\]\]", "<%[^%\r\n]{1,200}%>", "(?i)\b(TODO|TBD|PLACEHOLDER)\b", "请输入|待填写|待填入"
        )
        $placeholderHits = [System.Collections.Generic.List[string]]::new()
        foreach ($pattern in $placeholderPatterns) {
            foreach ($match in [regex]::Matches($visibleText.ToString(), $pattern)) {
                if (-not $placeholderHits.Contains($match.Value)) { $placeholderHits.Add($match.Value) }
            }
        }

        $contentTypesText = if ($xmlDocuments.ContainsKey("[Content_Types].xml")) { $xmlDocuments["[Content_Types].xml"].OuterXml } else { "" }
        Assert-Qa ($contentTypesText -notmatch "(?i)(macroEnabled|vbaProject)") "$Label Content Types 不声明宏格式"
        Assert-Qa ($externalFieldCount -eq 0) "$Label 不包含字段代码形式的外部 URL"
        Assert-Qa ($placeholderHits.Count -eq 0) "$Label 不包含未解析占位符"
        Assert-Qa (($highlightCount + $yellowShadingCount) -eq 0) "$Label 不包含高亮或黄色编辑标记"
        Assert-Qa ($revisionCount -eq 0) "$Label 不包含未接受的修订标记"
        $duplicateDocPrIds = @($docPrIds | Group-Object | Where-Object { $_.Count -gt 1 -and $_.Name -ne "" })
        $duplicateCNvPrIds = @($cNvPrIds | Group-Object | Where-Object { $_.Count -gt 1 -and $_.Name -ne "" })
        Assert-Qa ($duplicateDocPrIds.Count -eq 0) "$Label 的 wp:docPr 图片对象 ID 唯一"
        Assert-Qa ($duplicateCNvPrIds.Count -eq 0) "$Label 的 pic:cNvPr 图片对象 ID 唯一"
        Write-Host "[INFO] $Label ZIP 条目 $($zip.Entries.Count)，XML/RELS $($xmlDocuments.Count)，图片关系 $imageRelationshipCount。"
    } catch {
        Write-Failure "$Label DOCX/ZIP 审计异常：$($_.Exception.Message)"
    } finally {
        if ($null -ne $zip) { $zip.Dispose() }
    }
}

function Get-PdfPageCount {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 8) { return 0 }
    $latin1 = [System.Text.Encoding]::GetEncoding(28591).GetString($bytes)
    $pageObjects = [regex]::Matches($latin1, "/Type\s*/Page\b").Count
    if ($pageObjects -gt 0) { return $pageObjects }
    $counts = @([regex]::Matches($latin1, "/Count\s+(\d+)") | ForEach-Object { [int]$_.Groups[1].Value })
    if ($counts.Count -gt 0) { return ($counts | Measure-Object -Maximum).Maximum }
    return 0
}

function Test-PdfFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label,
        [int]$ExpectedPages = 0
    )
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $headerLength = [Math]::Min(8, $bytes.Length)
    $header = [System.Text.Encoding]::ASCII.GetString($bytes, 0, $headerLength)
    $tailStart = [Math]::Max(0, $bytes.Length - 2048)
    $tail = [System.Text.Encoding]::ASCII.GetString($bytes, $tailStart, $bytes.Length - $tailStart)
    Assert-Qa ($header.StartsWith("%PDF-", [System.StringComparison]::Ordinal)) "$Label 具有有效 PDF 文件头"
    Assert-Qa ($tail.Contains("%%EOF")) "$Label 具有 PDF EOF 标记"
    $pages = Get-PdfPageCount $Path
    Assert-Qa ($pages -gt 0) "$Label 可识别页数"
    if ($ExpectedPages -gt 0) { Assert-Qa ($pages -eq $ExpectedPages) "$Label 页数为预期 $ExpectedPages 页（实际 $pages 页）" } else { Write-Host "[INFO] $Label 页数：$pages" }
    return $pages
}

function Release-ComObject {
    param([object]$ComObject)
    if ($null -ne $ComObject -and [System.Runtime.InteropServices.Marshal]::IsComObject($ComObject)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($ComObject)
    }
}

function Export-ReadOnlyDocxWithWord {
    param(
        [Parameter(Mandatory = $true)][string]$DocxPath,
        [Parameter(Mandatory = $true)][string]$PdfPath,
        [int]$ExpectedPages = 0
    )
    $beforeProcessIds = @(Get-Process -Name WINWORD -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $word = $null
    $document = $null
    try {
        try { $word = New-Object -ComObject Word.Application } catch {
            if ($RequireWord) { Write-Failure "系统无法启动 Word COM，但指定了 -RequireWord：$($_.Exception.Message)" } else { Write-QaWarning "系统无可用 Word COM，跳过 DOCX 转 PDF。" }
            return
        }
        $word.Visible = $false
        $word.DisplayAlerts = 0
        try { $word.AutomationSecurity = 3 } catch { Write-QaWarning "无法设置 Word AutomationSecurity；仍以只读方式打开。" }
        Assert-Qa (Test-Path -LiteralPath $DocxPath -PathType Leaf) "Word 导出前 DOCX 文件存在"
        $hashBefore = Get-Sha256 $DocxPath
        $document = $word.Documents.Open($DocxPath, $false, $true)
        $wordPages = [int]$document.ComputeStatistics(2)
        Write-Host "[INFO] Word 只读渲染页数：$wordPages"
        if ($ExpectedPages -gt 0) { Assert-Qa ($wordPages -eq $ExpectedPages) "Word 渲染页数为预期 $ExpectedPages 页（实际 $wordPages 页）" }
        $document.ExportAsFixedFormat($PdfPath, 17)
        Assert-Qa (Test-Path -LiteralPath $PdfPath -PathType Leaf) "Word 导出后 PDF 文件存在"
        if (Test-Path -LiteralPath $PdfPath -PathType Leaf) {
            $pdfPages = Test-PdfFile $PdfPath "Word 导出 PDF" $ExpectedPages
            Assert-Qa ($pdfPages -eq $wordPages) "Word 统计页数与导出 PDF 页数一致"
            Write-Host "[INFO] Word 导出 PDF SHA-256: $(Get-Sha256 $PdfPath)"
        }
        Assert-Qa (Test-Path -LiteralPath $DocxPath -PathType Leaf) "Word 导出后源 DOCX 文件仍存在"
        Assert-Qa ((Get-Sha256 $DocxPath) -eq $hashBefore) "Word 只读导出未改变源 DOCX"
    } catch {
        Write-Failure "Word 只读导出失败：$($_.Exception.Message)"
    } finally {
        if ($null -ne $document) {
            try { $document.Close(0) } catch { Write-QaWarning "关闭 Word 文档时出现异常：$($_.Exception.Message)" }
            Release-ComObject $document
        }
        if ($null -ne $word) {
            try { $word.Quit() } catch { Write-QaWarning "退出 Word 时出现异常：$($_.Exception.Message)" }
            Release-ComObject $word
        }
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        Start-Sleep -Milliseconds 750
        $newWordProcesses = @(Get-Process -Name WINWORD -ErrorAction SilentlyContinue | Where-Object { $beforeProcessIds -notcontains $_.Id })
        foreach ($process in $newWordProcesses) {
            try { Stop-Process -Id $process.Id -Force -ErrorAction Stop } catch { Write-Failure "无法清理本次启动的 WINWORD 进程 $($process.Id)：$($_.Exception.Message)" }
        }
        Start-Sleep -Milliseconds 250
        $remaining = @(Get-Process -Name WINWORD -ErrorAction SilentlyContinue | Where-Object { $beforeProcessIds -notcontains $_.Id })
        Assert-Qa ($remaining.Count -eq 0) "本次 Word QA 未残留 WINWORD 进程"
    }
}

Write-Host "制作成果确认 v1 QA / 视觉验证门禁" -ForegroundColor Cyan
Write-Host "====================================" -ForegroundColor Cyan

if ([string]::IsNullOrWhiteSpace($TemplatePath)) {
    $TemplatePath = $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH
}
if ([string]::IsNullOrWhiteSpace($HistoricalPdfPath)) {
    $HistoricalPdfPath = $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH
}
if ([string]::IsNullOrWhiteSpace($TemplatePath) -or [string]::IsNullOrWhiteSpace($HistoricalPdfPath)) {
    throw "Pass -TemplatePath and -HistoricalPdfPath, or set BSAIGC_ACCEPTANCE_TEMPLATE_PATH and BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH."
}

$TemplatePath = Resolve-QaPath $TemplatePath
$HistoricalPdfPath = Resolve-QaPath $HistoricalPdfPath
if (-not [string]::IsNullOrWhiteSpace($ArtifactDocxPath)) { $ArtifactDocxPath = Resolve-QaPath $ArtifactDocxPath }
if (-not [string]::IsNullOrWhiteSpace($ArtifactPdfPath)) { $ArtifactPdfPath = Resolve-QaPath $ArtifactPdfPath }
if (-not [string]::IsNullOrWhiteSpace($WordPdfOutputPath)) { $WordPdfOutputPath = Resolve-QaPath $WordPdfOutputPath }

$inputHashesBefore = @{}
foreach ($inputPath in @($TemplatePath, $HistoricalPdfPath, $ArtifactDocxPath, $ArtifactPdfPath)) {
    if (-not [string]::IsNullOrWhiteSpace($inputPath) -and (Test-Path -LiteralPath $inputPath -PathType Leaf)) { $inputHashesBefore[$inputPath] = Get-Sha256 $inputPath }
}

Assert-Qa (Test-Path -LiteralPath $TemplatePath -PathType Leaf) "制作成果确认 v1 真实模板存在"
Assert-Qa (Test-Path -LiteralPath $HistoricalPdfPath -PathType Leaf) "制作成果确认历史 PDF 存在"
if (Test-Path -LiteralPath $TemplatePath -PathType Leaf) {
    Test-ExpectedHash $TemplatePath $ExpectedTemplateSha256 "真实模板"
    Test-DocxPackage $TemplatePath "真实模板"
}
if (Test-Path -LiteralPath $HistoricalPdfPath -PathType Leaf) {
    Test-ExpectedHash $HistoricalPdfPath $ExpectedHistoricalPdfSha256 "历史 PDF"
    [void](Test-PdfFile $HistoricalPdfPath "历史 PDF" $ExpectedHistoricalPages)
}

if (-not [string]::IsNullOrWhiteSpace($ArtifactDocxPath)) {
    Assert-Qa (Test-Path -LiteralPath $ArtifactDocxPath -PathType Leaf) "待验收 DOCX 产物存在"
    if (Test-Path -LiteralPath $ArtifactDocxPath -PathType Leaf) {
        $artifactHash = Get-Sha256 $ArtifactDocxPath
        Write-Host "[INFO] 待验收 DOCX SHA-256: $artifactHash"
        if (-not [string]::IsNullOrWhiteSpace($ExpectedArtifactDocxSha256)) { Assert-Qa ($artifactHash -eq $ExpectedArtifactDocxSha256.ToUpperInvariant()) "待验收 DOCX SHA-256 与指定值一致" }
        Test-DocxPackage $ArtifactDocxPath "待验收 DOCX"
    }
}
if (-not [string]::IsNullOrWhiteSpace($ArtifactPdfPath)) {
    Assert-Qa (Test-Path -LiteralPath $ArtifactPdfPath -PathType Leaf) "待验收 PDF 产物存在"
    if (Test-Path -LiteralPath $ArtifactPdfPath -PathType Leaf) {
        Write-Host "[INFO] 待验收 PDF SHA-256: $(Get-Sha256 $ArtifactPdfPath)"
        [void](Test-PdfFile $ArtifactPdfPath "待验收 PDF" $ExpectedArtifactPages)
    }
}

$temporaryRoot = $null
$wordPdfIsTemporary = $false
if (-not $SkipWordExport -and -not [string]::IsNullOrWhiteSpace($ArtifactDocxPath) -and (Test-Path -LiteralPath $ArtifactDocxPath -PathType Leaf)) {
    if ([string]::IsNullOrWhiteSpace($WordPdfOutputPath)) {
        $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("bsaigc-production-result-confirmation-qa-" + [Guid]::NewGuid().ToString("N"))
        [void](New-Item -ItemType Directory -Path $temporaryRoot)
        $WordPdfOutputPath = Join-Path $temporaryRoot "production-result-confirmation-word-qa.pdf"
        $wordPdfIsTemporary = $true
    } else {
        $outputDirectory = Split-Path -Parent $WordPdfOutputPath
        if (-not (Test-Path -LiteralPath $outputDirectory -PathType Container)) { [void](New-Item -ItemType Directory -Path $outputDirectory) }
    }
    $inputPaths = @($TemplatePath, $HistoricalPdfPath, $ArtifactDocxPath, $ArtifactPdfPath) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    if ($inputPaths -contains $WordPdfOutputPath) {
        Write-Failure "Word PDF 输出路径不得覆盖任何输入文件：$WordPdfOutputPath"
    } elseif ((Test-Path -LiteralPath $WordPdfOutputPath) -and -not $OverwriteWordPdf) {
        Write-Failure "Word PDF 输出已存在；请改用新路径或传入 -OverwriteWordPdf：$WordPdfOutputPath"
    } else {
        if (Test-Path -LiteralPath $WordPdfOutputPath) { Remove-Item -LiteralPath $WordPdfOutputPath -Force }
        Export-ReadOnlyDocxWithWord $ArtifactDocxPath $WordPdfOutputPath $ExpectedArtifactPages
    }
}

foreach ($entry in $inputHashesBefore.GetEnumerator()) {
    $stillExists = Test-Path -LiteralPath $entry.Key -PathType Leaf
    Assert-Qa $stillExists "QA 完成后输入文件仍存在：$($entry.Key)"
    if ($stillExists) { Assert-Qa ((Get-Sha256 $entry.Key) -eq $entry.Value) "QA 完成后输入文件 SHA-256 未改变：$($entry.Key)" }
}

if ($wordPdfIsTemporary -and -not $KeepTemporaryWordPdf -and $null -ne $temporaryRoot) {
    $resolvedTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd("\")
    $resolvedCleanup = [System.IO.Path]::GetFullPath($temporaryRoot)
    $safePrefix = Join-Path $resolvedTemp "bsaigc-production-result-confirmation-qa-"
    if ($resolvedCleanup.StartsWith($safePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedCleanup -Recurse -Force
        Write-Pass "已清理本次 Word QA 临时目录"
    } else {
        Write-Failure "拒绝清理未通过路径边界检查的临时目录：$resolvedCleanup"
    }
} elseif ($wordPdfIsTemporary -and $KeepTemporaryWordPdf) {
    Write-Host "[INFO] 保留 Word QA PDF：$WordPdfOutputPath"
}

Write-Host ""
Write-Host "QA 汇总：PASS=$script:PassCount WARN=$($script:Warnings.Count) FAIL=$($script:Failures.Count)" -ForegroundColor Cyan
foreach ($warning in $script:Warnings) { Write-Host "  WARN: $warning" -ForegroundColor Yellow }
if ($script:Failures.Count -gt 0) {
    foreach ($failure in $script:Failures) { Write-Host "  FAIL: $failure" -ForegroundColor Red }
    exit 1
}
Write-Host "制作成果确认 v1 QA 门禁通过。仍须按 docs/PRODUCTION_RESULT_CONFIRMATION_QA_20260729.md 完成人工逐页视觉验收。" -ForegroundColor Green
exit 0

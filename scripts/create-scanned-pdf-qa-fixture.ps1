param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\.runtime\qa-fixtures"),
    [switch]$KeepIntermediates
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
[System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null

[object[]]$browserCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft\Edge\Application\msedge.exe"),
    (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Google\Chrome\Application\chrome.exe"),
    (Join-Path $env:ProgramFiles "Google\Chrome\Application\chrome.exe")
) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) }
if ($browserCandidates.Count -eq 0) {
    throw "未找到 Microsoft Edge 或 Google Chrome，无法生成扫描 PDF QA 样本。"
}
$browser = $browserCandidates[0]
$work = Join-Path $OutputDirectory ".scanned-pdf-build"
if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
[System.IO.Directory]::CreateDirectory($work) | Out-Null

$sourceHtml = Join-Path $work "source.html"
$scanPng = Join-Path $work "scan.png"
$imageHtml = Join-Path $work "image.html"
$outputPdf = Join-Path $OutputDirectory "04-scanned-contract-ocr.pdf"

$html = @"
<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>
@page { size: A4; margin: 0; }
* { box-sizing: border-box; }
html, body { width: 1240px; height: 1754px; margin: 0; overflow: hidden; background: white; }
body { font-family: "Microsoft YaHei", "SimSun", sans-serif; color: #171717; }
.page { width: 1240px; height: 1754px; padding: 96px 112px; background: white; }
h1 { text-align: center; font-size: 42px; margin: 0 0 48px; letter-spacing: 2px; }
p { font-size: 25px; line-height: 1.78; margin: 14px 0; text-align: justify; }
.meta { font-size: 23px; line-height: 1.65; margin-bottom: 30px; }
.sign { margin-top: 54px; display: grid; grid-template-columns: 1fr 1fr; gap: 50px; }
</style></head><body><main class="page">
<h1>商务视频制作服务合同（扫描件 QA 样本）</h1>
<div class="meta">合同编号：BSAIGC-QA-OCR-004<br>甲方：华南示例商业管理有限公司<br>乙方：半山示例文化传媒有限公司</div>
<p><strong>一、项目内容。</strong>乙方为甲方完成一条品牌宣传片及三条短视频，包括需求梳理、脚本、拍摄、剪辑、调色、字幕与约定格式交付。</p>
<p><strong>二、合同金额。</strong>合同含税总价为人民币捌万陆仟元整（¥86,000.00），税率为6%。范围外新增工作须经双方书面确认后执行。</p>
<p><strong>三、付款安排。</strong>合同签署后五个工作日内支付50%首付款；初版提交后支付30%；最终验收通过且收到合规发票后十个工作日内支付20%尾款。</p>
<p><strong>四、交付与验收。</strong>乙方于2026年8月28日前提交初版。甲方应在收到每版后五个工作日内一次性书面反馈。双方以已确认需求、脚本、分辨率、时长、字幕准确性和无明显技术瑕疵作为验收标准。</p>
<p><strong>五、修改。</strong>合同价包含两轮常规修改。方向变化、脚本推翻、补拍或新增版本另行评估费用和周期。</p>
<p><strong>六、知识产权与保密。</strong>甲方保证其提供素材合法。全部款项付清后，甲方取得最终成片在约定业务范围内的使用权。双方对未公开资料承担三年保密义务。</p>
<p><strong>七、违约与争议。</strong>逾期履行每日按对应未履行金额0.05%承担违约金，累计不超过合同总价10%。争议协商不成，向广州市海珠区有管辖权的人民法院起诉。</p>
<div class="sign"><p>甲方（盖章）：<br><br>日期：2026年7月21日</p><p>乙方（盖章）：<br><br>日期：2026年7月21日</p></div>
</main></body></html>
"@
[System.IO.File]::WriteAllText($sourceHtml, $html, [System.Text.UTF8Encoding]::new($false))

$sourceUri = ([System.Uri]$sourceHtml).AbsoluteUri
& $browser --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 --window-size=1240,1754 --screenshot=$scanPng $sourceUri | Out-Null
if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $scanPng -PathType Leaf)) { throw "浏览器截图生成失败。" }

$pngBytes = [System.IO.File]::ReadAllBytes($scanPng)
$pngBase64 = [Convert]::ToBase64String($pngBytes)
$imageOnlyHtml = @"
<!doctype html><html><head><meta charset="utf-8"><style>@page{size:A4;margin:0}html,body{margin:0;width:210mm;height:297mm;overflow:hidden}img{display:block;width:210mm;height:297mm;object-fit:fill}</style></head><body><img alt="" src="data:image/png;base64,$pngBase64"></body></html>
"@
[System.IO.File]::WriteAllText($imageHtml, $imageOnlyHtml, [System.Text.UTF8Encoding]::new($false))
$imageUri = ([System.Uri]$imageHtml).AbsoluteUri
if (Test-Path -LiteralPath $outputPdf) { Remove-Item -LiteralPath $outputPdf -Force }
& $browser --headless=new --disable-gpu --no-pdf-header-footer --print-to-pdf=$outputPdf $imageUri | Out-Null
if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $outputPdf -PathType Leaf)) { throw "扫描 PDF 生成失败。" }

$sha256 = (Get-FileHash -LiteralPath $outputPdf -Algorithm SHA256).Hash.ToLowerInvariant()
$record = [ordered]@{
    schemaVersion = "bsaigc.scanned-pdf-fixture.v1"
    file = [System.IO.Path]::GetFileName($outputPdf)
    sha256 = $sha256
    byteSize = (Get-Item -LiteralPath $outputPdf).Length
    imageOnly = $true
    expectedOcrMarkers = @("商务视频制作服务合同", "BSAIGC-QA-OCR-004", "86000", "验收标准", "知识产权")
    generatedAt = "2026-07-21T00:00:00+08:00"
}
$manifestPath = Join-Path $OutputDirectory "04-scanned-contract-ocr.json"
[System.IO.File]::WriteAllText($manifestPath, (($record | ConvertTo-Json -Depth 5) + "`n"), [System.Text.UTF8Encoding]::new($false))

if (-not $KeepIntermediates) { Remove-Item -LiteralPath $work -Recurse -Force }
$record | ConvertTo-Json -Depth 5
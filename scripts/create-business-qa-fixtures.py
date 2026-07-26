#!/usr/bin/env python3
"""Generate deterministic Chinese business QA fixtures as real OOXML DOCX files."""

from __future__ import annotations

import hashlib
import io
import json
import os
import sys
import zipfile
from pathlib import Path
from xml.etree import ElementTree as ET
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".runtime" / "qa-fixtures"
DOC_TIME = "2026-07-21T00:00:00Z"
ZIP_TIME = (1980, 1, 1, 0, 0, 0)
W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
RELS = "http://schemas.openxmlformats.org/package/2006/relationships"
CT = "http://schemas.openxmlformats.org/package/2006/content-types"
OFFICE_DOC_REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
REQUIRED_PARTS = {
    "[Content_Types].xml", "_rels/.rels", "docProps/app.xml",
    "docProps/core.xml", "word/_rels/document.xml.rels",
    "word/document.xml", "word/settings.xml", "word/styles.xml",
}
BUSINESS_SECTIONS = ("客户", "项目", "金额", "付款", "交付", "验收", "违约", "保密", "知识产权", "争议")

FIXTURES = (
    {
        "id": "contract-standard-low-risk",
        "file": "01-standard-low-risk-contract.docx",
        "title": "商务视频制作服务合同（标准低风险 QA 样本）",
        "number": "BSAIGC-QA-LOW-001",
        "riskLevel": "low",
        "risks": [],
        "missing": [],
        "markers": ["华东示例置业有限公司", "¥120,000.00", "两轮常规修改", "责任上限"],
        "sections": [
            ("一、客户与合同主体", [
                "甲方（客户）：华东示例置业有限公司；统一社会信用代码：91310000QA0000001A；地址：上海市浦东新区示例路100号；联系人：陈示例。",
                "乙方（服务方）：南方示例文化传媒有限公司；统一社会信用代码：91440000QA0000002B；地址：广州市海珠区样本路88号；联系人：林示例。",
            ]),
            ("二、项目内容", [
                "项目名称：澄明生活美学品牌视频项目。乙方按双方确认的需求简报和脚本，完成一条三分钟品牌主片及三条十五秒竖版短视频。",
                "项目范围包括创意、脚本、两天拍摄、剪辑、基础调色、字幕、版权音乐和约定格式输出；范围外工作须书面确认后另行报价。",
            ]),
            ("三、合同金额", [
                "合同总价为含税人民币壹拾贰万元整（¥120,000.00），税率为6%，包含本合同范围内人员、设备、差旅、后期和两轮常规修改费用。",
            ]),
            ("四、付款安排", [
                "首付款为合同总价40%，合同生效且收到合规发票后五个工作日内支付；脚本定稿后五个工作日内支付40%进度款；最终验收通过且收到发票后十个工作日内支付20%尾款。",
            ]),
            ("五、交付安排", [
                "乙方于2026年8月5日前提交脚本定稿，8月20日前完成拍摄，9月5日前提交第一版成片，并在收到完整反馈后三个工作日内提交修改版。",
                "最终交付包括4K横版主片、1080×1920竖版短视频、无字幕洁净版和字幕文件。",
            ]),
            ("六、验收标准", [
                "验收以确认的需求简报、定稿脚本、分辨率、时长、字幕准确性和无明显技术瑕疵为标准。甲方应在收到每版后五个工作日内一次性书面反馈。",
                "合同价包含两轮常规修改。甲方逾期未提出有明确依据的异议，视为该版验收通过；新增需求或方向变更另行确认费用和周期。",
            ]),
            ("七、违约责任", [
                "任一方逾期履行，每日按对应未履行金额0.05%承担违约金，累计不超过合同总价10%。可预见直接损失的累计赔偿责任上限为合同总价20%。",
                "不可抗力导致延期的，受影响方及时通知并举证，双方据实调整周期，互不承担违约责任。",
            ]),
            ("八、保密与知识产权", [
                "双方对未公开经营信息、客户资料、脚本和报价承担保密义务，保密期限为合同终止后三年，依法披露或已公开信息除外。",
                "甲方素材归甲方或合法权利人所有。甲方付清款项后取得最终成片在约定业务范围内的永久使用权；乙方既有模板、方法、工具和通用技术仍归乙方所有。",
            ]),
            ("九、争议解决", [
                "本合同适用中华人民共和国法律。争议先友好协商；协商不成的，任一方可向合同签订地广州市海珠区有管辖权的人民法院起诉。",
            ]),
        ],
    },
    {
        "id": "contract-high-risk",
        "file": "02-high-risk-contract.docx",
        "title": "商务视频制作服务合同（高风险 QA 样本）",
        "number": "BSAIGC-QA-HIGH-001",
        "riskLevel": "critical",
        "risks": [
            {"code": "unilateral_change", "severity": "critical", "title": "甲方可单方变更且乙方无偿承担", "evidenceContains": ["甲方有权随时单方变更", "不得追加费用"]},
            {"code": "unlimited_liability", "severity": "critical", "title": "乙方承担无限责任及间接损失", "evidenceContains": ["不受合同金额限制", "预期利益"]},
            {"code": "payment_terms_ambiguous", "severity": "high", "title": "付款节点和期限由甲方单方流程决定", "evidenceContains": ["资金安排允许时", "不构成逾期"]},
            {"code": "acceptance_criteria_ambiguous", "severity": "high", "title": "以甲方主观满意为唯一验收标准", "evidenceContains": ["甲方主观满意", "不限次数"]},
            {"code": "ip_full_assignment_overreach", "severity": "high", "title": "成果及乙方既有知识产权全部转让", "evidenceContains": ["乙方既有材料", "无偿、永久、不可撤销"]},
            {"code": "unfavorable_dispute_venue", "severity": "high", "title": "争议管辖地单方有利于甲方", "evidenceContains": ["北京市朝阳区人民法院"]},
        ],
        "missing": [],
        "markers": ["甲方有权随时单方变更", "不受合同金额限制", "甲方主观满意", "北京市朝阳区人民法院"],
        "sections": [
            ("一、客户与合同主体", [
                "甲方（客户）：北方示例商业管理有限公司；统一社会信用代码：91110000QA0000003C；地址：北京市朝阳区风险路66号；联系人：赵示例。",
                "乙方（服务方）：南方示例文化传媒有限公司；统一社会信用代码：91440000QA0000002B；地址：广州市海珠区样本路88号；联系人：林示例。",
            ]),
            ("二、项目内容及单方变更", [
                "项目名称：北方商业年度视频传播项目。乙方负责年度创意、脚本、拍摄、剪辑、动画、配音、音乐和全部源文件交付。",
                "甲方有权随时单方变更项目范围、数量、风格、标准、人员和交付时间，乙方必须无条件执行，不得追加费用、延长周期或拒绝实施。",
            ]),
            ("三、合同金额", [
                "合同总价暂定为含税人民币叁拾万元整（¥300,000.00）。无论甲方增加多少工作内容，乙方均不得调整合同金额。",
            ]),
            ("四、付款安排", [
                "付款比例、节点和日期由甲方根据项目表现、内部审批及资金计划另行决定。甲方完成内部流程且资金安排允许时支付；审批、预算或资金原因导致的延迟不构成逾期，乙方不得暂停服务。",
            ]),
            ("五、交付安排", [
                "乙方应在甲方每次通知的时间内完成交付。甲方可临时提前日期，乙方须自行增加人员和设备，全部成本由乙方承担。",
            ]),
            ("六、验收标准", [
                "所有成果以甲方主观满意为唯一验收标准，甲方无须在固定期限内反馈或说明依据。乙方须不限次数、无偿修改，直至甲方书面表示完全满意。",
                "甲方使用、发布或传播成果不视为验收通过，也不影响甲方继续提出修改和索赔。",
            ]),
            ("七、违约责任与无限责任", [
                "乙方对项目一切后果承担全部且无限的赔偿责任，包括直接损失、间接损失、商誉损失、预期利益、律师费及关联方损失，且不受合同金额限制。",
                "甲方可同时要求退还全部已付款、支付合同总价三倍违约金并继续履行；甲方迟延提供资料、变更需求或迟延付款均不承担责任。",
            ]),
            ("八、保密与知识产权全转让", [
                "乙方承担永久且单方的保密义务，甲方无需承担对等保密责任。",
                "全部成片、草稿、脚本、源文件、未采用方案、素材、模板、方法、通用组件及乙方既有材料的全部知识产权均无偿、永久、不可撤销地转让给甲方，且不以付款为前提。",
            ]),
            ("九、争议解决", [
                "本合同适用中华人民共和国法律。任何争议均由甲方住所地北京市朝阳区人民法院专属管辖，乙方承担甲方处理争议产生的全部费用。",
            ]),
        ],
    },
    {
        "id": "contract-missing-fields",
        "file": "03-missing-fields-contract.docx",
        "title": "商务视频制作服务合同（缺字段 QA 样本）",
        "number": "BSAIGC-QA-MISSING-001",
        "riskLevel": "critical",
        "risks": [
            {"code": "customer_identity_missing", "severity": "critical", "title": "客户法定主体信息缺失", "evidenceContains": ["甲方（客户）：【未填写】"]},
            {"code": "provider_identity_missing", "severity": "critical", "title": "服务方法定主体信息缺失", "evidenceContains": ["乙方（服务方）：【未填写】"]},
            {"code": "project_scope_missing", "severity": "high", "title": "项目名称和服务范围缺失", "evidenceContains": ["项目名称：【未填写】", "服务范围：【待补充】"]},
            {"code": "contract_amount_missing", "severity": "high", "title": "金额、税率和币种缺失", "evidenceContains": ["合同金额：【未填写】"]},
            {"code": "payment_schedule_missing", "severity": "high", "title": "付款比例、节点和期限缺失", "evidenceContains": ["付款安排：【双方另行约定】"]},
            {"code": "delivery_deadline_missing", "severity": "high", "title": "交付日期和清单缺失", "evidenceContains": ["交付日期：【待通知】"]},
            {"code": "acceptance_terms_missing", "severity": "high", "title": "验收标准、期限和修改轮次缺失", "evidenceContains": ["验收标准：【未约定】"]},
            {"code": "breach_liability_missing", "severity": "medium", "title": "违约责任没有可执行约定", "evidenceContains": ["违约责任：【另行协商】"]},
            {"code": "confidentiality_ip_missing", "severity": "high", "title": "保密和知识产权边界缺失", "evidenceContains": ["保密期限：【未填写】", "知识产权归属：【未填写】"]},
            {"code": "dispute_resolution_missing", "severity": "medium", "title": "争议管辖缺失", "evidenceContains": ["争议管辖：【未填写】"]},
        ],
        "missing": [
            "甲乙方名称、统一社会信用代码、地址和联系人", "项目名称、范围和成果数量",
            "金额、币种、税率和含税口径", "付款比例、节点、期限和发票条件",
            "交付日期、格式和清单", "验收标准、反馈期限、修改轮次和视为通过条件",
            "违约金、损失范围和责任上限", "保密范围、期限、例外和知识产权归属",
            "适用法律、协商机制和争议管辖",
        ],
        "markers": ["甲方（客户）：【未填写】", "合同金额：【未填写】", "验收标准：【未约定】", "争议管辖：【未填写】"],
        "sections": [
            ("一、客户与合同主体", [
                "甲方（客户）：【未填写】；统一社会信用代码：【未填写】；地址：【未填写】；联系人：【未填写】。",
                "乙方（服务方）：【未填写】；统一社会信用代码：【未填写】；地址：【未填写】；联系人：【未填写】。",
            ]),
            ("二、项目内容", ["项目名称：【未填写】。服务范围：【待补充】。成果数量、时长、比例、语言版本和源文件要求均未录入。"]),
            ("三、合同金额", ["合同金额：【未填写】。币种、大小写金额、税率、含税口径和费用范围均未录入。"]),
            ("四、付款安排", ["付款安排：【双方另行约定】。首付款、进度款、尾款比例，付款节点、期限、发票和收款条件均未录入。"]),
            ("五、交付安排", ["交付日期：【待通知】。交付清单、分辨率、格式、渠道、修改响应时间和延期处理均未录入。"]),
            ("六、验收标准", ["验收标准：【未约定】。反馈期限、确认方式、修改轮次、异议依据和逾期是否视为验收通过均未录入。"]),
            ("七、违约责任", ["违约责任：【另行协商】。违约金、损失范围、责任上限、免责事由和不可抗力处理均未录入。"]),
            ("八、保密与知识产权", [
                "保密范围：【未填写】；保密期限：【未填写】；披露例外：【未填写】。",
                "知识产权归属：【未填写】；素材权属保证、既有工具保留、成果使用范围和案例展示授权均未录入。",
            ]),
            ("九、争议解决", ["适用法律：【未填写】；协商期限：【未填写】；争议管辖：【未填写】。"]),
        ],
    },
)


def xb(text: str) -> bytes:
    return text.encode("utf-8")


def paragraph(text: str, style: str = "Normal") -> str:
    return (
        "<w:p><w:pPr><w:pStyle w:val=\"" + style + "\"/></w:pPr>"
        "<w:r><w:rPr><w:lang w:val=\"zh-CN\" w:eastAsia=\"zh-CN\"/></w:rPr>"
        "<w:t>" + escape(text) + "</w:t></w:r></w:p>"
    )


def main_document(fixture: dict) -> bytes:
    items = [
        paragraph(fixture["title"], "Title"),
        paragraph("本文件为全量虚构的自动化 QA 样本，不构成真实交易文件。", "Notice"),
        paragraph("合同编号：" + fixture["number"], "Meta"),
        paragraph("签订日期：2026年7月21日", "Meta"),
    ]
    for heading, values in fixture["sections"]:
        items.append(paragraph(heading, "Heading1"))
        items.extend(paragraph(value) for value in values)
    items.extend([
        paragraph("（以下无正文，为签署页）", "Meta"),
        paragraph("甲方（盖章）：________________    授权代表：________________", "Signature"),
        paragraph("乙方（盖章）：________________    授权代表：________________", "Signature"),
    ])
    body = "".join(items)
    return xb(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document xmlns:w="{W}"><w:body>{body}'
        '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" '
        'w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>'
        '</w:body></w:document>'
    )


def styles() -> bytes:
    return xb(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr>'
        '<w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="等线"/>'
        '<w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="zh-CN" w:eastAsia="zh-CN"/>'
        '</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>'
        '<w:spacing w:after="120" w:line="360" w:lineRule="auto"/>'
        '</w:pPr></w:pPrDefault></w:docDefaults>'
        '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>'
        '<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:qFormat/>'
        '<w:pPr><w:jc w:val="center"/><w:spacing w:after="280"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>'
        '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:qFormat/>'
        '<w:pPr><w:keepNext/><w:spacing w:before="240" w:after="100"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style>'
        '<w:style w:type="paragraph" w:styleId="Notice"><w:name w:val="Fixture Notice"/><w:basedOn w:val="Normal"/>'
        '<w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:color w:val="9C0006"/><w:sz w:val="18"/></w:rPr></w:style>'
        '<w:style w:type="paragraph" w:styleId="Meta"><w:name w:val="Fixture Meta"/><w:basedOn w:val="Normal"/>'
        '<w:rPr><w:color w:val="666666"/></w:rPr></w:style>'
        '<w:style w:type="paragraph" w:styleId="Signature"><w:name w:val="Signature"/><w:basedOn w:val="Normal"/>'
        '<w:pPr><w:spacing w:before="260"/></w:pPr></w:style></w:styles>'
    )


def parts(fixture: dict) -> dict[str, bytes]:
    return {
        "[Content_Types].xml": xb(
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="{CT}">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
            '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>'
            '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>'
            '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
            '<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>'
            '</Types>'
        ),
        "_rels/.rels": xb(
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS}">'
            f'<Relationship Id="rId1" Type="{OFFICE_DOC_REL}" Target="word/document.xml"/>'
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>'
            '<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>'
            '</Relationships>'
        ),
        "docProps/app.xml": xb(
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">'
            '<Application>BSAIGC QA Fixture Generator</Application><AppVersion>1.0</AppVersion>'
            '<Company>BSAIGC</Company></Properties>'
        ),
        "docProps/core.xml": xb(
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
            'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" '
            'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
            f'<dc:title>{escape(fixture["title"])}</dc:title><dc:creator>BSAIGC QA Fixture Generator</dc:creator>'
            f'<dcterms:created xsi:type="dcterms:W3CDTF">{DOC_TIME}</dcterms:created>'
            f'<dcterms:modified xsi:type="dcterms:W3CDTF">{DOC_TIME}</dcterms:modified></cp:coreProperties>'
        ),
        "word/document.xml": main_document(fixture),
        "word/styles.xml": styles(),
        "word/settings.xml": xb(
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{W}">'
            '<w:zoom w:percent="100"/><w:compat><w:compatSetting w:name="compatibilityMode" '
            'w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>'
        ),
        "word/_rels/document.xml.rels": xb(
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS}">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>'
            '</Relationships>'
        ),
    }


def zip_info(name: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, ZIP_TIME)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.create_system = 0
    info.create_version = info.extract_version = 20
    info.flag_bits = info.internal_attr = info.external_attr = 0
    info.extra = info.comment = b""
    return info


def build(fixture: dict) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, payload in parts(fixture).items():
            archive.writestr(zip_info(name), payload, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    return buffer.getvalue()


def validate(fixture: dict, payload: bytes) -> dict[str, object]:
    if not payload.startswith(b"PK\x03\x04"):
        raise RuntimeError(f"{fixture['file']}: not a ZIP package")
    with zipfile.ZipFile(io.BytesIO(payload), "r") as archive:
        names = set(archive.namelist())
        missing = REQUIRED_PARTS - names
        if missing:
            raise RuntimeError(f"{fixture['file']}: missing parts {sorted(missing)}")
        if archive.testzip() is not None:
            raise RuntimeError(f"{fixture['file']}: ZIP CRC failed")
        parsed = {}
        for name in sorted(names):
            if name.endswith(".xml") or name.endswith(".rels"):
                parsed[name] = ET.fromstring(archive.read(name))
        overrides = parsed["[Content_Types].xml"].findall(f"{{{CT}}}Override")
        if not any(x.get("PartName") == "/word/document.xml" for x in overrides):
            raise RuntimeError(f"{fixture['file']}: missing document content type")
        relationships = parsed["_rels/.rels"].findall(f"{{{RELS}}}Relationship")
        if not any(x.get("Type") == OFFICE_DOC_REL and x.get("Target") == "word/document.xml" for x in relationships):
            raise RuntimeError(f"{fixture['file']}: missing officeDocument relationship")
        document = parsed["word/document.xml"]
        if document.tag != f"{{{W}}}document":
            raise RuntimeError(f"{fixture['file']}: invalid Word document root")
        text = "\n".join(x.text or "" for x in document.iter(f"{{{W}}}t"))
    if "?" in text or "\ufffd" in text:
        raise RuntimeError(f"{fixture['file']}: Chinese text contains replacement characters")
    cjk = sum("\u4e00" <= char <= "\u9fff" for char in text)
    if cjk < 100:
        raise RuntimeError(f"{fixture['file']}: too little Chinese text")
    absent = [value for value in BUSINESS_SECTIONS if value not in text]
    if absent:
        raise RuntimeError(f"{fixture['file']}: missing business sections {absent}")
    absent = [value for value in fixture["markers"] if value not in text]
    if absent:
        raise RuntimeError(f"{fixture['file']}: missing expected markers {absent}")
    return {
        "zipRequiredParts": True, "zipCrc": True, "xmlParse": True,
        "officeDocumentRelationship": True, "wordDocumentRoot": True,
        "utf8ChineseText": True, "noQuestionMarkReplacement": True,
        "requiredBusinessSections": True, "cjkCharacterCount": cjk,
    }


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def write_atomic(path: Path, payload: bytes) -> None:
    temporary = path.with_name("." + path.name + ".tmp")
    temporary.write_bytes(payload)
    os.replace(temporary, path)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    entries = []
    for fixture in FIXTURES:
        first = build(fixture)
        second = build(fixture)
        if first != second or digest(first) != digest(second):
            raise RuntimeError(f"{fixture['file']}: build is not deterministic")
        checks = validate(fixture, first)
        write_atomic(OUT / fixture["file"], first)
        entries.append({
            "id": fixture["id"], "file": fixture["file"], "sha256": digest(first),
            "byteSize": len(first), "expectedRiskLevel": fixture["riskLevel"],
            "expectedRisks": fixture["risks"], "expectedMissingFields": fixture["missing"],
            "requiredBusinessSections": list(BUSINESS_SECTIONS),
            "validation": {**checks, "byteForByteDeterministic": True, "sha256Stable": True},
        })
    manifest = {
        "schemaVersion": "bsaigc.business-qa-fixtures.v1",
        "fixtureSetId": "cn-business-contracts-2026-07-21",
        "description": "用于商务工作台合同解析、规则审查和 Agent 审查的全量虚构中文 DOCX 样本。",
        "syntheticData": True,
        "outputDirectory": ".runtime/qa-fixtures",
        "determinism": {
            "documentPropertiesTimestamp": DOC_TIME, "zipEntryTimestamp": "1980-01-01T00:00:00",
            "zipCompression": "deflate-9", "xmlEncoding": "UTF-8",
            "eachDocumentBuiltTwiceInMemory": True,
        },
        "fixtures": entries,
    }
    manifest_path = OUT / "manifest.json"
    write_atomic(manifest_path, (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps({
        "outputDirectory": str(OUT),
        "documents": [{"file": x["file"], "sha256": x["sha256"], "bytes": x["byteSize"], "verified": True} for x in entries],
        "manifest": str(manifest_path), "allChecksPassed": True,
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
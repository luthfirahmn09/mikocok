#!/usr/bin/env python3
"""Writes the sample .docx/.xlsx files the unit tests read (to $TMPDIR/mikocok-samples)."""
import os, re, struct, tempfile, zipfile, zlib

OUT = os.path.join(tempfile.gettempdir(), "mikocok-samples")
os.makedirs(OUT, exist_ok=True)


def png(w, h, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


W = ('xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
     'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
     'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" '
     'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"')

styles = f'''<?xml version="1.0"?><w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/><w:spacing w:after="240"/></w:pPr><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/><w:b/><w:sz w:val="52"/><w:color w:val="1F3864"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="240" w:after="80"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:color w:val="2E74B5"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/></w:pPr></w:style></w:styles>'''

numbering = f'''<?xml version="1.0"?><w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="&#xF0B7;"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>'''


def p(t, style=None):
    s = f'<w:pStyle w:val="{style}"/>' if style else ""
    return f'<w:p><w:pPr>{s}</w:pPr><w:r><w:t xml:space="preserve">{t}</w:t></w:r></w:p>'


def li(t, n):
    return (f'<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="{n}"/></w:numPr></w:pPr>'
            f'<w:r><w:t>{t}</w:t></w:r></w:p>')


def cell(t, w, fill=""):
    return f'<w:tc><w:tcPr><w:tcW w:w="{w}" w:type="dxa"/>{fill}</w:tcPr>{p(t)}</w:tc>'


head = '<w:shd w:val="clear" w:fill="D9E2F3"/>'
tbl = ('<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid>'
       f'<w:tr>{cell("Nama", 3000, head)}{cell("Jabatan", 3000, head)}{cell("Gaji", 3000, head)}</w:tr>'
       f'<w:tr>{cell("Budi", 3000)}{cell("Manajer", 3000)}{cell("10.000.000", 3000)}</w:tr></w:tbl>')
img = ('<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:drawing><wp:inline><wp:extent cx="1905000" cy="635000"/>'
       '<a:graphic><a:graphicData><a:pic><a:blipFill><a:blip r:embed="rId9"/></a:blipFill></a:pic></a:graphicData></a:graphic>'
       '</wp:inline></w:drawing></w:r></w:p>')
body = (p("Laporan Bulanan", "Title") + p("1. Ringkasan", "Heading1")
        + '<w:p><w:pPr><w:jc w:val="both"/></w:pPr><w:r><w:t xml:space="preserve">Halo, ini dokumen contoh dengan teks </w:t></w:r>'
          '<w:r><w:rPr><w:b/></w:rPr><w:t>tebal</w:t></w:r><w:r><w:t xml:space="preserve">, </w:t></w:r>'
          '<w:r><w:rPr><w:i/></w:rPr><w:t>miring</w:t></w:r><w:r><w:t xml:space="preserve"> dan </w:t></w:r>'
          '<w:r><w:rPr><w:color w:val="C00000"/><w:u w:val="single"/></w:rPr><w:t>merah bergaris bawah</w:t></w:r>'
          '<w:r><w:t xml:space="preserve">. Paragraf ini dibuat panjang supaya terlihat bagaimana perataan rata kiri-kanan dan '
          'pembungkusan baris bekerja di halaman dengan margin yang sesuai.</w:t></w:r></w:p>'
        + li("Poin pertama", 1) + li("Poin kedua", 1) + li("Langkah satu", 2) + li("Langkah dua", 2)
        + p("2. Data", "Heading1") + tbl + p("") + img
        + '<w:p><w:r><w:t>Nama:</w:t></w:r><w:r><w:tab/><w:t>Budi Santoso</w:t></w:r></w:p>')
sect = '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>'

rels = ('<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId9" Type="x" Target="media/image1.png"/></Relationships>')
ct = ('<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
      '<Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>'
      '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')


def write_docx(name, inner):
    doc = f'<?xml version="1.0"?><w:document {W}><w:body>{inner}{sect}</w:body></w:document>'
    with zipfile.ZipFile(os.path.join(OUT, name), "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", ct)
        z.writestr("word/document.xml", doc)
        z.writestr("word/styles.xml", styles)
        z.writestr("word/numbering.xml", numbering)
        z.writestr("word/_rels/document.xml.rels", rels)
        z.writestr("word/media/image1.png", png(60, 20, (230, 120, 40)))


write_docx("rich.docx", body)
filler = "".join(
    f'<w:p><w:r><w:t>Paragraf nomor {i}. ' + "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " * 6 + "</w:t></w:r></w:p>"
    for i in range(150))
write_docx("long.docx", body + filler + '<w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:r><w:t>Setelah page break</w:t></w:r></w:p>')

NS = ('xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
      'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"')
sheet = f'''<?xml version="1.0"?><worksheet {NS}><dimension ref="A1:B2"/><sheetData>
<row r="1" spans="1:2"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>5</v></c></row>
<row r="2" spans="1:2"><c r="A2" s="3"><f>B1*2</f><v>10</v></c><c r="B2" t="inlineStr"><is><t>a &amp; b</t></is></c></row>
<row r="4"><c r="A4" t="b"><v>1</v></c></row></sheetData></worksheet>'''
with zipfile.ZipFile(os.path.join(OUT, "t.xlsx"), "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/xl/workbook.xml" ContentType="x"/><Override PartName="/xl/calcChain.xml" ContentType="y"/></Types>')
    z.writestr("xl/workbook.xml", f'<?xml version="1.0"?><workbook {NS}><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>')
    z.writestr("xl/_rels/workbook.xml.rels", '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="t" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="c" Target="calcChain.xml"/></Relationships>')
    z.writestr("xl/sharedStrings.xml", f'<?xml version="1.0"?><sst {NS}><si><t>Nama</t></si></sst>')
    z.writestr("xl/worksheets/sheet1.xml", sheet)
    z.writestr("xl/calcChain.xml", "<calcChain/>")
print("samples written to", OUT)

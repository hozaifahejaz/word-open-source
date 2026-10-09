#!/usr/bin/env python3
"""Create self-authored DOCX input fixtures in ignored build/smoke; stdlib only."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
from xml.sax.saxutils import escape

root = Path(__file__).resolve().parent.parent
out = root / 'build' / 'smoke'
out.mkdir(parents=True, exist_ok=True)
content_types = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
relationships = '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="main" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'

def package(name, xml):
    with ZipFile(out / name, 'w', ZIP_DEFLATED) as archive:
        for part, value in [('[Content_Types].xml', content_types), ('_rels/.rels', relationships), ('word/document.xml', xml)]:
            archive.writestr(part, value)

package('styled.docx', (root / 'crates/docx/tests/fixtures/styled.xml').read_text())
package('unsupported.docx', (root / 'crates/docx/tests/fixtures/unsupported.xml').read_text())
package('serif.docx', '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:rFonts w:ascii="Noto Serif" w:hAnsi="Noto Serif"/><w:b/><w:i/></w:rPr><w:t>Imported Noto Serif</w:t></w:r></w:p></w:body></w:document>')
paragraphs = ''.join('<w:p><w:r><w:t>' + escape(f'Paragraph {n:03}: Folio multi-page editing sample. Café 你好 é. ' * 3) + '</w:t></w:r></w:p>' for n in range(1, 81))
package('multipage.docx', '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>' + paragraphs + '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>')
(out / 'directory.docx').mkdir(exist_ok=True)
print(out)

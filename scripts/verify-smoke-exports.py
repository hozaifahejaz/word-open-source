#!/usr/bin/env python3
"""Inspect completed GUI smoke exports independently; does not drive the GUI."""
from pathlib import Path
from zipfile import ZipFile
import hashlib
import shutil
import subprocess
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent.parent
out = root / 'build' / 'smoke'
ns = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}

def document(name):
    with ZipFile(out / name) as archive:
        return ET.fromstring(archive.read('word/document.xml'))

def paragraphs(tree):
    return [''.join(p.itertext()) for p in tree.findall('w:body/w:p', ns)]

gui = document('gui-export.docx')
assert paragraphs(gui)[:2] == ['Folio bundle test', 'Café 你好 é 👩‍👩‍👧‍👦 اردو']
for paragraph in gui.findall('w:body/w:p', ns)[:2]:
    assert paragraph.find('w:pPr/w:jc', ns).get('{'+ns['w']+'}val') == 'center'
    assert paragraph.find('w:r/w:rPr/w:b', ns).get('{'+ns['w']+'}val') == 'true'
multi = document('multipage.docx')
assert paragraphs(multi)[-1] == 'Explicit new page'
assert 'Last page edit' in paragraphs(multi)
assert multi.find('.//w:br', ns).get('{'+ns['w']+'}type') == 'page'
assert all(r.find('w:rPr/w:i', ns).get('{'+ns['w']+'}val') == 'true' for r in multi.findall('.//w:r', ns) if r.find('w:t', ns) is not None)
assert paragraphs(document('unsupported-converted.docx')) == ['Visiblelinked textcachedinserted converted again']
serif = document('serif-export.docx')
assert paragraphs(serif) == ['Imported Noto Serif — Café 你好']
for run in serif.findall('.//w:r', ns):
    if run.find('w:t', ns) is not None:
        assert run.find('w:rPr/w:rFonts', ns).get('{'+ns['w']+'}ascii') == 'Noto Serif'
        assert run.find('w:rPr/w:b', ns).get('{'+ns['w']+'}val') == 'true'
        assert run.find('w:rPr/w:i', ns).get('{'+ns['w']+'}val') == 'true'
assert paragraphs(document('final-bundle.docx')) == ['Final bundle 0.1.0 — Café 你好']
source = out / 'unsupported.docx' 
source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
for name in ['unsupported-hardlink.docx', 'unsupported-symlink.docx']:
    assert hashlib.sha256((out / name).read_bytes()).hexdigest() == source_hash
print('Independent XML assertions: Unicode, bold/center, italic, explicit page break, converted-copy text and alias identity PASS')
print('Warned source SHA256:', source_hash)
if shutil.which('textutil'):
    for name, expected in [('gui-export.docx', 'Café 你好 é 👩‍👩‍👧‍👦 اردو'), ('multipage.docx', 'Explicit new page'), ('unsupported-converted.docx', 'Visiblelinked textcachedinserted converted again'), ('serif-export.docx', 'Imported Noto Serif — Café 你好'), ('final-bundle.docx', 'Final bundle 0.1.0 — Café 你好')]:
        text = subprocess.check_output(['textutil', '-convert', 'txt', '-stdout', str(out / name)]).decode()
        assert expected in text
        (out / (name + '.txt')).write_text(text)
        print('macOS textutil independent text read PASS:', name)
    print('textutil is not Microsoft Word/LibreOffice rendering validation.')
else:
    print('textutil unavailable; external text read NOT TESTED.')

#!/usr/bin/env python3
"""Regenerate the offline catalog from a reviewed SIX XML snapshot. No network I/O.

Usage: python3 scripts/update-currencies.py docs/data/iso-4217-list-one.xml
Existing scales are immutable here: changes require an explicit data migration
and a new JSON interchange version, as in migration 0005 / snapshot format 2.
"""
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
source = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "docs/data/iso-4217-list-one.xml"
xml = ET.parse(source).getroot()
old = json.loads((ROOT / "src/lib/currencies.json").read_text())
catalog = json.loads((ROOT / "src/lib/currency-catalog.json").read_text())
current = {}
scales = {}
for entry in xml.findall(".//CcyNtry"):
    code, scale = entry.findtext("Ccy"), entry.findtext("CcyMnrUnts", "")
    if code and scale.isdigit():
        current[code] = entry.findtext("CcyNm")
        scales[code] = int(scale)
changes = {code: (old[code], scale) for code, scale in scales.items() if code in old and old[code] != scale}
if changes:
    raise SystemExit(f"Decimal scale changes need a migration before catalog regeneration: {changes}")
catalog["published"] = xml.attrib["Pblshd"]
catalog["current"] = dict(sorted(current.items()))
catalog["legacy"] = {code: scale for code, scale in old.items() if code not in current}
for name, data in [("currencies.json", dict(sorted((old | scales).items()))), ("currency-catalog.json", catalog)]:
    (ROOT / "src/lib" / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
print(f"{len(current)} current currencies/fund units; {len(catalog['legacy'])} compatibility codes")

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BANNED_TERMS = re.compile(r"\b(bmad|zico|kiro|hermes|pi-bmad|pi\.dev|pewdiepie)\b", re.IGNORECASE)


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def fail(message: str) -> None:
    raise SystemExit(message)


def main() -> int:
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    plugin = read_json(ROOT / ".codex-plugin" / "plugin.json")
    listing_path = ROOT / "assets" / "marketplace" / "listing.json"
    listing = read_json(listing_path)

    if listing.get("name") != plugin.get("name"):
        fail(f"listing name does not match plugin name: {listing.get('name')}")
    if listing.get("version") != version:
        fail(f"listing version does not match VERSION: {listing.get('version')} != {version}")
    if listing.get("display_name") != plugin.get("interface", {}).get("displayName"):
        fail("listing display name does not match plugin displayName")

    referenced_assets = listing.get("assets", [])
    if not referenced_assets:
        fail("listing has no onboarding assets")
    for item in referenced_assets:
        asset_path = ROOT / item.get("path", "")
        if not asset_path.exists():
            fail(f"listing asset missing: {asset_path}")
        if asset_path.suffix == ".svg" and "<svg" not in asset_path.read_text(encoding="utf-8")[:200]:
            fail(f"listing svg asset is not an SVG: {asset_path}")

    onboarding_doc = ROOT / "docs" / "08-marketplace-onboarding.md"
    if not onboarding_doc.exists():
        fail("marketplace onboarding doc missing")
    doc_text = onboarding_doc.read_text(encoding="utf-8")
    if "../assets/onboarding/first-run-flow.svg" not in doc_text:
        fail("onboarding doc does not reference first-run flow asset")
    if "public directory submission remains external" not in doc_text:
        fail("onboarding doc must preserve publication boundary")

    scan_paths = [
        listing_path,
        onboarding_doc,
        ROOT / "assets" / "onboarding" / "first-run-flow.svg",
    ]
    for path in scan_paths:
        match = BANNED_TERMS.search(path.read_text(encoding="utf-8"))
        if match:
            fail(f"product-surface term not allowed in {path}: {match.group(0)}")

    print("Onboarding assets validation passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

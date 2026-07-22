#!/usr/bin/env python3
"""Source-only regressions for deterministic closed release-set metadata."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/build-release-set-manifest.py"
VERSION = "1.2.3"
TAG = f"v{VERSION}"
COMMIT = "a" * 40


def load_builder():
    spec = importlib.util.spec_from_file_location("forge_release_set_builder", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load_builder()


class ReleaseSetManifestTests(unittest.TestCase):
    def populate(self, root: Path) -> None:
        for index, name in enumerate(builder.CANONICAL_ARCHIVES, 1):
            (root / name).write_bytes(f"archive-{index}\n".encode())
            (root / f"{name}.sha256").write_text(f"checksum-{index}\n", encoding="utf-8")
            (root / f"{name}.sigstore").write_text(f"signature-{index}\n", encoding="utf-8")
        (root / f"forge-core-{VERSION}.cdx.json").write_text(
            '{"bomFormat":"CycloneDX"}\n', encoding="utf-8"
        )

    def build(self, root: Path) -> None:
        builder.build(
            root,
            version=VERSION,
            tag=TAG,
            source_commit=COMMIT,
            archives=builder.CANONICAL_ARCHIVES,
        )

    def verify(self, root: Path) -> None:
        builder.verify(
            root,
            version=VERSION,
            tag=TAG,
            source_commit=COMMIT,
            archives=builder.CANONICAL_ARCHIVES,
        )

    def test_deterministically_binds_exact_ordered_archives_and_sbom(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.populate(root)
            self.build(root)
            first = (root / builder.MANIFEST_NAME).read_bytes()
            first_checksum = (root / builder.CHECKSUM_NAME).read_bytes()
            self.verify(root)
            document = json.loads(first)
            self.assertEqual(document["version"], VERSION)
            self.assertEqual(document["tag"], TAG)
            self.assertEqual(document["source_commit"], COMMIT)
            self.assertEqual(
                [member["filename"] for member in document["archives"]],
                list(builder.CANONICAL_ARCHIVES),
            )
            self.assertEqual(document["sbom"]["filename"], f"forge-core-{VERSION}.cdx.json")
            self.assertRegex(document["sbom"]["sha256"], r"^[0-9a-f]{64}$")
            for member in document["archives"]:
                self.assertRegex(member["sha256"], r"^[0-9a-f]{64}$")
            (root / builder.MANIFEST_NAME).unlink()
            (root / builder.CHECKSUM_NAME).unlink()
            self.build(root)
            self.assertEqual((root / builder.MANIFEST_NAME).read_bytes(), first)
            self.assertEqual((root / builder.CHECKSUM_NAME).read_bytes(), first_checksum)

    def test_missing_extra_and_empty_members_fail_closed(self) -> None:
        mutations = {
            "missing": lambda root: (root / builder.CANONICAL_ARCHIVES[0]).unlink(),
            "extra": lambda root: (root / "forge-core-attacker.tar.gz").write_bytes(b"extra"),
            "empty": lambda root: (root / builder.CANONICAL_ARCHIVES[1]).write_bytes(b""),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.populate(root)
                mutate(root)
                with self.assertRaises(builder.ReleaseSetError):
                    self.build(root)

    def test_substituted_or_unordered_archive_authority_fails_closed(self) -> None:
        variants = {
            "missing": builder.CANONICAL_ARCHIVES[:-1],
            "extra": (*builder.CANONICAL_ARCHIVES, "forge-core-extra.tar.gz"),
            "substituted": (
                "forge-core-attacker-linux.tar.gz",
                *builder.CANONICAL_ARCHIVES[1:],
            ),
            "unordered": (
                builder.CANONICAL_ARCHIVES[1],
                builder.CANONICAL_ARCHIVES[0],
                *builder.CANONICAL_ARCHIVES[2:],
            ),
        }
        for name, archives in variants.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.populate(root)
                with self.assertRaises(builder.ReleaseSetError):
                    builder.build(
                        root,
                        version=VERSION,
                        tag=TAG,
                        source_commit=COMMIT,
                        archives=archives,
                    )

    def test_manifest_substitution_and_checksum_bypass_fail_closed(self) -> None:
        for name in ("filename", "digest", "checksum"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.populate(root)
                self.build(root)
                manifest = root / builder.MANIFEST_NAME
                checksum = root / builder.CHECKSUM_NAME
                if name == "checksum":
                    checksum.write_text("0" * 64 + f"  {builder.MANIFEST_NAME}\n", encoding="ascii")
                else:
                    document = json.loads(manifest.read_bytes())
                    if name == "filename":
                        document["archives"][0]["filename"] = "forge-core-substituted.tar.gz"
                    else:
                        document["archives"][0]["sha256"] = "A" * 64
                    mutated = builder.encode_document(document)
                    manifest.write_bytes(mutated)
                    checksum.write_text(
                        f"{builder._digest(mutated)}  {builder.MANIFEST_NAME}\n",
                        encoding="ascii",
                    )
                with self.assertRaises(builder.ReleaseSetError):
                    self.verify(root)

    def test_identity_and_exact_sbom_filename_are_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.populate(root)
            with self.assertRaises(builder.ReleaseSetError):
                builder.build(
                    root,
                    version=VERSION,
                    tag="v9.9.9",
                    source_commit=COMMIT,
                    archives=builder.CANONICAL_ARCHIVES,
                )
            (root / f"forge-core-{VERSION}.cdx.json").rename(root / "substitute.cdx.json")
            with self.assertRaises(builder.ReleaseSetError):
                self.build(root)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Static and real-Cargo regressions for release lock enforcement."""

from __future__ import annotations

import hashlib
import json
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release.yml"
FIXTURE = ROOT / "contracts/fixtures/release-lock/manifest-drift"


def load_checker():
    path = ROOT / "scripts/check-release-locking.py"
    spec = importlib.util.spec_from_file_location("forge_release_lock_checker", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


def load_runner():
    path = ROOT / "scripts/run-release-locked-sbom.py"
    spec = importlib.util.spec_from_file_location("forge_release_sbom_runner", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner_module = load_runner()


def commit_repository(repository: Path) -> None:
    subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
    subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
    subprocess.run(
        ["git", "-c", "user.name=Release Test", "-c", "user.email=release@example.invalid",
         "commit", "-qm", "fixture"],
        cwd=repository, check=True,
    )


def release_cargo() -> str | None:
    """Prefer the real toolchain binary so parallel rustup proxies cannot serialize probes."""
    rustup_home = Path(os.environ.get("RUSTUP_HOME", Path.home() / ".rustup"))
    candidates = sorted((rustup_home / "toolchains").glob("*/bin/cargo"))
    stable = [path for path in candidates if path.parents[1].name.startswith("stable-")]
    selected = (stable or candidates)[:1]
    return str(selected[0]) if selected else shutil.which("cargo")


class ReleaseLockingTests(unittest.TestCase):
    def assert_mutation_rejected(self, mutated: str) -> None:
        """Bypass only the byte hash: the independent graph must still reject."""
        original = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotEqual(mutated, original)
        with self.assertRaises(checker.ReleaseLockError):
            checker.check_source(
                mutated,
                repo_root=ROOT,
                expected_workflow_sha256=hashlib.sha256(mutated.encode()).hexdigest(),
            )

    def assert_semantic_rejected(self, mutated: str) -> None:
        """Authorize candidate hashes so semantic/parser checks must reject."""
        original = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotEqual(mutated, original)
        with self.assertRaises(checker.ReleaseLockError):
            graph_hash = checker.graph_digest(mutated)
            checker.check_source(
                mutated,
                repo_root=ROOT,
                expected_workflow_sha256=hashlib.sha256(mutated.encode()).hexdigest(),
                expected_graph_sha256=graph_hash,
            )

    def replace_once(self, old: str, new: str) -> str:
        source = WORKFLOW.read_text(encoding="utf-8")
        self.assertEqual(source.count(old), 1, old)
        return source.replace(old, new, 1)

    def copy_governed(self, root: Path) -> None:
        for relative in checker.GOVERNED_FILE_SHA256:
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)

    def canonical_governed_fixture(self, relative: str) -> bytes:
        materialized = (ROOT / relative).read_bytes()
        return checker._canonical_governed_bytes(relative, materialized)

    def check_with_governed_variant(self, relative: str, content: bytes):
        source = WORKFLOW.read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_governed(root)
            (root / relative).write_bytes(content)
            return checker.check_source(source, repo_root=root)

    def test_real_release_topology_is_exact_and_locked(self) -> None:
        invocations = checker.check(WORKFLOW)
        self.assertEqual(
            [(item.tool, item.subcommand) for item in invocations],
            [
                ("cargo", "install"),
                ("cross", "build"),
                ("cargo", "build"),
                ("cargo", "install"),
                ("cargo", "metadata"),
            ],
        )

    def test_removing_any_release_locked_flag_is_rejected(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        offsets: list[int] = []
        start = 0
        while (offset := source.find("--locked", start)) >= 0:
            offsets.append(offset)
            start = offset + 1
        self.assertEqual(len(offsets), 4)
        for number, offset in enumerate(offsets, 1):
            with self.subTest(invocation=number):
                self.assert_semantic_rejected(
                    source[:offset] + source[offset + len("--locked") :]
                )

    def test_reviewer_workflow_bypasses_are_rejected(self) -> None:
        native = "run: cargo build --locked --release --target ${{ matrix.target }} -p forge-core-cli"
        mutations = {
            "path-qualified": self.replace_once(native, native.replace("cargo", "/usr/bin/cargo", 1)),
            "toolchain": self.replace_once(native, native.replace("cargo build", "cargo +1.97.0 build", 1)),
            "global-config": self.replace_once(native, native.replace("cargo build", "cargo --config net.retry=2 build", 1)),
            "quoted-cargo-env": self.replace_once(native, "run: |\n          \"$CARGO\" build --locked --release"),
            "shell-variable": self.replace_once(native, "run: |\n          tool=cargo\n          \"$tool\" build --locked --release"),
            "alias": self.replace_once(native, "run: |\n          alias builder='cargo'\n          builder build --locked --release"),
            "function": self.replace_once(native, "run: |\n          builder() { cargo \"$@\"; }\n          builder build --locked --release"),
            "env": self.replace_once(native, native.replace("cargo", "env cargo", 1)),
            "usr-bin-env": self.replace_once(native, native.replace("cargo", "/usr/bin/env cargo", 1)),
            "eval": self.replace_once(native, "run: eval 'cargo build --locked --release'"),
            "sh-c": self.replace_once(native, "run: sh -c 'cargo build --locked --release'"),
            "bash-c": self.replace_once(native, "run: bash -c 'cargo build --locked --release'"),
            "exec": self.replace_once(native, native.replace("cargo", "exec cargo", 1)),
            "time": self.replace_once(native, native.replace("cargo", "time cargo", 1)),
            "command-option": self.replace_once(native, native.replace("cargo", "command -- cargo", 1)),
            "command-p": self.replace_once(native, native.replace("cargo", "command -p cargo", 1)),
            "source": self.replace_once(native, "run: source scripts/package.sh"),
            "dot": self.replace_once(native, "run: . scripts/package.sh"),
            "payload-locked": self.replace_once(native, "run: cargo run --release -- --locked"),
            "called-python": self.replace_once(native, "run: python scripts/package.py"),
            "called-shell": self.replace_once(native, "run: bash scripts/package.sh"),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_semantic_rejected(mutated)

    def test_full_manifest_rejects_reviewer_semantic_repros_with_candidate_hashes(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        native_step = (
            "      - name: Test deterministic archive tooling\n"
            "        run: python scripts/test-release-archive.py"
        )
        remote_job = (
            "\n  remote-escape:\n"
            "    name: Remote escape\n"
            "    needs: metadata\n"
            "    uses: example/release/.github/workflows/unlocked.yml@" + "0" * 40 + "\n"
        )
        mutations = {
            "attacker-checkout-ref": source.replace(
                "ref: ${{ needs.metadata.outputs.commit_sha }}",
                "ref: refs/heads/attacker",
                1,
            ),
            "build-if-always": source.replace(
                "  build:\n    name: Build ${{ matrix.target }}",
                "  build:\n    name: Build ${{ matrix.target }}\n    if: always()",
                1,
            ),
            "self-hosted-matrix-runner": source.replace(
                "runner: ubuntu-latest\n            expected-arch: x86_64",
                "runner: self-hosted\n            expected-arch: x86_64",
                1,
            ),
            "rustup-nightly": source.replace(
                "env:\n  CARGO_INCREMENTAL",
                "env:\n  RUSTUP_TOOLCHAIN: nightly\n  CARGO_INCREMENTAL",
                1,
            ),
            "local-rustc-wrapper": source.replace(
                "env:\n  CARGO_INCREMENTAL",
                "env:\n  RUSTC_WRAPPER: ./scripts/unlocked\n  CARGO_INCREMENTAL",
                1,
            ),
            "extensionless-python": source.replace(
                native_step,
                native_step + "\n\n      - name: Unlocked extensionless tool\n"
                "        run: python scripts/unlocked",
                1,
            ),
            "github-path-mutation": source.replace(
                native_step,
                native_step + "\n\n      - name: Mutate executable search\n"
                "        run: echo \\\"$PWD/tool-bin\\\" >> \\\"$GITHUB_PATH\\\"",
                1,
            ),
            "path-env-mutation": source.replace(
                "env:\n  CARGO_INCREMENTAL",
                "env:\n  PATH: ${{ github.workspace }}/tool-bin:${{ env.PATH }}\n"
                "  CARGO_INCREMENTAL",
                1,
            ),
            "pinned-remote-reusable-job": source + remote_job,
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_semantic_rejected(mutated)

    def test_arm64_smoke_removal_disable_and_bypass_are_rejected(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        job_start = source.index("  arm64-linux-smoke:\n")
        job_end = source.index("  release:\n", job_start)
        mutations = {
            "can-smoke-false": source.replace(
                "            can_smoke: true # executed after packaging on the bounded native ARM64 smoke runner",
                "            can_smoke: false",
                1,
            ),
            "job-removed": source[:job_start] + source[job_end:],
            "conditional-bypass": source.replace(
                "  arm64-linux-smoke:\n    name: Smoke packaged Linux ARM64 install on native ARM64",
                "  arm64-linux-smoke:\n    name: Smoke packaged Linux ARM64 install on native ARM64\n    if: false",
                1,
            ),
            "job-continue-on-error": source.replace(
                "  arm64-linux-smoke:\n    name: Smoke packaged Linux ARM64 install on native ARM64",
                "  arm64-linux-smoke:\n    name: Smoke packaged Linux ARM64 install on native ARM64\n    continue-on-error: true",
                1,
            ),
            "step-if-false": source.replace(
                "      - name: Smoke extracted Linux ARM64 release install\n        shell: bash",
                "      - name: Smoke extracted Linux ARM64 release install\n        if: false\n        shell: bash",
                1,
            ),
            "step-continue-on-error": source.replace(
                "      - name: Smoke extracted Linux ARM64 release install\n        shell: bash",
                "      - name: Smoke extracted Linux ARM64 release install\n        continue-on-error: true\n        shell: bash",
                1,
            ),
            "publication-bypass": source.replace(
                "    needs: [metadata, build, arm64-linux-smoke]",
                "    needs: [metadata, build]",
                1,
            ),
            "wrong-runner": source.replace("    runs-on: ubuntu-24.04-arm", "    runs-on: ubuntu-latest", 1),
            "direct-binary-bypass": source.replace(
                "          python scripts/smoke-release-install.py \\\n            --archive arm64-release-assets/forge-core-aarch64-linux.tar.gz \\",
                "          arm64-release-assets/forge-core-aarch64-linux.tar.gz --version \\",
                1,
            ),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_semantic_rejected(mutated)
                with self.assertRaises(checker.ReleaseLockError):
                    checker._check_arm64_smoke(mutated, checker.parse_graph(mutated))

    def test_release_set_removal_substitution_and_order_bypass_are_rejected(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        mutations = {
            "build-removed": source.replace(
                "          python scripts/build-release-set-manifest.py build \\",
                "          true \\",
                1,
            ),
            "verify-removed": source.replace(
                "          python scripts/build-release-set-manifest.py verify \\",
                "          true \\",
                1,
            ),
            "substituted": source.replace(
                "            --archive forge-core-aarch64-linux.tar.gz \\",
                "            --archive forge-core-attacker-linux.tar.gz \\",
                1,
            ),
            "unordered": source.replace(
                "            --archive forge-core-x86_64-linux.tar.gz \\\n            --archive forge-core-aarch64-linux.tar.gz \\",
                "            --archive forge-core-aarch64-linux.tar.gz \\\n            --archive forge-core-x86_64-linux.tar.gz \\",
                1,
            ),
            "release-set-continue-on-error": source.replace(
                "      - name: Build and verify deterministic release-set manifest\n        shell: bash",
                "      - name: Build and verify deterministic release-set manifest\n        continue-on-error: true\n        shell: bash",
                1,
            ),
            "final-set-continue-on-error": source.replace(
                "      - name: Require exact final release asset set\n        shell: bash",
                "      - name: Require exact final release asset set\n        continue-on-error: true\n        shell: bash",
                1,
            ),
            "manifest-unpublished": source.replace(
                "            release-assets/forge-core-release-set.json\n",
                "",
                1,
            ),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_semantic_rejected(mutated)
                if name != "manifest-unpublished":
                    jobs = checker.parse_graph(mutated)
                    by_identity = {
                        (step.job, step.name): step
                        for job in jobs
                        for step in job.steps
                    }
                    with self.assertRaises(checker.ReleaseLockError):
                        checker._check_release_set_step(by_identity)

    def test_duplicate_yaml_keys_are_rejected_at_every_mapping_depth(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        mutations = {
            "step-with": source.replace(
                "          ref: ${{ needs.metadata.outputs.commit_sha }}",
                "          ref: ${{ needs.metadata.outputs.commit_sha }}\n"
                "          ref: refs/heads/attacker",
                1,
            ),
            "global-env": source.replace(
                '  CARGO_INCREMENTAL: "0"',
                '  CARGO_INCREMENTAL: "0"\n  CARGO_INCREMENTAL: "1"',
                1,
            ),
            "matrix-entry": source.replace(
                "          - target: x86_64-unknown-linux-gnu",
                "          - target: x86_64-unknown-linux-gnu\n"
                "            target: attacker",
                1,
            ),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_semantic_rejected(mutated)

    def test_exact_graph_rejects_changes_when_only_workflow_hash_is_updated(self) -> None:
        native = "run: cargo build --locked --release --target ${{ matrix.target }} -p forge-core-cli"
        mutations = {
            "changed-body": self.replace_once(native, native + " --verbose"),
            "new-step": self.replace_once(
                "      - name: Test deterministic archive tooling\n        run: python scripts/test-release-archive.py",
                "      - name: Test deterministic archive tooling\n        run: python scripts/test-release-archive.py\n\n      - name: Unexpected\n        run: echo unexpected",
            ),
            "new-job": WORKFLOW.read_text(encoding="utf-8") + "\n  unexpected:\n    name: Unexpected\n    needs: metadata\n    runs-on: ubuntu-latest\n    steps:\n      - name: Unexpected\n        run: echo unexpected\n",
            "dependency-edge": self.replace_once("    needs: metadata", "    needs: [metadata, release]"),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_mutation_rejected(mutated)

    def test_local_job_level_reusable_workflow_is_rejected_semantically(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8") + (
            "\n  escape:\n    name: Escape\n    needs: metadata\n"
            "    uses: ./.github/workflows/unlocked.yml\n"
        )
        self.assert_semantic_rejected(source)

    def test_dispatch_ref_and_tag_commit_mismatch_fails_identity_gate(self) -> None:
        jobs = checker.parse_graph(WORKFLOW.read_text(encoding="utf-8"))
        metadata = next(job for job in jobs if job.key == "metadata")
        gate = next(step for step in metadata.steps if step.name.startswith("Bind executing"))
        environment = os.environ.copy()
        environment["EXECUTING_WORKFLOW_SHA"] = "b" * 40
        simulated = gate.run.replace(
            'checked_out_sha="$(git rev-parse HEAD)"',
            f'checked_out_sha="{"a" * 40}"',
            1,
        )
        rejected = subprocess.run(
            ["bash", "-c", simulated], cwd=ROOT, env=environment,
            text=True, capture_output=True, check=False, timeout=10,
        )
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("does not match checked-out release commit", rejected.stderr)
        mutation = WORKFLOW.read_text(encoding="utf-8").replace(
            "EXECUTING_WORKFLOW_SHA: ${{ github.workflow_sha }}",
            "EXECUTING_WORKFLOW_SHA: ${{ github.sha }}",
            1,
        )
        self.assert_semantic_rejected(mutation)

    def test_echoed_wrapper_plus_direct_plugin_is_rejected(self) -> None:
        command = """          python scripts/run-release-locked-sbom.py \\
            --lockfile Cargo.lock \\
            -- \\
            --format json \\
            --manifest-path crates/forge-core-cli/Cargo.toml \\
            --override-filename \"forge-core-$VERSION.cdx\""""
        bypass = """          echo \"python scripts/run-release-locked-sbom.py\"
          cargo-cyclonedx cyclonedx --format json --manifest-path crates/forge-core-cli/Cargo.toml"""
        self.assert_semantic_rejected(self.replace_once(command, bypass))

    def test_yaml_alias_run_is_rejected_as_yaml_not_as_text(self) -> None:
        source = self.replace_once(
            "run: cargo build --locked --release --target ${{ matrix.target }} -p forge-core-cli",
            "run: *native_build",
        )
        source = source.replace("jobs:\n", "jobs:\n  native_template: &native_build cargo build --locked\n", 1)
        with self.assertRaisesRegex(checker.ReleaseLockError, "unsupported YAML|anchors, aliases"):
            checker.check_source(
                source,
                repo_root=ROOT,
                expected_workflow_sha256=hashlib.sha256(source.encode()).hexdigest(),
            )

    def test_inline_literal_and_folded_run_scalars_are_semantic(self) -> None:
        source = """jobs:
  audit:
    steps:
      - name: Inline
        run: 'cargo build --locked'
      - name: Literal
        run: |
          cargo build \\
            --locked --release
      - name: Folded
        run: >
          cross build
          --locked --release
"""
        steps = checker.parse_workflow(source)
        self.assertEqual([step.name for step in steps], ["Inline", "Literal", "Folded"])
        parsed = [checker.find_invocations(step.run, step.line)[0] for step in steps]
        self.assertEqual(
            [(item.tool, item.subcommand) for item in parsed],
            [("cargo", "build"), ("cargo", "build"), ("cross", "build")],
        )

    def test_multiline_and_chained_commands_are_independently_checked(self) -> None:
        body = """cargo build \\
  --locked --release
cross build --locked && cargo test --workspace
"""
        with self.assertRaisesRegex(checker.ReleaseLockError, "cargo test"):
            checker.find_invocations(body)
        positive = """cargo build \\
  --locked --release
cross build \\
  --locked --release
"""
        invocations = checker.find_invocations(positive)
        self.assertEqual(len(invocations), 2)

    def test_cargo_parser_rejects_all_executable_spellings_and_payload_lock(self) -> None:
        negatives = {
            "path": "/usr/bin/cargo package --locked",
            "toolchain-unlocked": "cargo +1.97.0 package",
            "global-option-unlocked": "cargo --config net.retry=2 package",
            "quoted-env": '"$CARGO" package --locked',
            "assignment": "tool=cargo\n\"$tool\" package --locked",
            "alias": "alias c=cargo; c package --locked",
            "function": "function c { cargo package --locked; }; c",
            "env": "env cargo package --locked",
            "usr-bin-env": "/usr/bin/env cargo package --locked",
            "eval": "eval 'cargo package --locked'",
            "sh-c": "sh -c 'cargo package --locked'",
            "bash-c": "bash -c 'cargo package --locked'",
            "exec": "exec cargo package --locked",
            "time": "time cargo package --locked",
            "command-option": "command -- cargo package --locked",
            "command-p": "command -p cargo package --locked",
            "source": "source scripts/package.sh",
            "dot": ". scripts/package.sh",
            "plugin": "cargo-cyclonedx cyclonedx --locked",
            "cargo-plugin": "cargo cyclonedx --locked",
            "payload": "cargo run --release -- --locked",
        }
        for name, body in negatives.items():
            with self.subTest(name=name), self.assertRaises(checker.ReleaseLockError):
                checker.find_invocations(body)
        accepted = checker.find_invocations(
            "cargo +1.97.0 --config net.retry=2 package --locked --allow-dirty"
        )
        self.assertEqual([(item.tool, item.subcommand) for item in accepted], [("cargo", "package")])

    def test_lf_and_crlf_forge_cmd_share_governed_commitment(self) -> None:
        relative = "distribution/forge.cmd"
        materialized = (ROOT / relative).read_bytes()
        canonical = checker._canonical_governed_bytes(relative, materialized)
        crlf = canonical.replace(b"\n", b"\r\n")
        self.assertNotIn(b"\r", canonical)
        expected = checker.GOVERNED_FILE_SHA256[relative]
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), expected)
        self.assertEqual(
            hashlib.sha256(checker._canonical_governed_bytes(relative, crlf)).hexdigest(),
            expected,
        )
        lf_invocations = self.check_with_governed_variant(relative, canonical)
        crlf_invocations = self.check_with_governed_variant(relative, crlf)
        self.assertEqual(lf_invocations, crlf_invocations)

    def test_all_approved_git_text_wrappers_accept_crlf(self) -> None:
        self.assertEqual(
            checker.GIT_TEXT_GOVERNED_FILES,
            {"distribution/forge", "distribution/forge.cmd"},
        )
        for relative in checker.GIT_TEXT_GOVERNED_FILES:
            canonical = self.canonical_governed_fixture(relative)
            with self.subTest(relative=relative):
                self.assertNotIn(b"\r", canonical)
                invocations = self.check_with_governed_variant(
                    relative, canonical.replace(b"\n", b"\r\n")
                )
                self.assertEqual(len(invocations), 5)

    def test_malicious_forge_cmd_is_rejected_under_lf_and_crlf(self) -> None:
        relative = "distribution/forge.cmd"
        canonical = self.canonical_governed_fixture(relative)
        malicious = canonical + b"start calc.exe\n"
        for name, content in {
            "lf": malicious,
            "crlf": malicious.replace(b"\n", b"\r\n"),
        }.items():
            with self.subTest(eol=name), self.assertRaisesRegex(
                checker.ReleaseLockError, "forge.cmd"
            ):
                self.check_with_governed_variant(relative, content)

    def test_mixed_and_lone_cr_forge_cmd_are_rejected_as_malformed(self) -> None:
        relative = "distribution/forge.cmd"
        canonical = self.canonical_governed_fixture(relative)
        malformed = {
            "mixed": canonical.replace(b"\n", b"\r\n", 1),
            "lone-cr": canonical.replace(b"\n", b"\r", 1),
        }
        for name, content in malformed.items():
            with self.subTest(representation=name), self.assertRaisesRegex(
                checker.ReleaseLockError, "mixed or lone CR"
            ):
                self.check_with_governed_variant(relative, content)

    def test_forge_cmd_byte_substitution_is_rejected(self) -> None:
        relative = "distribution/forge.cmd"
        canonical = self.canonical_governed_fixture(relative)
        substituted = canonical.replace(b"Forge wrapper", b"Xorge wrapper", 1)
        self.assertEqual(len(substituted), len(canonical))
        with self.assertRaisesRegex(checker.ReleaseLockError, "forge.cmd"):
            self.check_with_governed_variant(relative, substituted)

    def test_security_sensitive_governed_script_keeps_exact_eol_hash(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_governed(root)
            script = root / "scripts/run-release-locked-sbom.py"
            script.write_bytes(script.read_bytes().replace(b"\n", b"\r\n"))
            with self.assertRaisesRegex(
                checker.ReleaseLockError, "run-release-locked-sbom.py"
            ):
                checker.check_source(source, repo_root=root)


    def test_manifest_parent_swap_restore_rejects_authorized_semantics(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        mutations = [
            source.replace(
                "env:\n  CARGO_INCREMENTAL",
                "env:\n  PATH: ${{ github.workspace }}/tool-bin:${{ env.PATH }}\n  CARGO_INCREMENTAL",
                1,
            ),
            source.replace(
                "  build:\n    name: Build ${{ matrix.target }}",
                "  build:\n    name: Build ${{ matrix.target }}\n    if: always()",
                1,
            ),
        ]
        for mutated in mutations:
            with self.subTest(mutation=hashlib.sha256(mutated.encode()).hexdigest()):
                with tempfile.TemporaryDirectory() as directory:
                    base = Path(directory)
                    root = base / "repository"
                    self.copy_governed(root)
                    parent = root / "contracts/fixtures/release-lock"
                    retained = parent.with_name("release-lock.retained")
                    outside = base / "outside-release-lock"
                    outside.mkdir()
                    (outside / "workflow-semantic-manifest.json").write_text(
                        json.dumps(checker.workflow_semantic_manifest(mutated)),
                        encoding="utf-8",
                    )
                    parent.rename(retained)
                    parent.symlink_to(outside, target_is_directory=True)
                    try:
                        with self.assertRaisesRegex(
                            checker.ReleaseLockError, "cannot read release semantic manifest"
                        ):
                            checker.check_source(
                                mutated, repo_root=root,
                                expected_workflow_sha256=hashlib.sha256(mutated.encode()).hexdigest(),
                                expected_graph_sha256=checker.graph_digest(mutated),
                            )
                    finally:
                        parent.unlink()
                        retained.rename(parent)
                    self.assertTrue(parent.is_dir())
                    self.assertFalse(parent.is_symlink())

    def test_governed_parent_symlink_and_hardlink_are_rejected(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            self.copy_governed(root)
            scripts = root / "scripts"
            retained = root / "scripts.retained"
            scripts.rename(retained)
            scripts.symlink_to(retained, target_is_directory=True)
            try:
                with self.assertRaises((checker.ReleaseLockError, OSError)):
                    checker.check_source(source, repo_root=root)
            finally:
                scripts.unlink()
                retained.rename(scripts)
            linked = root / "forge-hardlink"
            os.link(root / "distribution/forge.cmd", linked)
            with self.assertRaisesRegex(checker.ReleaseLockError, "safe regular file"):
                checker.check_source(source, repo_root=root)

    def test_canonical_workflow_parent_symlink_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "repository"
            outside = base / "outside-workflows"
            outside.mkdir()
            shutil.copy2(WORKFLOW, outside / "release.yml")
            (root / ".github").mkdir(parents=True)
            (root / ".github/workflows").symlink_to(outside, target_is_directory=True)
            with self.assertRaises(checker.ReleaseLockError):
                checker.check(root / ".github/workflows/release.yml", repo_root=root)


    def test_canonical_workflow_symlink_is_rejected_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflow = root / ".github/workflows/release.yml"
            workflow.parent.mkdir(parents=True)
            workflow.symlink_to(WORKFLOW)
            with self.assertRaisesRegex(checker.ReleaseLockError, "cannot read|unsafe"):
                checker.check(workflow, repo_root=root)

    def test_sbom_lockfile_symlink_and_outside_repository_are_rejected(self) -> None:
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            link = Path(directory) / "Cargo.lock"
            link.symlink_to(ROOT / "Cargo.lock")
            linked = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(link), "--", "--format", "json"],
                cwd=ROOT, text=True, capture_output=True, check=False, timeout=10,
            )
            self.assertNotEqual(linked.returncode, 0)
            self.assertIn("symbolic links", linked.stderr)
        with tempfile.TemporaryDirectory() as directory:
            outside = Path(directory) / "Cargo.lock"
            outside.write_bytes((ROOT / "Cargo.lock").read_bytes())
            rejected = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(outside), "--", "--format", "json"],
                cwd=ROOT, text=True, capture_output=True, check=False, timeout=10,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("inside checked repository", rejected.stderr)

    def test_sbom_lockfile_replacement_race_fails_without_publishing_output(self) -> None:
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir(parents=True)
            shutil.copy2(production_runner, runner)
            fixture = repository / "fixture"
            shutil.copytree(FIXTURE, fixture)
            commit_repository(repository)
            lockfile = fixture / "Cargo.lock"
            outside = root / "outside.lock"
            outside.write_bytes(lockfile.read_bytes())
            fake_bin = root / "bin"
            fake_bin.mkdir()
            fake_cargo = fake_bin / "cargo"
            fake_cargo.write_text(
                f"#!{sys.executable}\nraise SystemExit(0)\n",
                encoding="utf-8",
            )
            fake_cargo.chmod(0o755)
            fake_plugin = fake_bin / "cargo-cyclonedx"
            fake_plugin.write_text(
                f"#!{sys.executable}\n"
                "import os,pathlib,sys\n"
                "lock=pathlib.Path(os.environ['ATTACK_LOCK'])\n"
                "lock.unlink()\n"
                "lock.symlink_to(pathlib.Path(os.environ['OUTSIDE_LOCK']))\n"
                "args=sys.argv[1:]\n"
                "manifest=pathlib.Path(args[args.index('--manifest-path')+1])\n"
                "name=args[args.index('--override-filename')+1] + '.json'\n"
                "(manifest.parent/name).write_text('attacker output')\n"
                "raise SystemExit(0)\n",
                encoding="utf-8",
            )
            fake_plugin.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = str(fake_bin) + os.pathsep + environment.get("PATH", "")
            environment["ATTACK_LOCK"] = str(lockfile)
            environment["OUTSIDE_LOCK"] = str(outside)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(runner),
                    "--lockfile",
                    str(lockfile),
                    "--",
                    "--format",
                    "json",
                    "--manifest-path",
                    str(fixture / "Cargo.toml"),
                    "--override-filename",
                    "race-proof.cdx",
                ],
                cwd=fixture,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertRegex(
                completed.stderr, "path was replaced|symbolic links|safe unlinked regular file"
            )
            self.assertEqual(list(fixture.rglob("*.cdx.json")), [])

    def test_immutable_head_symlink_lock_is_rejected_without_outside_write(self) -> None:
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / "repository"
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir(parents=True)
            shutil.copy2(production_runner, runner)
            fixture = repository / "fixture"
            shutil.copytree(FIXTURE, fixture)
            lock = fixture / "Cargo.lock"
            lock_bytes = lock.read_bytes()
            outside = base / "outside.lock"
            outside.write_bytes(b"OUTSIDE-UNCHANGED\n")
            lock.unlink()
            lock.symlink_to(outside)
            commit_repository(repository)
            lock.unlink()
            lock.write_bytes(lock_bytes)
            fake_bin = base / "bin"
            fake_bin.mkdir()
            for name in ("cargo", "cargo-cyclonedx"):
                tool = fake_bin / name
                tool.write_text(f"#!{sys.executable}\nraise SystemExit(0)\n", encoding="utf-8")
                tool.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = str(fake_bin) + os.pathsep + environment.get("PATH", "")
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(lock), "--", "--format", "json",
                 "--manifest-path", str(fixture / "Cargo.toml"), "--override-filename", "symlink.cdx"],
                cwd=fixture, env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("non-symlink blob", completed.stderr)
            self.assertEqual(outside.read_bytes(), b"OUTSIDE-UNCHANGED\n")
            self.assertEqual(list(repository.rglob("*.cdx.json")), [])

    def test_initial_hardlinked_cargo_lock_is_rejected(self) -> None:
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / "repository"
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir(parents=True)
            shutil.copy2(production_runner, runner)
            fixture = repository / "fixture"
            shutil.copytree(FIXTURE, fixture)
            commit_repository(repository)
            os.link(fixture / "Cargo.lock", base / "attacker-hardlink.lock")
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(fixture / "Cargo.lock"), "--",
                 "--format", "json", "--manifest-path", str(fixture / "Cargo.toml"),
                 "--override-filename", "hardlink.cdx"],
                cwd=fixture, text=True, capture_output=True, check=False, timeout=10,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("safe unlinked regular file", completed.stderr)
            self.assertEqual(list(repository.rglob("*.cdx.json")), [])

    def test_source_parent_swap_restore_cannot_substitute_staged_lock(self) -> None:
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / "repository"
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir(parents=True)
            shutil.copy2(production_runner, runner)
            fixture = repository / "fixture"
            shutil.copytree(FIXTURE, fixture)
            committed_lock = (fixture / "Cargo.lock").read_bytes()
            commit_repository(repository)
            fake_bin = base / "bin"
            fake_bin.mkdir()
            cargo = fake_bin / "cargo"
            cargo.write_text(f"#!{sys.executable}\nraise SystemExit(0)\n", encoding="utf-8")
            cargo.chmod(0o755)
            plugin = fake_bin / "cargo-cyclonedx"
            plugin.write_text(
                f"#!{sys.executable}\n"
                "import os,pathlib,shutil,sys\n"
                "source=pathlib.Path(os.environ['SOURCE_PARENT'])\n"
                "retained=source.with_name(source.name+'.retained')\n"
                "source.rename(retained)\n"
                "source.mkdir()\n"
                "try:\n"
                " (source/'Cargo.lock').write_bytes(b'ATTACKER-LOCK\\n')\n"
                " args=sys.argv[1:]\n"
                " manifest=pathlib.Path(args[args.index('--manifest-path')+1])\n"
                " name=args[args.index('--override-filename')+1]+'.json'\n"
                " (manifest.parent/name).write_bytes((manifest.parent/'Cargo.lock').read_bytes())\n"
                "finally:\n"
                " shutil.rmtree(source)\n"
                " retained.rename(source)\n",
                encoding="utf-8",
            )
            plugin.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = str(fake_bin) + os.pathsep + environment.get("PATH", "")
            environment["SOURCE_PARENT"] = str(fixture)
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(fixture / "Cargo.lock"), "--",
                 "--format", "json", "--manifest-path", str(fixture / "Cargo.toml"),
                 "--override-filename", "bound.cdx"],
                cwd=fixture, env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual((fixture / "bound.cdx.json").read_bytes(), committed_lock)
            self.assertEqual((fixture / "Cargo.lock").read_bytes(), committed_lock)

    def test_output_parent_symlink_race_writes_neither_outside_nor_output(self) -> None:
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            repository = base / "repository"
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir(parents=True)
            shutil.copy2(production_runner, runner)
            fixture = repository / "fixture"
            shutil.copytree(FIXTURE, fixture)
            shutil.copy2(fixture / "Cargo.lock", repository / "Cargo.lock")
            commit_repository(repository)
            outside = base / "outside"
            outside.mkdir()
            retained = repository / "fixture.retained"
            fake_bin = base / "bin"
            fake_bin.mkdir()
            cargo = fake_bin / "cargo"
            cargo.write_text(f"#!{sys.executable}\nraise SystemExit(0)\n", encoding="utf-8")
            cargo.chmod(0o755)
            plugin = fake_bin / "cargo-cyclonedx"
            plugin.write_text(
                f"#!{sys.executable}\n"
                "import os,pathlib,sys\n"
                "source=pathlib.Path(os.environ['SOURCE_PARENT'])\n"
                "source.rename(pathlib.Path(os.environ['RETAINED_PARENT']))\n"
                "source.symlink_to(pathlib.Path(os.environ['OUTSIDE_PARENT']), target_is_directory=True)\n"
                "args=sys.argv[1:]\n"
                "manifest=pathlib.Path(args[args.index('--manifest-path')+1])\n"
                "name=args[args.index('--override-filename')+1]+'.json'\n"
                "(manifest.parent/name).write_bytes(b'STAGED')\n",
                encoding="utf-8",
            )
            plugin.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = str(fake_bin) + os.pathsep + environment.get("PATH", "")
            environment["SOURCE_PARENT"] = str(fixture)
            environment["RETAINED_PARENT"] = str(retained)
            environment["OUTSIDE_PARENT"] = str(outside)
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(repository / "Cargo.lock"), "--",
                 "--format", "json", "--manifest-path", str(fixture / "Cargo.toml"),
                 "--override-filename", "escaped.cdx"],
                cwd=fixture, env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("destination parent", completed.stderr)
            self.assertEqual(list(outside.iterdir()), [])
            self.assertEqual(list(retained.rglob("*.cdx.json")), [])
            fixture.unlink()
            retained.rename(fixture)

    def test_atomic_replace_failure_cleans_temporary_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            fd = os.open(destination, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                with mock.patch.object(runner_module.os, "replace", side_effect=OSError("injected")):
                    with self.assertRaises(OSError):
                        runner_module._publish_output(fd, "result.cdx.json", b"SBOM", lambda: None)
            finally:
                os.close(fd)
            self.assertEqual(list(destination.iterdir()), [])
    def test_sbom_runner_content_drift_is_rejected(self) -> None:
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        source = runner.read_text(encoding="utf-8")
        mutated = source.replace(
            'return [real_cargo, "metadata", "--locked", *arguments[1:]]',
            'return [real_cargo, "metadata", *arguments[1:]]',
            1,
        )
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / runner.name
            candidate.write_text(mutated, encoding="utf-8")
            with self.assertRaisesRegex(checker.ReleaseLockError, "outside the governed"):
                checker.check(WORKFLOW, candidate, ROOT)

    def test_sbom_metadata_shim_rejects_manifest_lock_drift(self) -> None:
        cargo = release_cargo()
        self.assertIsNotNone(cargo, "Cargo is required to prove release lock enforcement")
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "fixture"
            shutil.copytree(FIXTURE, root)
            original_lock = (root / "Cargo.lock").read_bytes()
            environment = os.environ.copy()
            environment["FORGE_RELEASE_LOCKED_CARGO_SHIM"] = "1"
            environment["FORGE_RELEASE_REAL_CARGO"] = str(cargo)
            completed = subprocess.run(
                [sys.executable, str(runner), "metadata", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)

    def test_sbom_shim_preserves_cargo_failure_status(self) -> None:
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            capture = root / "argv"
            fake = root / "cargo"
            fake.write_text(
                f"#!{sys.executable}\nimport pathlib,sys\npathlib.Path({str(capture)!r}).write_text('\\n'.join(sys.argv[1:]))\nraise SystemExit(47)\n",
                encoding="utf-8",
            )
            fake.chmod(0o755)
            environment = os.environ.copy()
            environment["FORGE_RELEASE_LOCKED_CARGO_SHIM"] = "1"
            environment["FORGE_RELEASE_REAL_CARGO"] = str(fake)
            completed = subprocess.run(
                [sys.executable, str(runner), "metadata", "--format-version", "1"],
                env=environment, text=True, capture_output=True, check=False, timeout=10,
            )
            self.assertEqual(completed.returncode, 47)
            self.assertEqual(capture.read_text(encoding="utf-8").splitlines()[:2], ["metadata", "--locked"])

    def test_real_cargo_cyclonedx_stale_lock_has_no_output(self) -> None:
        if shutil.which("cargo-cyclonedx") is None:
            self.skipTest("real cargo-cyclonedx 0.5.9 probe requires installed plugin")
        production_runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            runner = repository / "scripts/run-release-locked-sbom.py"
            runner.parent.mkdir()
            shutil.copy2(production_runner, runner)
            root = repository / "fixture"
            shutil.copytree(FIXTURE, root)
            commit_repository(repository)
            original_lock = (root / "Cargo.lock").read_bytes()
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(root / "Cargo.lock"), "--", "--format", "json", "--manifest-path", str(root / "Cargo.toml"), "--override-filename", "stale-proof.cdx"],
                cwd=root, text=True, capture_output=True, check=False, timeout=60,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
            self.assertEqual(list(root.rglob("*.cdx.json")), [])

    def test_manifest_lock_drift_fails_before_packaging(self) -> None:
        cargo = release_cargo()
        self.assertIsNotNone(cargo, "Cargo is required to prove release lock enforcement")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "fixture"
            shutil.copytree(FIXTURE, root)
            original_lock = (root / "Cargo.lock").read_bytes()
            environment = os.environ.copy()
            environment["CARGO_HOME"] = str(Path(directory) / "cargo-home")
            environment["CARGO_TARGET_DIR"] = str(Path(directory) / "target")
            completed = subprocess.run(
                [str(cargo), "package", "--locked", "--allow-dirty", "--no-verify", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("lock file", completed.stderr)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
            self.assertEqual(list(Path(directory).rglob("*.crate")), [])
            self.assertFalse((Path(directory) / "target/package").exists())

            generated = subprocess.run(
                [str(cargo), "generate-lockfile", "--offline", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertEqual(generated.returncode, 0, generated.stderr)
            repaired_lock = (root / "Cargo.lock").read_text(encoding="utf-8")
            self.assertNotEqual(repaired_lock.encode(), original_lock)
            self.assertIn('version = "0.2.0"', repaired_lock)


if __name__ == "__main__":
    unittest.main()

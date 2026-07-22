#!/usr/bin/env python3
"""Adversarial and real-toolchain tests for the fail-closed MSRV lane."""

from __future__ import annotations

import importlib.util
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/ci.yml"
POLICY_WORKFLOW = ROOT / ".github/workflows/msrv-policy.yml"
FIXTURE = ROOT / "contracts/fixtures/msrv/post-1.85-language"


def load_checker():
    path = ROOT / "scripts/check-msrv.py"
    spec = importlib.util.spec_from_file_location("forge_msrv_checker", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


class MsrvContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = WORKFLOW.read_text(encoding="utf-8")
        cls.policy_source = POLICY_WORKFLOW.read_text(encoding="utf-8")

    def replace_once(self, old: str, new: str) -> str:
        self.assertEqual(self.source.count(old), 1, old)
        return self.source.replace(old, new, 1)

    def assert_source_rejected(self, source: str, reason: str) -> None:
        with self.assertRaisesRegex(checker.MsrvCheckError, reason):
            checker.check_workflow_source(source)

    def assert_workflow_rejected(self, old: str, new: str, reason: str) -> None:
        self.assert_source_rejected(self.replace_once(old, new), reason)

    def replace_policy_once(self, old: str, new: str) -> str:
        self.assertEqual(self.policy_source.count(old), 1, old)
        return self.policy_source.replace(old, new, 1)

    def assert_policy_rejected(self, old: str, new: str, reason: str) -> None:
        with self.assertRaisesRegex(checker.MsrvCheckError, reason):
            checker.check_policy_workflow_source(self.replace_policy_once(old, new))

    def copied_manifests(self, destination: Path) -> None:
        shutil.copy2(ROOT / "Cargo.toml", destination / "Cargo.toml")
        shutil.copytree(ROOT / "crates", destination / "crates")

    def test_repository_contract_is_complete(self) -> None:
        packages = checker.check()
        self.assertEqual(len(packages), 23)
        self.assertEqual(len(packages), len(set(packages)))

    def test_duplicate_safe_structured_parse_preserves_scalars(self) -> None:
        document = checker.parse_workflow(self.source)
        self.assertEqual(document["on"]["pull_request"], "")
        self.assertEqual(document["concurrency"]["cancel-in-progress"], "true")
        header = (
            "  msrv:\n    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
        )
        duplicate = self.replace_once(header, header + "    needs: focused\n")
        self.assert_source_rejected(duplicate, "duplicate YAML key 'needs'")

    def test_rejects_yaml_anchors_aliases_merges_and_tags(self) -> None:
        msrv_header = (
            "  msrv:\n    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
        )
        mutations = [
            ("  static_docs:\n", "  static_docs: &template\n", "anchors and aliases"),
            ("  msrv:\n", "  msrv:\n    <<: *template\n", "anchors and aliases"),
            ("  msrv:\n", "  msrv:\n    <<: {if: false}\n", "merges are forbidden"),
            (
                msrv_header,
                msrv_header.replace("needs:", "!!str needs:"),
                "explicit YAML tags",
            ),
        ]
        for old, new, reason in mutations:
            with self.subTest(reason=reason):
                self.assert_workflow_rejected(old, new, reason)

    def test_rejects_newer_or_unpinned_toolchains(self) -> None:
        for replacement in ("1.85", "1.86.0", "stable"):
            with self.subTest(toolchain=replacement):
                mutated = self.source.replace(
                    "toolchain: 1.85.1", f"toolchain: {replacement}", 1
                )
                self.assert_source_rejected(mutated, "exact values")

    def test_requires_exact_pinned_no_deps_pyyaml_provisioning(self) -> None:
        mutations = [
            (f"{checker.PYYAML_INSTALL_COMMAND} && ", ""),
            (f"PyYAML=={checker.PYYAML_VERSION}", "PyYAML"),
            (f"PyYAML=={checker.PYYAML_VERSION}", "PyYAML==6.0.2"),
            (" --no-deps ", " "),
            ("python -m pip install", "pip install"),
        ]
        for old, new in mutations:
            with self.subTest(mutation=(old, new)):
                self.assert_workflow_rejected(old, new, "exact values")

    def test_rejects_pyyaml_install_after_contract_verification(self) -> None:
        self.assert_workflow_rejected(
            checker.CHECK_COMMAND,
            f"{checker.CONTRACT_COMMAND} && {checker.PYYAML_INSTALL_COMMAND}",
            "exact values",
        )

    def test_rejects_every_omitted_cargo_dimension(self) -> None:
        for flag in ("--locked", "--workspace", "--all-targets", "--all-features"):
            with self.subTest(flag=flag):
                mutated = self.source.replace(f" {flag}", "", 1)
                self.assert_source_rejected(mutated, "exact values")

    def test_rejects_toolchain_command_bypass(self) -> None:
        self.assert_workflow_rejected(
            "cargo +1.85.1 check", "cargo check", "exact values"
        )

    def test_rejects_exact_trigger_dependency_and_runner_drift(self) -> None:
        dependency = (
            "  msrv:\n    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
        )
        self.assert_workflow_rejected(
            dependency, dependency.replace("static_docs", "focused"), "job dependency"
        )
        runner = dependency + "    if: always()\n    runs-on: ubuntu-latest\n"
        self.assert_workflow_rejected(
            runner, runner.replace("ubuntu-latest", "windows-latest"), "job runner"
        )
        self.assert_workflow_rejected(
            "  pull_request:\n", "  workflow_dispatch:\n", "workflow triggers"
        )

    def test_rejects_unknown_or_forbidden_job_keys(self) -> None:
        for field in (
            "continue-on-error: true",
            "container: ubuntu:latest",
            "services: {}",
            "strategy: {}",
            "defaults: {}",
            "uses: ./reusable.yml",
            "shell: bash",
            "working-directory: crates",
            "permissions: read-all",
        ):
            with self.subTest(field=field):
                runner = (
                    "  msrv:\n    name: Rust 1.85 minimum supported version\n"
                    "    needs: static_docs\n    if: always()\n"
                    "    runs-on: ubuntu-latest\n"
                )
                mutated = self.replace_once(runner, runner + f"    {field}\n")
                self.assert_source_rejected(mutated, "msrv job keys")

    def test_rejects_job_environment_overrides_including_exact_reproducer(self) -> None:
        overrides = {
            "RUSTC": "/tmp/newer-rustc",
            "RUSTC_WRAPPER": "/tmp/wrapper",
            "RUSTDOC": "/tmp/rustdoc",
            "RUSTUP_TOOLCHAIN": "stable",
            "CARGO_BUILD_RUSTC": "/tmp/rustc",
            "RUSTFLAGS": "--cfg bypass",
            "CARGO_ENCODED_RUSTFLAGS": "--cfg\\u001fbypass",
            "PATH": "/tmp/bin",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": "/tmp/linker",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER": "/tmp/runner",
        }
        env = '      FORGE_CI_CACHE_CONTEXT: "disabled-msrv-1.85.1"\n'
        for key, value in overrides.items():
            with self.subTest(key=key):
                mutated = self.replace_once(env, f'{env}      {key}: "{value}"\n')
                self.assert_source_rejected(mutated, "msrv job environment")

    def test_rejects_workflow_environment_and_defaults_overrides(self) -> None:
        self.assert_workflow_rejected(
            '  CARGO_INCREMENTAL: "0"\n',
            '  CARGO_INCREMENTAL: "0"\n  RUSTC: /tmp/newer-rustc\n',
            "workflow environment",
        )
        self.assert_workflow_rejected(
            "jobs:\n",
            "defaults:\n  run:\n    shell: bash\n    working-directory: crates\njobs:\n",
            "CI workflow keys",
        )

    def test_rejects_continue_on_error_on_exact_compile_step(self) -> None:
        compile_timeout = (
            "      - name: Check complete candidate workspace at MSRV\n"
            "        timeout-minutes: 31\n"
        )
        self.assert_workflow_rejected(
            compile_timeout,
            compile_timeout + "        continue-on-error: true\n",
            "msrv step 'Check complete candidate workspace at MSRV' keys",
        )

    def test_rejects_compile_step_conditions_shell_directory_and_unknown_keys(
        self,
    ) -> None:
        compile_timeout = (
            "      - name: Check complete candidate workspace at MSRV\n"
            "        timeout-minutes: 31\n"
        )
        for field in (
            "if: false",
            "shell: bash",
            "working-directory: crates",
            "permissions: write-all",
        ):
            with self.subTest(field=field):
                self.assert_workflow_rejected(
                    compile_timeout,
                    compile_timeout + f"        {field}\n",
                    "msrv step 'Check complete candidate workspace at MSRV' keys",
                )

    def test_rejects_compile_step_environment_overrides(self) -> None:
        compile_timeout = (
            "      - name: Check complete candidate workspace at MSRV\n"
            "        timeout-minutes: 31\n"
        )
        for key in ("RUSTC", "RUSTC_WRAPPER", "RUSTUP_TOOLCHAIN", "PATH"):
            with self.subTest(key=key):
                self.assert_workflow_rejected(
                    compile_timeout,
                    compile_timeout + f"        env:\n          {key}: /tmp/bypass\n",
                    "msrv step 'Check complete candidate workspace at MSRV' keys",
                )

    def test_rejects_nameless_cache_action_exact_reproducer(self) -> None:
        install = "      - name: Install exact MSRV toolchain\n"
        self.assert_workflow_rejected(
            install,
            "      - uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32\n"
            + install,
            "step topology",
        )

    def test_rejects_nameless_run_and_uses_steps(self) -> None:
        install = "      - name: Install exact MSRV toolchain\n"
        for step in (
            "      - run: cargo +1.85.1 check --locked --workspace --all-targets --all-features\n",
            f"      - uses: {checker.CHECKOUT_ACTION}\n",
        ):
            with self.subTest(step=step.strip()):
                self.assert_workflow_rejected(install, step + install, "step topology")

    def test_rejects_extra_reordered_and_duplicate_steps(self) -> None:
        candidate_checkout = (
            "      - name: Checkout candidate as untrusted data\n"
            f"        uses: {checker.CHECKOUT_ACTION} # v4\n"
            "        with:\n"
            "          path: candidate\n"
            "          persist-credentials: false\n\n"
        )
        trusted_checkout = (
            "      - name: Checkout immutable trusted MSRV tools\n"
            f"        uses: {checker.CHECKOUT_ACTION} # v4\n"
            "        with:\n"
            "          repository: ${{ github.repository }}\n"
            f"          ref: {checker.TRUSTED_REF}\n"
            "          path: trusted\n"
            "          persist-credentials: false\n"
            "          fetch-depth: 1\n\n"
        )
        install = (
            "      - name: Install exact MSRV toolchain\n"
            f"        uses: {checker.TOOLCHAIN_ACTION} # stable action\n"
            "        with:\n"
            "          toolchain: 1.85.1\n\n"
        )
        mutations = [
            self.source.replace(install, install + candidate_checkout, 1),
            self.source.replace(
                candidate_checkout + trusted_checkout,
                trusted_checkout + candidate_checkout,
                1,
            ),
            self.source.replace(
                install,
                "      - name: Extra step\n        run: true\n\n" + install,
                1,
            ),
        ]
        for index, mutated in enumerate(mutations):
            with self.subTest(case=index):
                self.assert_source_rejected(mutated, "step topology")

    def test_rejects_cache_actions_regardless_of_name(self) -> None:
        msrv_checkout = (
            '      FORGE_CI_CACHE_CONTEXT: "disabled-msrv-1.85.1"\n'
            "    steps:\n"
            "      - name: Checkout candidate as untrusted data\n"
            f"        uses: {checker.CHECKOUT_ACTION} # v4\n"
        )
        mutated = msrv_checkout.replace(
            f"uses: {checker.CHECKOUT_ACTION}",
            "uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
        )
        self.assert_workflow_rejected(msrv_checkout, mutated, "exact values")

    def test_requires_immutable_trusted_checkout_boundary(self) -> None:
        mutations = [
            (
                f"          ref: {checker.TRUSTED_REF}\n",
                "          ref: ${{ github.sha }}\n",
            ),
            (
                "          path: trusted\n          persist-credentials: false\n",
                "          path: candidate/trusted\n          persist-credentials: false\n",
            ),
            (
                "          path: trusted\n          persist-credentials: false\n",
                "          path: trusted\n          persist-credentials: true\n",
            ),
            ("          fetch-depth: 1\n", "          fetch-depth: 0\n"),
        ]
        for old, new in mutations:
            with self.subTest(mutation=new.strip()):
                self.assert_workflow_rejected(old, new, "exact values")

    def test_requires_isolated_python_for_every_trusted_invocation(self) -> None:
        workflow_mutations = [
            (
                checker.CONTRACT_COMMAND,
                checker.CONTRACT_COMMAND.replace(
                    "python -I trusted/scripts/run-ci-tier.py",
                    "python trusted/scripts/run-ci-tier.py",
                ),
            ),
            (
                checker.CONTRACT_COMMAND,
                checker.CONTRACT_COMMAND.replace(
                    "-- python -I trusted/scripts/check-msrv.py",
                    "-- python trusted/scripts/check-msrv.py",
                ),
            ),
            (
                checker.CARGO_COMMAND,
                checker.CARGO_COMMAND.replace(
                    "python -I trusted/scripts/run-ci-tier.py",
                    "python trusted/scripts/run-ci-tier.py",
                ),
            ),
        ]
        for old, new in workflow_mutations:
            with self.subTest(mutation=new.split()[0:3]):
                self.assert_workflow_rejected(old, new, "exact values")
        self.assert_policy_rejected(
            checker.POLICY_COMMAND,
            checker.POLICY_COMMAND.replace("python -I", "python", 1),
            "policy steps",
        )

    def test_candidate_success_wrapper_is_never_accepted_or_executed(self) -> None:
        mutated = self.source.replace(
            checker.CARGO_COMMAND,
            checker.CARGO_COMMAND.replace(
                "trusted/scripts/run-ci-tier.py", "candidate/scripts/run-ci-tier.py"
            ),
            1,
        )
        self.assert_source_rejected(mutated, "exact values")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "candidate-wrapper-ran"
            fake = root / "candidate/scripts/run-ci-tier.py"
            fake.parent.mkdir(parents=True)
            fake.write_text(
                f"from pathlib import Path\nPath({str(marker)!r}).touch()\nraise SystemExit(0)\n",
                encoding="utf-8",
            )
            report = root / "trusted-report.json"
            result = subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(ROOT / "scripts/run-ci-tier.py"),
                    "--tier",
                    "adversarial-msrv-wrapper",
                    "--budget-seconds",
                    "30",
                    "--report",
                    str(report),
                    "--",
                    sys.executable,
                    "-c",
                    "raise SystemExit(7)",
                ],
                cwd=root,
                text=True,
                capture_output=True,
                timeout=60,
                check=False,
            )
            self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
            self.assertTrue(report.is_file())
            self.assertFalse(marker.exists(), "candidate success wrapper was executed")

    def test_rejects_action_step_unknown_fields_and_open_with_maps(self) -> None:
        self.assert_workflow_rejected(
            "          toolchain: 1.85.1\n",
            "          toolchain: 1.85.1\n          components: rustfmt\n",
            "exact values",
        )
        upload = "      - name: Upload MSRV timing reports\n        if: always()\n"
        self.assert_workflow_rejected(
            upload,
            upload + "        continue-on-error: true\n",
            "msrv step 'Upload MSRV timing reports' keys",
        )

    def test_rejects_missing_or_weakened_timing_artifact(self) -> None:
        upload = "      - name: Upload MSRV timing reports\n        if: always()\n"
        self.assert_workflow_rejected(
            upload, upload.replace("always()", "success()"), "exact values"
        )
        self.assert_workflow_rejected(
            "          retention-days: 14\n",
            "          retention-days: 1\n",
            "exact values",
        )
        self.assert_workflow_rejected(
            "--budget-seconds 1800 --report target/ci-timing/msrv-workspace.json",
            "--budget-seconds 99999 --report target/ci-timing/msrv-workspace.json",
            "exact values",
        )

    def test_rejects_both_skip_gate_attacks(self) -> None:
        static_header = (
            "  static_docs:\n"
            "    name: Tier 0 static and docs\n"
            "    runs-on: ubuntu-latest\n"
        )
        self.assert_workflow_rejected(
            static_header,
            static_header + "    if: false\n",
            "static_docs job keys",
        )
        self.assert_workflow_rejected(
            "    if: always()\n    runs-on: ubuntu-latest\n",
            "    if: false\n    runs-on: ubuntu-latest\n",
            "msrv job condition",
        )

    def test_policy_rejects_trigger_permissions_and_job_bypasses(self) -> None:
        mutations = [
            (
                "  pull_request_target:\n",
                "  pull_request:\n",
                "workflow triggers",
            ),
            ("  contents: read\n", "  contents: write\n", "workflow permissions"),
            (
                "    timeout-minutes: 10\n",
                "    timeout-minutes: 10\n    if: false\n",
                "enforce job keys",
            ),
            (
                "    timeout-minutes: 10\n",
                "    timeout-minutes: 10\n    continue-on-error: true\n",
                "enforce job keys",
            ),
            (
                "jobs:\n  enforce:\n",
                "jobs:\n  bypass:\n    runs-on: ubuntu-latest\n"
                "    steps: []\n  enforce:\n",
                "policy jobs keys",
            ),
        ]
        for old, new, reason in mutations:
            with self.subTest(mutation=new.strip().splitlines()[-1]):
                self.assert_policy_rejected(old, new, reason)

    def test_policy_rejects_mutable_checkout_and_candidate_execution(self) -> None:
        candidate_checkout = (
            "      - name: Checkout candidate as untrusted data\n"
            f"        uses: {checker.CHECKOUT_ACTION} # v4\n"
        )
        mutations = [
            (
                candidate_checkout,
                candidate_checkout.replace(
                    checker.CHECKOUT_ACTION, "actions/checkout@v4"
                ),
            ),
            (
                "          ref: ${{ github.event.pull_request.head.sha }}\n",
                "          ref: ${{ github.event.pull_request.head.ref }}\n",
            ),
            (
                "          path: candidate\n          persist-credentials: false\n",
                "          path: candidate\n          persist-credentials: true\n",
            ),
            (
                "      - name: Provision exact YAML parser\n",
                "      - name: Run candidate checker\n"
                "        run: python candidate/scripts/check-msrv.py\n\n"
                "      - name: Provision exact YAML parser\n",
            ),
            (
                checker.PYYAML_INSTALL_COMMAND,
                "python -m pip install PyYAML",
            ),
            (
                checker.POLICY_COMMAND,
                checker.POLICY_COMMAND.replace("trusted/scripts", "candidate/scripts"),
            ),
        ]
        for old, new in mutations:
            with self.subTest(mutation=new.splitlines()[0]):
                self.assert_policy_rejected(old, new, "policy steps")

    def test_policy_rejects_base_ref_and_unknown_step_keys(self) -> None:
        self.assert_policy_rejected(
            "          ref: ${{ github.event.pull_request.base.sha }}\n",
            "          ref: main\n",
            "policy steps",
        )
        provision = "      - name: Provision exact YAML parser\n"
        self.assert_policy_rejected(
            provision,
            provision + "        continue-on-error: true\n",
            "policy steps",
        )

    def test_policy_deletion_rename_and_symlink_fail_closed(self) -> None:
        missing = ROOT / ".github/workflows/msrv-policy-renamed.yml"
        with self.assertRaisesRegex(checker.MsrvCheckError, "required MSRV policy"):
            checker.check(policy_workflow=missing)

        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            root = Path(directory)
            self.copied_manifests(root)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            shutil.copy2(WORKFLOW, workflows / "ci.yml")
            (workflows / "msrv-policy.yml").symlink_to(POLICY_WORKFLOW)
            with self.assertRaisesRegex(checker.MsrvCheckError, "symbolic link"):
                checker.check(workflows / "ci.yml", root, workflows / "msrv-policy.yml")

    def test_rejects_candidate_root_cargo_compiler_overrides_and_aliases(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            cargo_dir = root / ".cargo"
            cargo_dir.mkdir()
            marker = root / "newer-rustc-wrapper-ran"
            wrapper = root / "newer-rustc-wrapper"
            wrapper.write_text(f"#!/bin/sh\ntouch {marker}\nexit 0\n", encoding="utf-8")
            (cargo_dir / "config.toml").write_text(
                f'[build]\nrustc-wrapper = "{wrapper}"\n', encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.MsrvCheckError, "candidate root .cargo/config.toml is forbidden"
            ):
                checker.check_manifests(root)
            self.assertFalse(marker.exists(), "compiler wrapper ran before rejection")

        for alias in ("config-symlink", "config-toml-directory", "cargo-symlink"):
            with self.subTest(alias=alias), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.copied_manifests(root)
                cargo_dir = root / ".cargo"
                target = root / "alias-target"
                if alias == "cargo-symlink":
                    target.mkdir()
                    cargo_dir.symlink_to(target, target_is_directory=True)
                else:
                    cargo_dir.mkdir()
                    if alias == "config-symlink":
                        target.write_text("[build]\n", encoding="utf-8")
                        (cargo_dir / "config").symlink_to(target)
                    else:
                        (cargo_dir / "config.toml").mkdir()
                with self.assertRaisesRegex(
                    checker.MsrvCheckError, "candidate root .cargo"
                ):
                    checker.check_manifests(root)

    def test_rejects_workspace_member_omission_and_undeclared_crate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            manifest = root / "Cargo.toml"
            text = manifest.read_text(encoding="utf-8")
            text = text.replace('  "crates/forge-core-research",\n', "", 1)
            manifest.write_text(text, encoding="utf-8")
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            ghost = root / "crates/undeclared/src"
            ghost.mkdir(parents=True)
            (ghost.parent / "Cargo.toml").write_text(
                '[package]\nname = "undeclared"\nversion = "0.1.0"\nedition = "2021"\n',
                encoding="utf-8",
            )
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

    def test_rejects_manifest_parse_and_msrv_override_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            member = root / "crates/forge-core-research/Cargo.toml"
            text = member.read_text(encoding="utf-8")
            member.write_text(
                text.replace(
                    "edition.workspace = true",
                    'edition.workspace = true\nrust-version = "1.86"',
                    1,
                ),
                encoding="utf-8",
            )
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            member = root / "crates/forge-core-research/Cargo.toml"
            member.write_text("not valid [toml", encoding="utf-8")
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

    def test_real_post_185_fixture_fails_for_intended_language_gate(self) -> None:
        version = subprocess.run(
            ["rustc", "+1.85.1", "--version"],
            text=True,
            capture_output=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(
            version.returncode, 0, f"missing exact toolchain: {version.stderr}"
        )
        self.assertRegex(version.stdout, r"^rustc 1\.85\.1 ")
        with tempfile.TemporaryDirectory() as target:
            result = subprocess.run(
                [
                    "cargo",
                    "+1.85.1",
                    "check",
                    "--manifest-path",
                    str(FIXTURE / "Cargo.toml"),
                    "--locked",
                    "--target-dir",
                    target,
                ],
                text=True,
                capture_output=True,
                timeout=120,
                check=False,
            )
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn("E0658", output)
        self.assertRegex(
            output,
            re.compile(r"`let` expressions? in this position (?:are|is) unstable"),
        )
        self.assertNotIn("toolchain", output.casefold().split("error[e0658]", 1)[0])

        current = subprocess.run(
            ["rustc", "--version"],
            text=True,
            capture_output=True,
            timeout=30,
            check=True,
        ).stdout
        match = re.match(r"rustc (\d+)\.(\d+)\.(\d+)", current)
        self.assertIsNotNone(match, current)
        assert match is not None
        if tuple(map(int, match.groups())) > (1, 85, 1):
            with tempfile.TemporaryDirectory() as target:
                accepted = subprocess.run(
                    [
                        "cargo",
                        "check",
                        "--manifest-path",
                        str(FIXTURE / "Cargo.toml"),
                        "--locked",
                        "--target-dir",
                        target,
                    ],
                    text=True,
                    capture_output=True,
                    timeout=120,
                    check=False,
                )
            self.assertEqual(accepted.returncode, 0, accepted.stdout + accepted.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)

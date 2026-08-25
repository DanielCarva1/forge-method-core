#!/usr/bin/env python3
"""Adversarial and real-toolchain tests for the fail-closed MSRV lane."""

from __future__ import annotations

import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest


TRUSTED_ROOT = Path(__file__).resolve().parents[1]
ROOT = Path(
    os.environ.get("FORGE_MSRV_CANDIDATE_ROOT", str(TRUSTED_ROOT))
).resolve()
POLICY_WORKFLOW = ROOT / ".github/workflows/msrv-policy.yml"
ROLLOUT = ROOT / "contracts/migration/msrv-policy-v2-rollout.yaml"
PHASE_ONE_WORKFLOW = ROOT / "contracts/fixtures/msrv-policy/phase-1-ci.yml"
PHASE_TWO_WORKFLOW = ROOT / "contracts/fixtures/msrv-policy/phase-2-ci.yml"
ACTUAL_WORKFLOW = ROOT / ".github/workflows/ci.yml"
WORKFLOW = (
    ACTUAL_WORKFLOW
    if "  ci-verdict:\n" in ACTUAL_WORKFLOW.read_text(encoding="utf-8")
    else PHASE_TWO_WORKFLOW
)
FIXTURE = TRUSTED_ROOT / "contracts/fixtures/msrv/post-1.85-language"


def load_checker():
    return load_checker_from(TRUSTED_ROOT)


def load_checker_from(root: Path):
    path = root / "scripts/check-msrv.py"
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
        packages = checker.check(
            workflow=WORKFLOW,
            root=ROOT,
            policy_workflow=POLICY_WORKFLOW,
        )
        self.assertEqual(len(packages), 23)
        self.assertEqual(len(packages), len(set(packages)))

    def test_static_docs_uses_the_rust_markdown_link_contract(self) -> None:
        self.assertNotIn("scripts/check-doc-links.py", self.source)
        self.assertEqual(self.source.count(checker.RUST_MARKDOWN_LINK_COMMAND), 1)

    def test_two_phase_rollout_contract_is_explicit_and_non_claiming(self) -> None:
        document = checker.parse_workflow(ROLLOUT.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "immutable_procedure")
        self.assertIn("not a tracker", document["progress_semantics"])
        self.assertIn("makes no claim", document["progress_semantics"])
        self.assertIn("cannot both replace", document["reason_single_change_is_unsafe"])
        evidence = document["default_branch_evidence"]
        self.assertEqual(
            evidence["protected_policy_base_filters"], ["master", "main"]
        )
        self.assertEqual(
            evidence["remote_required_checks_configuration"], "unverified"
        )
        self.assertEqual(evidence["required_checks_claim"], "not_made")
        states = {state["id"]: state for state in document["states"]}
        phase_one = states["phase-1-bootstrap-trust-root"]
        phase_two = states["phase-2-activate-protected-topology"]
        self.assertEqual(
            phase_one["ci_workflow_digest"],
            f"sha256:{checker.LEGACY_WORKFLOW_DIGEST}",
        )
        self.assertIn(
            "contracts/migration/markdown-debt-inventory.yaml",
            phase_one["lands"],
        )
        self.assertIn(
            "crates/forge-contract-validator/tests/parity.rs",
            phase_one["lands"],
        )
        self.assertIn(".github/workflows/ci.yml", phase_one["explicitly_does_not_land"])
        self.assertEqual(
            phase_two["ci_workflow_digest"],
            f"sha256:{checker.FINAL_WORKFLOW_DIGEST}",
        )
        self.assertEqual(phase_two["lands"], [".github/workflows/ci.yml"])
        self.assertIn("immutable base SHA", document["trust_rule"])

    def test_rollout_accepts_legacy_only_while_bootstrap_is_explicit(self) -> None:
        legacy_source = PHASE_ONE_WORKFLOW.read_text(encoding="utf-8")
        legacy_document = checker.parse_workflow(legacy_source)
        self.assertEqual(
            checker._normalized_digest(legacy_document),
            checker.LEGACY_WORKFLOW_DIGEST,
        )
        checker.check_workflow_source(
            legacy_source,
            bootstrap_legacy_allowed=True,
        )
        with self.assertRaisesRegex(
            checker.MsrvCheckError,
            "legacy CI topology is valid only during the audited bootstrap state",
        ):
            checker.check_workflow_source(legacy_source)
        final_source = PHASE_TWO_WORKFLOW.read_text(encoding="utf-8")
        self.assertEqual(
            checker._normalized_digest(checker.parse_workflow(final_source)),
            checker.FINAL_WORKFLOW_DIGEST,
        )
        checker.check_workflow_source(final_source)
        checker.check_workflow_source(self.source)

        # Reproduce the real cross-root boundary: an immutable phase-1 base
        # checker validates a phase-2 candidate without executing candidate
        # trust code or defaulting manifest reads back to the base root.
        with tempfile.TemporaryDirectory() as directory:
            trusted_phase_one = Path(directory)
            (trusted_phase_one / "scripts").mkdir(parents=True)
            for relative in checker.PROTECTED_TRUST_FILES:
                target = trusted_phase_one / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(TRUSTED_ROOT / relative, target)
            trusted_workflow = trusted_phase_one / ".github/workflows/ci.yml"
            trusted_workflow.parent.mkdir(parents=True)
            shutil.copy2(PHASE_ONE_WORKFLOW, trusted_workflow)
            phase_one_checker = load_checker_from(trusted_phase_one)
            packages = phase_one_checker.check(
                workflow=PHASE_TWO_WORKFLOW,
                root=ROOT,
                policy_workflow=POLICY_WORKFLOW,
            )
            self.assertEqual(len(packages), 23)

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
                mutated_command = checker.CHECK_COMMAND.replace(old, new, 1)
                self.assertNotEqual(mutated_command, checker.CHECK_COMMAND)
                self.assert_workflow_rejected(
                    checker.CHECK_COMMAND,
                    mutated_command,
                    "exact values",
                )

    def test_rejects_pyyaml_install_after_contract_verification(self) -> None:
        self.assert_workflow_rejected(
            checker.CHECK_COMMAND,
            f"{checker.CONTRACT_COMMAND} && {checker.PYYAML_INSTALL_COMMAND}",
            "exact values",
        )

    def test_rejects_every_omitted_cargo_dimension(self) -> None:
        for flag in ("--locked", "--workspace", "--all-targets", "--all-features"):
            with self.subTest(flag=flag):
                mutated_command = checker.CARGO_COMMAND.replace(f" {flag}", "", 1)
                mutated = self.source.replace(
                    checker.CARGO_COMMAND, mutated_command, 1
                )
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
            "  pull_request:\n",
            "  pull_request:\n    branches: [develop]\n",
            "workflow triggers",
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
        trusted_checkout = (
            "      - name: Checkout immutable trusted MSRV tools\n"
            f"        uses: {checker.CHECKOUT_ACTION} # v4\n"
            "        with:\n"
            "          repository: ${{ github.repository }}\n"
            f"          ref: {checker.TRUSTED_REF}\n"
            "          path: trusted\n"
            "          persist-credentials: false\n"
            "          fetch-depth: 1\n"
        )
        mutations = [
            (
                f"          ref: {checker.TRUSTED_REF}\n",
                "          ref: ${{ github.sha }}\n",
            ),
            ("          path: trusted\n", "          path: candidate/trusted\n"),
            (
                "          persist-credentials: false\n",
                "          persist-credentials: true\n",
            ),
            ("          fetch-depth: 1\n", "          fetch-depth: 0\n"),
        ]
        for old, new in mutations:
            with self.subTest(mutation=new.strip()):
                self.assert_workflow_rejected(
                    trusted_checkout, trusted_checkout.replace(old, new), "exact values"
                )

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
                    str(TRUSTED_ROOT / "scripts/run-ci-tier.py"),
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
        msrv_condition = (
            "  msrv:\n"
            "    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
            "    if: always()\n"
            "    runs-on: ubuntu-latest\n"
        )
        self.assert_workflow_rejected(
            msrv_condition,
            msrv_condition.replace("always()", "false"),
            "msrv job condition",
        )

    def test_required_ci_jobs_reject_job_or_step_allowed_failure(self) -> None:
        focused_header = (
            "  focused:\n"
            "    name: Focused package and integration evidence\n"
            "    needs: static_docs\n"
        )
        self.assert_workflow_rejected(
            focused_header,
            focused_header + "    continue-on-error: true\n",
            "required CI job 'focused' cannot use allowed-failure semantics",
        )
        focused_step = "      - name: Check generated command surface docs\n"
        self.assert_workflow_rejected(
            focused_step,
            focused_step + "        continue-on-error: true\n",
            "required CI job 'focused' step .* cannot use allowed-failure semantics",
        )

    def test_rejects_false_green_focused_and_test_lane_mutations(self) -> None:
        focused_start = self.source.index("  focused:\n")
        focused_end = self.source.index("\n  platform:\n", focused_start)
        focused = self.source[focused_start:focused_end]
        steps_start = focused.index("    steps:\n")
        without_steps = focused[:steps_start] + "    steps: []\n"
        self.assert_source_rejected(
            self.source[:focused_start] + without_steps + self.source[focused_end:],
            "focused job steps must be a non-empty",
        )

        focused_header = (
            "  focused:\n"
            "    name: Focused package and integration evidence\n"
            "    needs: static_docs\n"
        )
        self.assert_workflow_rejected(
            focused_header,
            focused_header + "    if: false\n",
            "focused job keys",
        )
        focused_run = (
            "run: python scripts/run-ci-tier.py --tier focused-command-surface "
            "--budget-seconds 900 --report target/ci-timing/command-surface.json -- "
            "cargo run -p forge-core-command-surface --example "
            "generate_command_surface_docs -- --check"
        )
        self.assert_workflow_rejected(
            focused_run,
            "run: true",
            "focused job topology, conditions, and commands",
        )
        self.assert_workflow_rejected(
            focused_run,
            focused_run + " || true",
            "focused job topology, conditions, and commands",
        )
        self.assert_workflow_rejected(
            " && python -I scripts/test-check-ci-verdict.py",
            "",
            "static_docs step .* fields must match reviewed exact values",
        )

    def test_protected_msrv_regression_lane_cannot_be_removed_or_swapped(self) -> None:
        step = (
            "      - name: Run protected MSRV topology regression suite\n"
            "        timeout-minutes: 7\n"
            f"        run: {checker.MSRV_REGRESSION_COMMAND}\n\n"
        )
        self.assertEqual(self.source.count(step), 1)
        self.assert_source_rejected(
            self.source.replace(step, "", 1),
            "msrv job step topology",
        )
        self.assert_workflow_rejected(
            checker.MSRV_REGRESSION_COMMAND,
            checker.MSRV_REGRESSION_COMMAND.replace(
                "trusted/scripts/test-msrv.py",
                "candidate/scripts/test-msrv.py",
            ),
            "exact values",
        )

    def test_informational_alpha_jobs_are_explicitly_excluded(self) -> None:
        self.assertNotIn("0.12.0-alpha.1", self.source)
        self.assert_workflow_rejected(
            '    name: "[informational prerelease] ${{ matrix.name }} platform observation"\n',
            '    name: "${{ matrix.name }} platform gate"\n',
            "informational prerelease CI job 'platform' name",
        )
        mutated = self.source.replace(
            "    continue-on-error: true\n",
            "    continue-on-error: false\n",
            1,
        )
        self.assert_source_rejected(
            mutated, "informational prerelease CI job 'platform' allowed-failure marker"
        )
        self.assert_workflow_rejected(
            "--informational platform \"Prerelease-channel native platform observations\"",
            "--mandatory platform \"Prerelease-channel native platform observations\"",
            "ci-verdict steps",
        )

    def test_ci_verdict_requires_all_dependencies_and_always_runs(self) -> None:
        self.assert_workflow_rejected(
            "    needs: [static_docs, msrv, focused, platform, expensive-journey]\n",
            "    needs: [static_docs, msrv, focused, platform]\n",
            "ci-verdict dependencies",
        )
        verdict_condition = (
            "  ci-verdict:\n"
            "    name: Required source-only CI verdict\n"
            "    needs: [static_docs, msrv, focused, platform, expensive-journey]\n"
            "    if: always()\n"
        )
        self.assert_workflow_rejected(
            verdict_condition,
            verdict_condition.replace("always()", "success()"),
            "ci-verdict condition",
        )
        self.assert_workflow_rejected(
            "    timeout-minutes: 5\n    steps:\n",
            "    timeout-minutes: 5\n    continue-on-error: true\n    steps:\n",
            "ci-verdict job keys",
        )
        self.assert_workflow_rejected(
            "python -I trusted/scripts/check-ci-verdict.py",
            "python scripts/check-ci-verdict.py",
            "ci-verdict steps",
        )

    def test_policy_rejects_trigger_permissions_and_job_bypasses(self) -> None:
        mutations = [
            (
                "  pull_request_target:\n",
                "  pull_request:\n",
                "workflow triggers",
            ),
            (
                "    branches: [master, main]\n",
                "    branches: [develop]\n",
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
            checker.check(
                workflow=WORKFLOW,
                root=ROOT,
                policy_workflow=missing,
            )

        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            root = Path(directory)
            self.copied_manifests(root)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            shutil.copy2(WORKFLOW, workflows / "ci.yml")
            (workflows / "msrv-policy.yml").symlink_to(POLICY_WORKFLOW)
            with self.assertRaisesRegex(checker.MsrvCheckError, "symbolic link"):
                checker.check(
                    workflow=workflows / "ci.yml",
                    root=root,
                    policy_workflow=workflows / "msrv-policy.yml",
                )

    def test_candidate_msrv_checker_drift_is_rejected_without_execution(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            shutil.copy2(WORKFLOW, workflows / "ci.yml")
            shutil.copy2(POLICY_WORKFLOW, workflows / "msrv-policy.yml")
            scripts = root / "scripts"
            scripts.mkdir()
            marker = root / "candidate-checker-ran"
            (scripts / "check-msrv.py").write_text(
                "from pathlib import Path\n"
                f"Path({str(marker)!r}).touch()\n"
                "raise SystemExit(0)\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                checker.MsrvCheckError, "candidate MSRV checker differs"
            ):
                checker.check(
                    workflow=workflows / "ci.yml",
                    root=root,
                    policy_workflow=workflows / "msrv-policy.yml",
                )
            self.assertFalse(marker.exists(), "candidate MSRV checker was executed")

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

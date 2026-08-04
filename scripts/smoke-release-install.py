#!/usr/bin/env python3
"""Extract a release archive and prove its packaged Solo Dogfood journey."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import stat
import subprocess
import tempfile
import time
from typing import Any


class InstallSmokeError(RuntimeError):
    """The packaged release cannot complete the native installation journey."""

    def __init__(
        self,
        message: str,
        *,
        safe_message: str | None = None,
        stage: str = "validation",
        exit_code: int | None = None,
        timeout_seconds: int | None = None,
    ) -> None:
        super().__init__(message)
        self.safe_message = (
            safe_message
            if safe_message is not None
            else "packaged journey validation failed"
        )
        self.stage = stage
        self.exit_code = exit_code
        self.timeout_seconds = timeout_seconds


_SECRET_PATTERNS = (
    re.compile(
        r"(?i)\b(authorization|api[_-]?key|token|secret|password)"
        r"(\s*[:=]\s*)([^\s,;]+)"
    ),
    re.compile(r"(?i)\bbearer\s+[^\s,;]+"),
    re.compile(r"(?i)\b(?:sk|ghp|github_pat|xox[baprs])[-_][A-Za-z0-9_-]{8,}"),
)
_DIAGNOSTIC_LIMIT = 240


def sanitize_diagnostic_message(message: str) -> str:
    """Return one bounded secret-safe line suitable for retained evidence."""

    sanitized = " ".join(message.split())
    for pattern in _SECRET_PATTERNS:
        if "bearer" in pattern.pattern.casefold():
            sanitized = pattern.sub("Bearer [REDACTED]", sanitized)
        elif pattern.groups >= 3:
            sanitized = pattern.sub(
                lambda match: f"{match.group(1)}{match.group(2)}[REDACTED]",
                sanitized,
            )
        else:
            sanitized = pattern.sub("[REDACTED]", sanitized)
    if not sanitized:
        sanitized = "packaged journey failed"
    return sanitized[:_DIAGNOSTIC_LIMIT]


def evidence_diagnostic(error: Exception, fallback_stage: str) -> dict[str, Any]:
    """Project an exception to the intentionally tiny persisted diagnostic."""

    if isinstance(error, InstallSmokeError):
        stage = error.stage if error.stage != "validation" else fallback_stage
        message = error.safe_message
        exit_code = error.exit_code
        timeout_seconds = error.timeout_seconds
    elif isinstance(error, KeyError):
        stage = fallback_stage
        message = "an expected response field was absent"
        exit_code = None
        timeout_seconds = None
    else:
        stage = fallback_stage
        message = f"unexpected {type(error).__name__} during packaged journey"
        exit_code = None
        timeout_seconds = None
    diagnostic: dict[str, Any] = {
        "stage": sanitize_diagnostic_message(stage),
        "message": sanitize_diagnostic_message(message),
    }
    if exit_code is not None:
        diagnostic["exit_code"] = exit_code
    if timeout_seconds is not None:
        diagnostic["timeout_seconds"] = timeout_seconds
    return diagnostic


def load_checker():
    script = Path(__file__).resolve().with_name("check-release-archive.py")
    spec = importlib.util.spec_from_file_location("forge_release_checker_for_smoke", script)
    if spec is None or spec.loader is None:
        raise InstallSmokeError(f"cannot load archive reader from {script}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def extract_checked_members(archive: Path, destination: Path) -> dict[str, Path]:
    """Extract only regular, canonical members accepted by the archive checker."""

    checker = load_checker()
    try:
        members = checker.read_members(archive)
    except (OSError, checker.ArchiveCheckError) as error:
        raise InstallSmokeError(f"unsafe or unreadable archive: {error}") from error

    extracted: dict[str, Path] = {}
    for archive_path, (content, mode) in members.items():
        target = destination.joinpath(*PurePosixPath(archive_path).parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            with target.open("xb") as stream:
                stream.write(content)
            target.chmod(stat.S_IMODE(mode))
        except OSError as error:
            raise InstallSmokeError(f"extract {archive_path}: {error}") from error
        extracted[archive_path] = target
    return extracted


def run(
    command: list[os.PathLike[str] | str], label: str, timeout_seconds: int
) -> subprocess.CompletedProcess[str]:
    try:
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise InstallSmokeError(
            f"{label} exceeded bounded {timeout_seconds}-second execution window\n"
            f"stdout:\n{error.stdout or ''}\nstderr:\n{error.stderr or ''}",
            safe_message=f"{label} timed out",
            stage=label,
            timeout_seconds=timeout_seconds,
        ) from error
    if completed.returncode != 0:
        raise InstallSmokeError(
            f"{label} failed with exit {completed.returncode}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}",
            safe_message=f"{label} failed",
            stage=label,
            exit_code=completed.returncode,
        )
    return completed


def wrapper_command(wrapper: Path, arguments: list[str]) -> list[str]:
    if os.name != "nt":
        return [str(wrapper), *arguments]
    # `call` preserves the batch wrapper's exit status and handles a quoted
    # extraction path without asking Python to reinterpret command arguments.
    command_line = "call " + subprocess.list2cmdline([str(wrapper), *arguments])
    return ["cmd.exe", "/d", "/s", "/c", command_line]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise InstallSmokeError(message)


def require_version(
    command: list[str], expected: str, label: str, timeout_seconds: int
) -> None:
    actual = run(command, label, timeout_seconds).stdout.strip()
    if actual != expected:
        raise InstallSmokeError(
            f"{label}: expected {expected!r}, got {actual!r}",
            safe_message=f"{label} version mismatch",
            stage=label,
        )


def require_ok_json(command: list[str], label: str, timeout_seconds: int) -> dict[str, Any]:
    completed = run(command, label, timeout_seconds)
    try:
        envelope = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise InstallSmokeError(
            f"{label} did not emit JSON: {error}",
            safe_message=f"{label} did not emit valid JSON",
            stage=label,
        ) from error
    if not isinstance(envelope, dict) or envelope.get("ok") is not True:
        raise InstallSmokeError(
            f"{label} did not emit an ok envelope: {envelope!r}",
            safe_message=f"{label} emitted a non-success envelope",
            stage=label,
        )
    return envelope


def timed_ok_json(
    command: list[str], label: str, timeout_seconds: int
) -> tuple[dict[str, Any], float]:
    started = time.perf_counter()
    result = require_ok_json(command, label, timeout_seconds)
    return result, round(time.perf_counter() - started, 3)


def git_output(project: Path, *arguments: str) -> str:
    return run(
        ["git", "-C", str(project), *arguments],
        f"git {' '.join(arguments)}",
        30,
    ).stdout


def run_git(project: Path, *arguments: str) -> None:
    git_output(project, *arguments)


def write_json(path: Path, value: object) -> Path:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return path


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def count_cleanup_debt(root: Path) -> int:
    if not root.exists():
        return 0
    return sum(
        1
        for path in root.rglob("*")
        if ".forge-retained-delete-" in path.name
        or ".forge-crash-absence-claim-" in path.name
    )


def forge_call(
    wrapper: Path,
    arguments: list[str],
    label: str,
    timeout_seconds: int,
    timings: dict[str, float],
) -> dict[str, Any]:
    result, duration = timed_ok_json(
        wrapper_command(wrapper, arguments),
        label,
        timeout_seconds,
    )
    timings[label] = duration
    return result


def resume_summary_and_report(
    wrapper: Path,
    project: Path,
    timeout_seconds: int,
    timings: dict[str, float],
    label_prefix: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Run normal activation and the separate historical report."""

    summary = forge_call(
        wrapper,
        ["workflow", "resume", "--root", str(project), "--json"],
        f"{label_prefix} summary",
        timeout_seconds,
        timings,
    )
    data = summary.get("data")
    require(isinstance(data, dict), "workflow resume summary lacks data")
    require(
        data.get("schema_version") == "workflow_resume_summary_v3",
        "workflow resume default lacks workflow_resume_summary_v3",
    )
    require(
        "detail_argv" not in data,
        "workflow resume must not advertise a second mandatory detail pass",
    )
    report = forge_call(
        wrapper,
        ["workflow", "report", "--root", str(project), "--json"],
        f"{label_prefix} historical report",
        timeout_seconds,
        timings,
    )
    return summary, report


def require_release_status(release_status: dict[str, Any]) -> str:
    try:
        active = release_status["data"]["active"]
        release_id = active["release"]["release_id"]
        runtime_bundle = active["runtime_bundle"]
    except (KeyError, TypeError) as error:
        raise InstallSmokeError(
            "workflow release-status omitted its historical active release identity",
            stage="workflow release-status",
        ) from error
    require(
        isinstance(release_id, str) and bool(release_id),
        "workflow release-status returned an invalid active release id",
    )
    require(
        isinstance(runtime_bundle, dict)
        and isinstance(runtime_bundle.get("bundle_id"), str)
        and isinstance(runtime_bundle.get("bundle_digest"), str),
        "workflow release-status returned an invalid active runtime bundle",
    )
    return release_id


def require_archive_identity(
    extracted: dict[str, Path], expected_version: str
) -> dict[str, Any]:
    manifest_path = extracted.get("RELEASE-MANIFEST.json")
    if manifest_path is None:
        raise InstallSmokeError("archive lacks RELEASE-MANIFEST.json")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise InstallSmokeError(f"invalid RELEASE-MANIFEST.json: {error}") from error
    require(isinstance(manifest, dict), "release manifest must be an object")
    require(
        manifest.get("version") == expected_version,
        "release manifest version does not match the requested package version",
    )
    require(
        manifest.get("release_tag") == f"v{expected_version}",
        "release manifest tag does not bind the requested package version",
    )
    return manifest


def run_solo_journey(args: argparse.Namespace, ordinal: int) -> dict[str, Any]:
    started = time.perf_counter()
    timings: dict[str, float] = {}
    with tempfile.TemporaryDirectory(prefix="forge-packaged-solo-") as raw_directory:
        # macOS commonly exposes /var as a symlink to /private/var. Forge
        # correctly rejects symlink path components, so hand it the physical
        # temporary path rather than the user-facing alias.
        root = Path(raw_directory).resolve(strict=True)
        install_root = root / "installed"
        install_root.mkdir()
        extracted = extract_checked_members(args.archive, install_root)

        binary = extracted.get(args.binary_name)
        wrapper = extracted.get(args.wrapper_name)
        if binary is None or wrapper is None:
            raise InstallSmokeError(
                "archive lacks requested executable members: "
                f"binary={args.binary_name!r}, wrapper={args.wrapper_name!r}"
            )
        expected_version = f"forge-core {args.version}"
        require_version(
            [str(binary), "--version"],
            expected_version,
            "packaged binary --version",
            args.command_timeout_seconds,
        )
        require_version(
            wrapper_command(wrapper, ["--version"]),
            expected_version,
            "packaged wrapper --version",
            args.command_timeout_seconds,
        )
        manifest = require_archive_identity(extracted, args.version)

        project = root / "consumer project"
        project.mkdir()
        (project / ".gitignore").write_text(
            ".local/\ntarget/\n", encoding="utf-8", newline="\n"
        )
        (project / ".gitattributes").write_text(
            "LINE_ENDINGS.txt text eol=lf\n", encoding="utf-8", newline="\n"
        )
        (project / "README.md").write_text(
            "packaged consumer\n", encoding="utf-8", newline="\n"
        )
        (project / "LINE_ENDINGS.txt").write_text(
            "stable\n", encoding="utf-8", newline="\n"
        )
        run_git(project, "init", "-b", "master")
        run_git(project, "config", "user.name", "Forge Packaged Solo Gate")
        run_git(project, "config", "user.email", "packaged-gate@example.invalid")
        run_git(project, "config", "commit.gpgsign", "false")
        run_git(project, "config", "core.autocrlf", "false")
        run_git(project, "add", ".")
        run_git(project, "commit", "-m", "initial packaged consumer")

        root_args = ["--root", str(project), "--json"]
        start = forge_call(
            wrapper,
            ["start", *root_args],
            "start",
            args.command_timeout_seconds,
            timings,
        )
        state_root = Path(start["data"]["project"]["state_root"])
        require(
            (project / ".forge-method.yaml").is_file(),
            "start did not create the project link",
        )
        run_git(project, "add", ".forge-method.yaml")
        run_git(project, "commit", "-m", "track packaged Forge project link")
        forge_call(
            wrapper,
            ["workflow", "init", *root_args],
            "workflow init",
            args.command_timeout_seconds,
            timings,
        )
        release_status = forge_call(
            wrapper,
            ["workflow", "release-status", *root_args],
            "workflow release-status",
            args.command_timeout_seconds,
            timings,
        )
        active_release_id = require_release_status(release_status)
        guidance = forge_call(
            wrapper,
            ["workflow", "next", *root_args],
            "workflow next for objective",
            args.command_timeout_seconds,
            timings,
        )
        packets = guidance["data"]["authorization"]["action_packets"]
        packet = next(
            (
                candidate
                for candidate in packets
                if candidate.get("authorization_kind") == "intent_revision"
            ),
            None,
        )
        require(packet is not None, "workflow next did not offer an objective packet")
        now_unix = str(int(time.time()))
        principal = f"principal.agent.packaged-solo-{ordinal}"
        agent = f"agent.packaged-solo-{ordinal}"
        conversation_digest = "sha256:" + hashlib.sha256(
            f"{args.archive.name}:{ordinal}:objective".encode()
        ).hexdigest()
        objective_path = write_json(
            root / "objective.json",
            {
                "kind": "unambiguous",
                "proposal": {
                    "outcome": "Apply one exact isolated regular-file write",
                    "constraints": [
                        "retain exact claim authority",
                        "read back canonical bytes",
                    ],
                    "unacceptable_outcomes": ["copy files outside governed apply"],
                    "open_uncertainties": [],
                },
                "carrying_principal": principal,
                "host_provenance": {
                    "host_id": "host.packaged-runtime-gate",
                    "host_version": args.version,
                    "session_ref": f"session.packaged-solo-{ordinal}",
                    "interaction_ref": "turn.objective",
                    "conversation_digest": conversation_digest,
                    "observed_at_unix": int(now_unix),
                },
            },
        )
        forge_call(
            wrapper,
            [
                "workflow",
                "intent",
                "accept-cooperative",
                "--root",
                str(project),
                "--packet-digest",
                packet["packet_digest"],
                "--input-file",
                str(objective_path),
                "--json",
            ],
            "accept synthetic cooperative objective",
            args.command_timeout_seconds,
            timings,
        )
        claim = forge_call(
            wrapper,
            [
                "claim",
                "acquire",
                "--root",
                str(project),
                "--scope",
                "story",
                "--id",
                "packaged-solo",
                "--agent",
                agent,
                "--principal-id",
                principal,
                "--path",
                "README.md",
                "--now-unix",
                now_unix,
                "--json",
            ],
            "acquire exact claim",
            args.command_timeout_seconds,
            timings,
        )
        claim_id = claim["data"]["claim_id"]

        worktree = root / "worktrees" / "packaged"
        worktree.parent.mkdir()
        run_git(
            project,
            "worktree",
            "add",
            "-b",
            "agent/packaged",
            str(worktree),
            "master",
        )
        (worktree / "README.md").write_text(
            "packaged consumer\ngoverned packaged change\n",
            encoding="utf-8",
            newline="\n",
        )
        (worktree / "LINE_ENDINGS.txt").write_bytes(b"stable\r\n")
        (worktree / ".local").mkdir()
        (worktree / ".local" / "journal.md").write_text(
            "must not be promoted\n", encoding="utf-8", newline="\n"
        )
        (worktree / "untracked.tmp").write_text(
            "must not be promoted\n", encoding="utf-8", newline="\n"
        )
        isolation_id = "isolation.packaged-solo"
        worktree_path = Path(os.path.relpath(worktree, project)).as_posix()
        forge_call(
            wrapper,
            [
                "isolation",
                "propose",
                "--root",
                str(project),
                "--agent",
                agent,
                "--branch",
                "agent/packaged",
                "--worktree-path",
                worktree_path,
                "--base-ref",
                "master",
                "--claim",
                claim_id,
                "--id",
                isolation_id,
                "--now-unix",
                now_unix,
                "--json",
            ],
            "propose isolated work",
            args.command_timeout_seconds,
            timings,
        )
        forge_call(
            wrapper,
            [
                "isolation",
                "transition",
                "--root",
                str(project),
                "--id",
                isolation_id,
                "--to",
                "active",
                "--now-unix",
                now_unix,
                "--json",
            ],
            "activate isolated work",
            args.command_timeout_seconds,
            timings,
        )

        evidence_guidance = forge_call(
            wrapper,
            ["workflow", "next", *root_args],
            "workflow next for evidence",
            args.command_timeout_seconds,
            timings,
        )
        evidence_packet = evidence_guidance["data"].get(
            "cooperative_evidence_action_packet"
        )
        require(
            isinstance(evidence_packet, dict),
            "workflow next did not offer cooperative evidence",
        )
        offer = evidence_packet["offer_template"]
        offer["offer_id"] = f"offer.packaged-solo-{ordinal}.pass"
        evidence_path = write_json(root / "evidence.json", offer)
        admitted = forge_call(
            wrapper,
            [
                "workflow",
                "evidence",
                "admit-cooperative",
                "--root",
                str(project),
                "--input-file",
                str(evidence_path),
                "--json",
            ],
            "admit cooperative evidence",
            args.command_timeout_seconds,
            timings,
        )
        admitted_evidence = admitted["data"]["event"]["payload"]["admitted_evidence"]
        evidence_summary, suppression = resume_summary_and_report(
            wrapper,
            project,
            args.command_timeout_seconds,
            timings,
            "resume after evidence",
        )
        require(
            suppression["data"].get("cooperative_evidence_action_packet") is None,
            "current supporting evidence was offered again",
        )
        evidence_audit = suppression["data"].get("cooperative_evidence", [])
        require(
            any(item.get("current_status") == "supporting" for item in evidence_audit),
            "admitted evidence did not survive a fresh process",
        )
        supporting_evidence = next(
            item
            for item in evidence_audit
            if item.get("current_status") == "supporting"
        )
        require(
            admitted_evidence.get("outcome") == "pass",
            "cooperative evidence was not admitted as passing",
        )

        preview = forge_call(
            wrapper,
            [
                "workflow",
                "promotion",
                "preview",
                "--root",
                str(project),
                "--isolation-id",
                isolation_id,
                "--json",
            ],
            "preview exact promotion",
            args.command_timeout_seconds,
            timings,
        )
        require(preview["data"]["status"] == "reviewable", "promotion is not reviewable")
        require(
            preview["data"]["write_set"] == ["README.md"],
            f"promotion write set is not exact: {preview['data']['write_set']!r}",
        )
        require(
            (project / "README.md").read_text(encoding="utf-8")
            == "packaged consumer\n",
            "preview changed the canonical project",
        )
        preview_digest = preview["data"]["preview_digest"]
        promotion_args = [
            "--root",
            str(project),
            "--isolation-id",
            isolation_id,
            "--expected-preview-digest",
            preview_digest,
            "--json",
        ]
        applied = forge_call(
            wrapper,
            ["workflow", "promotion", "apply", *promotion_args],
            "apply exact promotion",
            args.command_timeout_seconds,
            timings,
        )
        require(applied["data"]["status"] == "applied", "promotion was not applied")
        receipt = applied["data"]["receipt"]
        require(receipt["readback_verified"] is True, "promotion readback was not verified")
        require(
            (project / "README.md").read_text(encoding="utf-8")
            == "packaged consumer\ngoverned packaged change\n",
            "canonical project bytes do not match the promoted result",
        )
        require(
            not (project / ".local" / "journal.md").exists(),
            "ignored local journal leaked into the canonical project",
        )
        require(
            not (project / "untracked.tmp").exists(),
            "unclaimed file leaked into the canonical project",
        )
        recovered = forge_call(
            wrapper,
            ["workflow", "promotion", "recover", *promotion_args],
            "recover completed promotion",
            args.command_timeout_seconds,
            timings,
        )
        require(
            recovered["data"]["status"] == "already_committed",
            "completed promotion recovery was not idempotent",
        )
        retry = forge_call(
            wrapper,
            ["workflow", "promotion", "apply", *promotion_args],
            "retry exact promotion",
            args.command_timeout_seconds,
            timings,
        )
        require(
            retry["data"]["status"] == "already_committed",
            "exact promotion retry was not idempotent",
        )
        receipt_digest = receipt["receipt_digest"]
        require(
            recovered["data"]["receipt"]["receipt_digest"] == receipt_digest
            and retry["data"]["receipt"]["receipt_digest"] == receipt_digest,
            "recovery or retry changed the durable receipt",
        )
        for target_status, label in [
            ("merging", "mark isolation merging"),
            ("merged", "mark isolation merged"),
        ]:
            forge_call(
                wrapper,
                [
                    "isolation",
                    "transition",
                    "--root",
                    str(project),
                    "--id",
                    isolation_id,
                    "--to",
                    target_status,
                    "--now-unix",
                    now_unix,
                    "--json",
                ],
                label,
                args.command_timeout_seconds,
                timings,
            )
        forge_call(
            wrapper,
            [
                "claim",
                "release",
                "--root",
                str(project),
                "--id",
                claim_id,
                "--agent",
                agent,
                "--now-unix",
                now_unix,
                "--json",
            ],
            "release exact claim",
            args.command_timeout_seconds,
            timings,
        )
        run_git(project, "worktree", "remove", "--force", str(worktree))
        run_git(project, "worktree", "prune")
        run_git(project, "branch", "-D", "agent/packaged")

        claim_status = forge_call(
            wrapper,
            [
                "claim",
                "status",
                "--root",
                str(project),
                "--now-unix",
                now_unix,
                "--json",
            ],
            "verify no active claims",
            args.command_timeout_seconds,
            timings,
        )
        require(
            claim_status["data"].get("active") == [],
            "released claim remained active",
        )
        isolation_status = forge_call(
            wrapper,
            ["isolation", "status", "--root", str(project), "--json"],
            "verify no active isolations",
            args.command_timeout_seconds,
            timings,
        )
        isolation_records = isolation_status["data"].get("active", [])
        require(
            isinstance(isolation_records, list)
            and all(
                isinstance(item, dict)
                and item.get("status") not in {"active", "merging"}
                for item in isolation_records
            ),
            "an isolation remained active after merge finalization",
        )

        replacement_summary, replacement = resume_summary_and_report(
            wrapper,
            project,
            args.command_timeout_seconds,
            timings,
            "replacement process resume",
        )
        require(
            replacement_summary["data"].get("active_isolations") == [],
            "resume summary still reported an active isolation",
        )
        continuity = replacement["data"]["replacement_continuity"]
        require(
            continuity["status"] == "ready",
            f"replacement continuity is not ready: {continuity['status']!r}",
        )
        completed = next(
            (
                promotion
                for promotion in continuity["promotions"]
                if promotion.get("preview_digest") == preview_digest
            ),
            None,
        )
        require(
            completed is not None and completed.get("status") == "completed",
            "replacement process did not observe the completed promotion",
        )

        worktree_listing = git_output(project, "worktree", "list", "--porcelain")
        listed_worktrees = [
            str(Path(line.removeprefix("worktree ")).resolve())
            for line in worktree_listing.splitlines()
            if line.startswith("worktree ")
        ]
        require(
            len(listed_worktrees) == 1
            and os.path.normcase(listed_worktrees[0])
            == os.path.normcase(str(project.resolve())),
            f"unexpected retained Git worktrees: {listed_worktrees!r}",
        )
        branches = [
            line.strip()
            for line in git_output(project, "branch", "--format=%(refname:short)").splitlines()
            if line.strip()
        ]
        require(branches == ["master"], f"unexpected retained branches: {branches!r}")
        git_status = git_output(
            project,
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        ).splitlines()
        require(
            git_status == [" M README.md"],
            f"canonical Git state differs from the exact promoted README change: {git_status!r}",
        )

        cleanup_debt_before = count_cleanup_debt(state_root)
        for pass_number in range(1, 4):
            forge_call(
                wrapper,
                [
                    "domain-pack",
                    "status",
                    "--state-root",
                    str(state_root),
                    "--json",
                ],
                f"cleanup status {pass_number}",
                args.command_timeout_seconds,
                timings,
            )
            forge_call(
                wrapper,
                [
                    "domain-pack",
                    "recover",
                    "--state-root",
                    str(state_root),
                    "--json",
                ],
                f"cleanup recover {pass_number}",
                args.command_timeout_seconds,
                timings,
            )
        cleanup_debt_after = count_cleanup_debt(state_root)
        require(
            cleanup_debt_before == cleanup_debt_after == 0,
            "read-only/recovery passes accumulated cleanup debt",
        )

        return {
            "ordinal": ordinal,
            "status": "passed",
            "duration_seconds": round(time.perf_counter() - started, 3),
            "start_count": 1,
            "release_tag": manifest["release_tag"],
            "source_commit": manifest["source_commit"],
            "active_release_id": active_release_id,
            "resume_summary_schema": evidence_summary["data"]["schema_version"],
            "binary_sha256": file_sha256(binary),
            "wrapper_sha256": file_sha256(wrapper),
            "objective_packet_digest": packet["packet_digest"],
            "evidence_receipt_digest": supporting_evidence["record_digest"],
            "promotion_preview_digest": preview_digest,
            "promotion_receipt_digest": receipt_digest,
            "write_set": preview["data"]["write_set"],
            "apply_status": applied["data"]["status"],
            "recover_status": recovered["data"]["status"],
            "retry_status": retry["data"]["status"],
            "replacement_continuity": continuity["status"],
            "active_claim_count": len(claim_status["data"]["active"]),
            "active_isolation_count": len(replacement_summary["data"]["active_isolations"]),
            "worktree_count": len(listed_worktrees),
            "branches": branches,
            "git_status": git_status,
            "cleanup_debt_before": cleanup_debt_before,
            "cleanup_debt_after": cleanup_debt_after,
            "timings_seconds": timings,
        }


def minimal_evidence_base(args: argparse.Namespace) -> dict[str, Any]:
    """Build a no-I/O fallback so broad execution failures still retain JSON."""

    archive = getattr(args, "archive", None)
    return {
        "schema_version": "packaged_solo_dogfood_evidence_v1",
        "status": "running",
        "proof_scope": "packaged_runtime",
        "host_authenticity_proven": False,
        "supported_hosts": [],
        "archive": Path(archive).name if archive is not None else "unknown",
        "archive_sha256": None,
        "version": str(getattr(args, "version", "unknown")),
        "platform": {
            "system": platform.system(),
            "machine": platform.machine(),
        },
        "requested_runs": int(getattr(args, "journey_runs", 0)),
        "runs": [],
    }


def evidence_base(args: argparse.Namespace) -> dict[str, Any]:
    evidence = minimal_evidence_base(args)
    if args.archive.is_file():
        evidence["archive_sha256"] = file_sha256(args.archive)
    return evidence


def smoke(args: argparse.Namespace, evidence: dict[str, Any]) -> None:
    actual_arch = platform.machine().casefold()
    expected_arch = args.expected_host_arch.casefold()
    if actual_arch != expected_arch:
        raise InstallSmokeError(
            f"host architecture boundary mismatch: expected {expected_arch!r}, got {actual_arch!r}"
        )
    if not 1 <= args.command_timeout_seconds <= 300:
        raise InstallSmokeError("command timeout must be between 1 and 300 seconds")
    if not 1 <= args.journey_runs <= 10:
        raise InstallSmokeError("journey runs must be between 1 and 10")

    started = time.perf_counter()
    for ordinal in range(1, args.journey_runs + 1):
        run_started = time.perf_counter()
        try:
            evidence["runs"].append(run_solo_journey(args, ordinal))
        except Exception as error:
            evidence["runs"].append(
                {
                    "ordinal": ordinal,
                    "status": "failed",
                    "duration_seconds": round(time.perf_counter() - run_started, 3),
                    "diagnostic": evidence_diagnostic(error, f"journey {ordinal}"),
                }
            )
            raise
    evidence["status"] = "passed"
    evidence["duration_seconds"] = round(time.perf_counter() - started, 3)


def write_evidence(path: Path | None, evidence: dict[str, Any]) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    write_json(path, evidence)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--archive", type=Path, required=True)
    result.add_argument("--binary-name", required=True)
    result.add_argument("--wrapper-name", required=True)
    result.add_argument("--version", required=True)
    result.add_argument("--expected-host-arch", required=True)
    result.add_argument("--command-timeout-seconds", type=int, default=60)
    result.add_argument(
        "--journey-runs",
        type=int,
        default=1,
        help="number of fresh consecutive packaged journeys (1-10)",
    )
    result.add_argument(
        "--evidence-output",
        type=Path,
        help="retain a machine-readable result without raw command output",
    )
    return result


if __name__ == "__main__":
    arguments = parser().parse_args()
    report = minimal_evidence_base(arguments)
    stage = "build evidence base"
    try:
        report = evidence_base(arguments)
        stage = "execute packaged journey"
        smoke(arguments, report)
        stage = "write passed evidence"
        write_evidence(arguments.evidence_output, report)
    except Exception as error:
        report["status"] = "failed"
        report["diagnostic"] = evidence_diagnostic(error, stage)
        try:
            write_evidence(arguments.evidence_output, report)
        except Exception as write_error:
            raise SystemExit(
                f"release install smoke failed: {error}; "
                f"could not retain secret-safe evidence: {write_error}"
            ) from error
        raise SystemExit(f"release install smoke failed: {error}") from error
    print(
        f"proved {len(report['runs'])} fresh packaged Solo Dogfood journey(s) "
        f"from {arguments.archive} in {report['duration_seconds']}s"
    )

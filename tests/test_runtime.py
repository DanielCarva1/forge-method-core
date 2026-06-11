import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "skills" / "forge-method" / "scripts" / "forge_method_runtime.py"


def run_cmd(
    *args: str,
    cwd: Path | None = None,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    result = subprocess.run(
        [sys.executable, str(RUNTIME), *args],
        cwd=str(cwd or ROOT),
        env=process_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode != 0:
        raise AssertionError(
            f"command failed: {args}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


class RuntimeTests(unittest.TestCase):
    def test_init_writes_durable_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            state = root / ".forge-method" / "state.yaml"
            sprint = root / ".forge-method" / "sprint.yaml"
            ledger = root / ".forge-method" / "ledger.ndjson"
            guidance = root / "AGENTS.md"
            verifier = root / ".codex" / "agents" / "forge-verifier.toml"

            self.assertTrue(state.exists())
            self.assertTrue(sprint.exists())
            self.assertTrue(ledger.exists())
            self.assertTrue(guidance.exists())
            self.assertTrue(verifier.exists())
            self.assertIn('runtime: "forge-method"', state.read_text(encoding="utf-8"))
            self.assertIn('"event": "project.initialized"', ledger.read_text(encoding="utf-8"))

    def test_start_routes_existing_project_from_child_directory(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            child = root / "src" / "feature"
            child.mkdir(parents=True)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            output = run_cmd("start", "--root", str(child)).stdout

            self.assertIn("Route: existing-method-project", output)
            self.assertIn("Project: Example Project", output)
            self.assertIn("Audit: passed", output)

    def test_start_lists_known_projects_without_initializing_parent(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            parent = Path(raw)
            project = parent / "client-project"
            project.mkdir()
            run_cmd("init", "--project", "Client Project", "--root", str(project))

            output = run_cmd("start", "--root", str(parent)).stdout

            self.assertIn("Project state: missing", output)
            self.assertIn("Known projects:", output)
            self.assertIn("Client Project", output)
            self.assertIn("Question: Which known project should be opened", output)
            self.assertFalse((parent / ".forge-method").exists())

    def test_preflight_resolves_project_identity_and_context_without_writing(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            child = root / "src" / "feature"
            child.mkdir(parents=True)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            text = run_cmd("preflight", "--root", str(child)).stdout
            payload = json.loads(run_cmd("preflight", "--root", str(child), "--json").stdout)
            selected_paths = [item["path"] for item in payload["context_load_plan"]["selected"]]

            self.assertIn("Forge Method Preflight", text)
            self.assertIn("Route: existing-method-project", text)
            self.assertIn("Read first:", text)
            self.assertIn("Resume:", text)
            self.assertEqual(payload["route"], "existing-method-project")
            self.assertEqual(payload["project_root"], str(root.resolve()))
            self.assertEqual(payload["status"]["project"], "Example Project")
            self.assertEqual(payload["status"]["resume"]["action"], "continue_current_workflow")
            self.assertIn(".forge-method/state.yaml", selected_paths)
            self.assertIn(".forge-method/sprint.yaml", selected_paths)
            self.assertFalse((root / ".forge-method" / "context" / "load-plan.json").exists())

    def test_preflight_lists_known_projects_and_requires_choice(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            parent = Path(raw)
            project = parent / "client-project"
            project.mkdir()
            run_cmd("init", "--project", "Client Project", "--root", str(project))

            text = run_cmd("preflight", "--root", str(parent), "--objective", "build a web product").stdout
            payload = json.loads(
                run_cmd("preflight", "--root", str(parent), "--objective", "build a web product", "--json").stdout
            )

            self.assertIn("Route: workspace-with-projects", text)
            self.assertIn("Question: Which existing project should be opened", text)
            self.assertEqual(payload["route"], "workspace-with-projects")
            self.assertTrue(payload["decision_required"])
            self.assertEqual(payload["known_projects"][0]["project"], "Client Project")
            self.assertEqual(payload["known_projects"][0]["path"], "client-project")
            self.assertEqual(payload["module_choices"][0]["id"], "software-builder")
            self.assertEqual(payload["decision"]["type"], "project-route")
            self.assertEqual(payload["decision"]["options"][0]["action"], "open_existing_project")
            self.assertEqual(payload["decision"]["options"][0]["project_path"], "client-project")
            self.assertEqual(payload["decision"]["options"][-1]["action"], "create_new_project")
            self.assertIn("Decision options:", text)
            self.assertFalse((parent / ".forge-method").exists())

    def test_preflight_detects_runtime_repo_without_project_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            manifest_dir = root / ".codex-plugin"
            manifest_dir.mkdir()
            (manifest_dir / "plugin.json").write_text(json.dumps({"name": "forge-method-core"}), encoding="utf-8")
            nested = root / "docs"
            nested.mkdir()
            example = root / "examples" / "sample"
            example.mkdir(parents=True)
            run_cmd("init", "--project", "Packaged Example", "--root", str(example))

            text = run_cmd("preflight", "--root", str(root)).stdout
            payload = json.loads(run_cmd("preflight", "--root", str(root), "--json").stdout)
            nested_payload = json.loads(run_cmd("preflight", "--root", str(nested), "--json").stdout)
            start = run_cmd("start", "--root", str(root)).stdout
            status = run_cmd("status", "--root", str(nested)).stdout

            self.assertIn("Route: runtime-repo", text)
            self.assertIn("outside the runtime repo", text)
            self.assertEqual(payload["route"], "runtime-repo")
            self.assertTrue(payload["runtime_repo"])
            self.assertEqual(payload["runtime_root"], str(root.resolve()))
            self.assertEqual(payload["known_projects"], [])
            self.assertEqual(payload["module_choices"][0]["id"], "software-builder")
            self.assertTrue(payload["decision_required"])
            self.assertIn("<parent-folder-outside-runtime-repo>", payload["commands"][0]["command"])
            self.assertEqual(payload["decision"]["options"][0]["action"], "choose_external_workspace")
            self.assertEqual(payload["decision"]["options"][1]["action"], "create_new_project")
            self.assertEqual(nested_payload["route"], "runtime-repo")
            self.assertEqual(nested_payload["runtime_root"], str(root.resolve()))
            self.assertIn("Known projects: not scanned inside runtime repo", start)
            self.assertIn(f"Runtime repo: {root.resolve()}", status)
            self.assertFalse((root / ".forge-method").exists())

    def test_preflight_empty_workspace_returns_create_decision(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)

            text = run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game").stdout
            payload = json.loads(
                run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game", "--json").stdout
            )

            self.assertEqual(payload["route"], "empty-workspace")
            self.assertTrue(payload["decision_required"])
            self.assertEqual(payload["decision"]["type"], "project-route")
            self.assertEqual(payload["decision"]["default_option"], "create-new-project")
            self.assertEqual(payload["decision"]["options"][0]["action"], "create_new_project")
            self.assertIn("project objective", payload["decision"]["options"][0]["requires"])
            self.assertIn("--objective", payload["decision"]["options"][0]["command"]["command"])
            self.assertIn("Decision options:", text)
            self.assertFalse((root / ".forge-method").exists())

    def test_invalid_phase_transition_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            result = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "4-build-verify",
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Invalid phase transition", result.stderr + result.stdout)

    def test_snapshot_reports_machine_readable_next_story(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd("transition", "--root", str(root), "--phase", "2-specification")
            run_cmd("transition", "--root", str(root), "--phase", "3-plan")
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(snapshot["runtime_version"], "1.22.0")
            self.assertEqual(snapshot["state"]["phase"], "4-build-verify")
            self.assertEqual(snapshot["stories"]["next"]["id"], "story-1")
            self.assertEqual(snapshot["route"]["recommendation"], "start_next_story")
            self.assertEqual(snapshot["resume"]["action"], "start_next_story")
            self.assertTrue(snapshot["resume"]["autonomous"])
            self.assertTrue(snapshot["quality"]["audit"]["passed"])

    def test_snapshot_does_not_start_story_before_build_phase(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(snapshot["state"]["phase"], "0-route")
            self.assertEqual(snapshot["stories"]["next"]["id"], "story-1")
            self.assertEqual(snapshot["route"]["recommendation"], "continue_current_workflow")

    def test_status_brief_surfaces_actionable_runtime_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("story", "start", "--root", str(root), "--id", "story-1")
            run_cmd("story", "review", "--root", str(root), "--id", "story-1")
            run_cmd(
                "review",
                "add",
                "--root",
                str(root),
                "--id",
                "status-review-proof",
                "--story",
                "story-1",
                "--title",
                "Status review proof",
                "--severity",
                "medium",
                "--summary",
                "Status should surface open review findings.",
            )

            brief = run_cmd("status", "--root", str(root), "--brief").stdout
            payload = json.loads(run_cmd("status", "--root", str(root), "--json").stdout)

            self.assertIn("Route: resolve_review_findings", brief)
            self.assertIn("Next story: story-1 [review] Build thing", brief)
            self.assertIn("Open review findings: 1", brief)
            self.assertIn("status-review-proof", brief)
            self.assertIn("Resume: resolve_review_findings", brief)
            self.assertEqual(payload["route"]["recommendation"], "resolve_review_findings")
            self.assertEqual(payload["resume"]["action"], "resolve_review_findings")
            self.assertTrue(payload["resume"]["autonomous"])
            self.assertEqual(payload["resume"]["target"]["id"], "status-review-proof")
            self.assertEqual(payload["stories"]["next"]["id"], "story-1")
            self.assertEqual(payload["open_review_findings"][0]["id"], "status-review-proof")

    def test_human_input_blocks_and_releases_runtime_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd(
                "input",
                "add",
                "--root",
                str(root),
                "--id",
                "target-user",
                "--prompt",
                "Who is the target user?",
                "--reason",
                "Discovery cannot choose the product route without an audience.",
            )

            blocked = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            next_text = run_cmd("next", "--root", str(root)).stdout
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            resume_text = run_cmd("resume", "--root", str(root)).stdout

            self.assertEqual(blocked["state"]["human_input_required"], "true")
            self.assertEqual(blocked["route"]["recommendation"], "wait_for_human_input")
            self.assertEqual(blocked["resume"]["action"], "answer_required_input")
            self.assertEqual(blocked["human_inputs"]["required_open"][0]["id"], "target-user")
            self.assertEqual(resume["action"], "answer_required_input")
            self.assertFalse(resume["autonomous"])
            self.assertEqual(resume["target"]["id"], "target-user")
            self.assertIn("input list", resume["next_command"])
            self.assertIn("Action: answer_required_input", resume_text)
            self.assertIn("answer human input target-user", next_text)
            self.assertIn("Audit passed.", run_cmd("audit", "--root", str(root)).stdout)

            run_cmd(
                "input",
                "answer",
                "--root",
                str(root),
                "--id",
                "target-user",
                "--answer",
                "Independent software founders",
                "--next-action",
                "continue discovery",
            )
            released = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(released["state"]["human_input_required"], "false")
            self.assertEqual(released["state"]["status"], "input-resolved")
            self.assertEqual(released["state"]["next_action"], "continue discovery")
            self.assertEqual(released["human_inputs"]["required_open"], [])
            answered = run_cmd("input", "list", "--root", str(root), "--status", "answered").stdout
            self.assertIn("target-user", answered)

    def test_story_block_routes_without_fake_human_input(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd("transition", "--root", str(root), "--phase", "2-specification")
            run_cmd("transition", "--root", str(root), "--phase", "3-plan")
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("story", "block", "--root", str(root), "--id", "story-1", "--reason", "dependency missing")

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(snapshot["state"]["human_input_required"], "false")
            self.assertEqual(snapshot["human_inputs"]["required_open"], [])
            self.assertEqual(snapshot["route"]["recommendation"], "resolve_story_blocker")
            self.assertIn("Audit passed.", run_cmd("audit", "--root", str(root)).stdout)

    def test_story_backlog_export_and_import_round_trip(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "source"
            target = Path(raw) / "target"
            root.mkdir()
            target.mkdir()
            run_cmd("init", "--project", "Source Project", "--root", str(root))
            run_cmd("init", "--project", "Target Project", "--root", str(target))
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-a",
                "--title",
                "Story A",
                "--acceptance",
                "A works",
            )
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-b",
                "--title",
                "Story B",
                "--acceptance",
                "B works",
                "--status",
                "planned",
            )

            exported = json.loads(run_cmd("story", "export", "--root", str(root)).stdout)
            export_path = root / ".forge-method" / "artifacts" / "backlog.json"
            out = run_cmd("story", "export", "--root", str(root), "--out", ".forge-method/artifacts/backlog.json").stdout
            target_import = target / "backlog.json"
            target_import.write_text(json.dumps(exported), encoding="utf-8")

            imported = run_cmd("story", "import", "--root", str(target), "--file", "backlog.json").stdout
            duplicate = run_cmd("story", "import", "--root", str(target), "--file", "backlog.json", check=False)
            target_snapshot = json.loads(run_cmd("snapshot", "--root", str(target)).stdout)

            self.assertEqual(exported["story_count"], 2)
            self.assertTrue(export_path.exists())
            self.assertIn(".forge-method/artifacts/backlog.json", out)
            self.assertIn("Stories imported: 2", imported)
            self.assertNotEqual(duplicate.returncode, 0)
            self.assertIn("Story already exists: story-a", duplicate.stderr + duplicate.stdout)
            self.assertEqual(target_snapshot["stories"]["counts"]["ready"], 1)
            self.assertEqual(target_snapshot["stories"]["counts"]["planned"], 1)
            self.assertIn("story-a", run_cmd("story", "list", "--root", str(target)).stdout)

    def test_done_story_requires_evidence_or_summary(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd("transition", "--root", str(root), "--phase", "2-specification")
            run_cmd("transition", "--root", str(root), "--phase", "3-plan")
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("story", "start", "--root", str(root), "--id", "story-1")

            result = run_cmd("story", "done", "--root", str(root), "--id", "story-1", check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Done stories require", result.stderr + result.stdout)

    def test_review_findings_block_done_until_resolved(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("story", "start", "--root", str(root), "--id", "story-1")
            run_cmd("story", "review", "--root", str(root), "--id", "story-1")
            run_cmd(
                "review",
                "add",
                "--root",
                str(root),
                "--id",
                "missing-proof",
                "--story",
                "story-1",
                "--title",
                "Missing proof",
                "--severity",
                "high",
                "--summary",
                "The review needs proof before completion.",
            )

            blocked = run_cmd(
                "story",
                "done",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--summary",
                "Done.",
                check=False,
            )
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            plan = json.loads(run_cmd("context", "plan", "--root", str(root), "--json").stdout)
            pack_path = Path(run_cmd("context", "pack", "--root", str(root)).stdout.strip())
            pack_text = pack_path.read_text(encoding="utf-8")

            self.assertNotEqual(blocked.returncode, 0)
            self.assertIn("Open review findings", blocked.stderr + blocked.stdout)
            self.assertEqual(snapshot["route"]["recommendation"], "resolve_review_findings")
            self.assertEqual(snapshot["review_findings"]["counts"]["open"], 1)
            self.assertEqual(snapshot["review_findings"]["open"][0]["id"], "missing-proof")
            self.assertIn(".forge-method/reviews/missing-proof.yaml", [item["path"] for item in plan["selected"]])
            self.assertIn("Open Review Findings", pack_text)
            self.assertIn("missing-proof", pack_text)

            run_cmd(
                "review",
                "resolve",
                "--root",
                str(root),
                "--id",
                "missing-proof",
                "--resolution",
                "Proof added through review.",
            )
            done = run_cmd("story", "done", "--root", str(root), "--id", "story-1", "--summary", "Done.")
            resolved = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertIn("Story done: story-1", done.stdout)
            self.assertEqual(resolved["review_findings"]["counts"]["open"], 0)
            self.assertEqual(resolved["review_findings"]["counts"]["resolved"], 1)
            self.assertIn("missing-proof", run_cmd("review", "list", "--root", str(root), "--status", "resolved").stdout)
            self.assertIn("Audit passed.", run_cmd("audit", "--root", str(root)).stdout)

    def test_ready_gate_writes_release_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("story", "start", "--root", str(root), "--id", "story-1")
            run_cmd("story", "done", "--root", str(root), "--id", "story-1", "--summary", "Done.")
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            resume_text = run_cmd("resume", "--root", str(root)).stdout

            self.assertEqual(resume["action"], "run_ready_gate")
            self.assertTrue(resume["autonomous"])
            self.assertIn("gate", resume["commands"][0]["name"])
            self.assertIn("ready", resume["commands"][1]["name"])
            self.assertIn("Action: run_ready_gate", resume_text)

            run_cmd("ready", "--root", str(root), "--summary", "Ready.")

            status = run_cmd("status", "--root", str(root)).stdout
            self.assertIn("Phase: 5-ready-operate", status)
            self.assertIn("Readiness: ready", status)

    def test_release_plan_suggests_version_without_publishing(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "VERSION").write_text("1.14.0\n", encoding="utf-8")

            batch = json.loads(
                run_cmd(
                    "release",
                    "plan",
                    "--root",
                    str(root),
                    "--mode",
                    "batch",
                    "--touches",
                    "runtime",
                    "--json",
                ).stdout
            )
            hotfix = json.loads(
                run_cmd(
                    "release",
                    "plan",
                    "--root",
                    str(root),
                    "--mode",
                    "hotfix",
                    "--current-version",
                    "1.14.0",
                    "--json",
                ).stdout
            )
            story = run_cmd(
                "release",
                "plan",
                "--root",
                str(root),
                "--mode",
                "story",
                "--touches",
                "docs",
            ).stdout

            self.assertEqual(batch["suggested_version"], "1.15.0")
            self.assertEqual(batch["validation"]["development"], "targeted-smoke")
            self.assertFalse(batch["publish"]["create_release"])
            self.assertEqual(hotfix["suggested_version"], "1.14.1")
            self.assertIn("Suggested version: 1.15.0", story)
            self.assertIn("Publish: no tag or release", story)

    def test_release_check_validates_local_release_readiness(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "VERSION").write_text("1.14.0\n", encoding="utf-8")
            (root / "CHANGELOG.md").write_text(
                "# Changelog\n\n## Unreleased\n\n- add useful release checks\n\n## 1.14.0\n",
                encoding="utf-8",
            )
            manifest_dir = root / ".codex-plugin"
            manifest_dir.mkdir()
            (manifest_dir / "plugin.json").write_text(
                json.dumps({"name": "forge-method-core", "version": "1.14.0"}),
                encoding="utf-8",
            )

            ready = json.loads(
                run_cmd(
                    "release",
                    "check",
                    "--root",
                    str(root),
                    "--mode",
                    "batch",
                    "--touches",
                    "docs",
                    "--json",
                ).stdout
            )
            (root / "CHANGELOG.md").write_text("# Changelog\n\n## Unreleased\n\n## 1.14.0\n", encoding="utf-8")
            blocked = run_cmd("release", "check", "--root", str(root), "--mode", "batch", check=False)

            self.assertTrue(ready["ready"])
            self.assertEqual(ready["suggested_version"], "1.15.0")
            self.assertFalse(ready["publish"]["create_release"])
            self.assertNotEqual(blocked.returncode, 0)
            self.assertIn("FAIL changelog_release_items", blocked.stdout)

    def test_doctor_reports_toolchain_and_validation_plan(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Doctor Project", "--root", str(root))

            payload = json.loads(
                run_cmd(
                    "doctor",
                    "--root",
                    str(root),
                    "--touches",
                    "runtime",
                    "--json",
                ).stdout
            )
            text = run_cmd("doctor", "--root", str(root), "--touches", "runtime").stdout

            self.assertEqual(payload["project_state_root"], str(root.resolve()))
            self.assertTrue(payload["audit"]["passed"])
            self.assertIn("toolchain", payload)
            self.assertIn("python", payload["toolchain"])
            self.assertEqual(payload["verification"]["validation"]["development"], "targeted-smoke")
            self.assertIn(
                "powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1",
                payload["verification"]["development_commands"]["windows"],
            )
            self.assertIn(
                "powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-all.ps1",
                payload["verification"]["release_commands"]["windows"],
            )
            self.assertIn("Forge Method Doctor", text)
            self.assertIn("Python current:", text)
            self.assertIn("Development validation: targeted-smoke", text)

    def test_doctor_reports_plugin_installation_status(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            home.mkdir()
            marketplace_path = home / ".agents" / "plugins" / "marketplace.json"
            plugin_root = home / "plugins" / "forge-method-core"
            manifest_path = plugin_root / ".codex-plugin" / "plugin.json"
            skill_path = plugin_root / "skills" / "forge-method" / "SKILL.md"
            marketplace_path.parent.mkdir(parents=True)
            manifest_path.parent.mkdir(parents=True)
            skill_path.parent.mkdir(parents=True)
            marketplace_path.write_text(
                json.dumps(
                    {
                        "name": "personal",
                        "plugins": [
                            {
                                "name": "forge-method-core",
                                "source": {"source": "local", "path": "./plugins/forge-method-core"},
                                "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                                "category": "Productivity",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            manifest_path.write_text(
                json.dumps({"name": "forge-method-core", "version": "1.22.0", "skills": "./skills/"}),
                encoding="utf-8",
            )
            skill_path.write_text("---\nname: forge-method\n---\n", encoding="utf-8")
            env = {"HOME": str(home), "USERPROFILE": str(home)}

            payload = json.loads(run_cmd("doctor", "--root", str(home), "--json", env=env).stdout)
            text = run_cmd("doctor", "--root", str(home), env=env).stdout

            plugin = payload["plugin_installation"]
            self.assertTrue(plugin["available"])
            self.assertEqual(plugin["status"], "ready")
            self.assertEqual(plugin["installed_version"], "1.22.0")
            self.assertEqual(plugin["plugin_path"], str(plugin_root.resolve()))
            self.assertIn("codex://plugins/forge-method-core?marketplacePath=", plugin["codex_deeplink"])
            self.assertIn("Plugin installation:", text)
            self.assertIn("Status: ready", text)
            self.assertIn("Open in Codex:", text)

    def test_artifact_is_indexed_and_added_to_context_pack(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Example spec",
                "--summary",
                "Artifact summary.",
            ).stdout.strip()
            run_cmd("context", "pack", "--root", str(root))

            index = root / ".forge-method" / "artifacts" / "index.ndjson"
            pack = root / ".forge-method" / "context" / "current-pack.md"

            self.assertTrue((root / artifact).exists())
            self.assertIn("Example spec", index.read_text(encoding="utf-8"))
            self.assertIn("Example spec", pack.read_text(encoding="utf-8"))
            self.assertIn("Artifact summary.", pack.read_text(encoding="utf-8"))

    def test_packaged_modules_and_workflows_validate(self) -> None:
        modules = run_cmd("module", "list").stdout
        modules_json = json.loads(run_cmd("module", "list", "--json").stdout)
        module_recommendation = json.loads(
            run_cmd("module", "recommend", "--objective", "build a web app with an API", "--json").stdout
        )
        validation = run_cmd("workflow", "validate").stdout
        version = run_cmd("version").stdout

        self.assertIn("core-runtime", modules)
        self.assertIn("software-builder", modules)
        self.assertTrue(modules_json["modules"])
        self.assertEqual(module_recommendation["recommended"][0]["id"], "software-builder")
        self.assertIn("Workflow validation passed.", validation)
        agents = run_cmd("agent", "list").stdout
        agent_validation = run_cmd("agent", "validate").stdout

        self.assertIn("facilitator", agents)
        self.assertIn("quality-reviewer", agents)
        self.assertIn("Agent profile validation passed.", agent_validation)
        self.assertEqual(version.strip(), "1.22.0")

    def test_context_plan_selects_relevant_files_and_updates_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan"]:
                run_cmd("transition", "--root", str(root), "--phase", phase)
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify", "--workflow", "build-story")
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Example spec",
                "--summary",
                "Artifact summary.",
                "--path",
                ".forge-method/artifacts/spec.md",
            ).stdout.strip()
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )
            run_cmd("artifact", "link-story", "--root", str(root), "--path", artifact, "--story", "story-1")
            run_cmd("story", "start", "--root", str(root), "--id", "story-1")

            plan_path = Path(run_cmd("context", "plan", "--root", str(root), "--max-chars", "1200").stdout.strip())
            plan = json.loads(plan_path.read_text(encoding="utf-8"))
            selected_paths = [item["path"] for item in plan["selected"]]
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(plan["runtime_version"], "1.22.0")
            self.assertEqual(plan["state"]["phase"], "4-build-verify")
            self.assertIn(".forge-method/state.yaml", selected_paths)
            self.assertIn(".forge-method/sprint.yaml", selected_paths)
            self.assertIn(".forge-method/stories/story-1.yaml", selected_paths)
            self.assertIn("skill:references/workflow-build-story.md", selected_paths)
            self.assertIn(".forge-method/context/load-plan.json", snapshot["context"]["load_plan"])
            self.assertLessEqual(plan["estimated_selected_chars"], plan["budget_chars"] + plan["estimated_required_chars"])

    def test_agent_recommendations_follow_runtime_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            route_recommendation = json.loads(
                run_cmd("agent", "recommend", "--root", str(root), "--json").stdout
            )
            self.assertEqual(route_recommendation["recommended"][0]["id"], "facilitator")

            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd("transition", "--root", str(root), "--phase", "2-specification")
            run_cmd("transition", "--root", str(root), "--phase", "3-plan")
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )

            build_recommendation = json.loads(
                run_cmd("agent", "recommend", "--root", str(root), "--json").stdout
            )
            build_ids = [item["id"] for item in build_recommendation["recommended"]]
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            snapshot_ids = [item["id"] for item in snapshot["agents"]["recommended"]]
            pack = run_cmd("context", "pack", "--root", str(root)).stdout.strip()

            self.assertIn("implementer", build_ids)
            self.assertIn("quality-reviewer", build_ids)
            self.assertEqual(snapshot_ids, build_ids)
            self.assertIn("current-pack.md", pack)
            self.assertIn(
                "Recommended Agent Profiles",
                (root / ".forge-method" / "context" / "current-pack.md").read_text(encoding="utf-8"),
            )

    def test_example_list_and_create_seed_project(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw) / "software-example"
            examples = run_cmd("example", "list").stdout
            create = run_cmd(
                "example",
                "create",
                "--root",
                str(root),
                "--module",
                "software-builder",
            ).stdout

            state = root / ".forge-method" / "state.yaml"
            story = root / ".forge-method" / "stories" / "example-start.yaml"
            artifact = root / ".forge-method" / "artifacts" / "example-brief.md"
            context_pack = root / ".forge-method" / "context" / "current-pack.md"
            readme = root / "README.md"
            gate = run_cmd("gate", "--root", str(root), "--require-evals").stdout

            self.assertIn("software-builder", examples)
            self.assertIn("Example created:", create)
            self.assertIn('module: "software-builder"', state.read_text(encoding="utf-8"))
            self.assertIn('status: "ready"', story.read_text(encoding="utf-8"))
            self.assertTrue(artifact.exists())
            self.assertTrue(context_pack.exists())
            self.assertIn("software-builder", readme.read_text(encoding="utf-8"))
            self.assertIn("Gate passed.", gate)
            self.assertIn("Evals: 1/1 passed", gate)

    def test_project_create_seeds_real_module_project(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            parent = Path(raw)
            create = run_cmd(
                "project",
                "create",
                "--root",
                str(parent),
                "--name",
                "Night Watch",
                "--module",
                "software-builder",
                "--objective",
                "Create a monitoring product.",
            ).stdout
            root = parent / "night-watch"
            state = root / ".forge-method" / "state.yaml"
            story = root / ".forge-method" / "stories" / "project-kickoff.yaml"
            artifact = root / ".forge-method" / "artifacts" / "project-brief.md"
            load_plan = root / ".forge-method" / "context" / "load-plan.json"
            project_list = run_cmd("project", "list", "--root", str(parent)).stdout
            gate = run_cmd("gate", "--root", str(root), "--require-evals").stdout

            self.assertIn("Project created: Night Watch", create)
            self.assertTrue(state.exists())
            self.assertTrue(story.exists())
            self.assertTrue(artifact.exists())
            self.assertTrue(load_plan.exists())
            self.assertIn('phase: "1-discovery"', state.read_text(encoding="utf-8"))
            self.assertIn('active_workflow: "discover-intent"', state.read_text(encoding="utf-8"))
            self.assertIn("software-builder", artifact.read_text(encoding="utf-8"))
            self.assertIn("night-watch", project_list)
            self.assertIn("Gate passed.", gate)
            self.assertIn("Evals: 1/1 passed", gate)

    def test_project_create_auto_selects_module_from_objective(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            parent = Path(raw)
            run_cmd(
                "project",
                "create",
                "--root",
                str(parent),
                "--name",
                "Game Idea",
                "--module",
                "auto",
                "--objective",
                "prototype a game with play loops",
            )
            root = parent / "game-idea"
            state = (root / ".forge-method" / "state.yaml").read_text(encoding="utf-8")

            self.assertIn('module: "game-studio"', state)

    def test_workflow_module_and_eval_generation(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            workflow = run_cmd(
                "workflow",
                "create",
                "--root",
                str(root),
                "--id",
                "market-scan",
                "--title",
                "Market Scan",
                "--trigger",
                "state.module == research",
                "--input",
                "research question",
                "--step",
                "collect current evidence",
                "--output",
                "research artifact",
                "--done",
                "artifact exists",
                "--blocked",
                "source access unavailable",
                "--handoff",
                "preserve sources and recommendation",
                "--eval-query",
                "research current market",
            ).stdout.strip()
            module = run_cmd(
                "module",
                "create",
                "--root",
                str(root),
                "--id",
                "research",
                "--title",
                "Research",
                "--purpose",
                "Turn questions into current evidence.",
                "--phase-span",
                "1-discovery",
                "--workflow",
                "market-scan",
            ).stdout.strip()

            self.assertTrue((root / workflow).exists())
            self.assertTrue((root / module).exists())
            self.assertIn("market-scan", run_cmd("workflow", "list", "--root", str(root)).stdout)
            self.assertIn("research", run_cmd("module", "list", "--root", str(root)).stdout)
            self.assertIn("Workflow validation passed.", run_cmd("workflow", "validate", "--root", str(root)).stdout)
            eval_output = run_cmd("eval", "run", "--root", str(root)).stdout
            self.assertIn("PASS market-scan-routing", eval_output)
            self.assertIn("PASS market-scan-trigger", eval_output)

    def test_artifact_story_link_is_audited(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Linked spec",
                "--summary",
                "Linked summary.",
            ).stdout.strip()
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Use linked spec",
                "--acceptance",
                "spec is linked",
            )
            run_cmd("artifact", "link-story", "--root", str(root), "--path", artifact, "--story", "story-1")
            story_file = root / ".forge-method" / "stories" / "story-1.yaml"

            self.assertIn(artifact, story_file.read_text(encoding="utf-8"))
            self.assertIn("Audit passed.", run_cmd("audit", "--root", str(root)).stdout)

    def test_missing_active_artifact_fails_verification_and_audit(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "plan",
                "--title",
                "Active plan",
                "--summary",
                "This active artifact must remain available.",
            ).stdout.strip()
            (root / artifact).unlink()

            verify = run_cmd("artifact", "verify", "--root", str(root), check=False)
            audit = run_cmd("audit", "--root", str(root), check=False)
            gate = run_cmd("gate", "--root", str(root), check=False)

            self.assertNotEqual(verify.returncode, 0)
            self.assertIn("missing active artifact", verify.stdout)
            self.assertNotEqual(audit.returncode, 0)
            self.assertIn("missing active artifact", audit.stdout)
            self.assertNotEqual(gate.returncode, 0)
            self.assertIn("Gate failed:", gate.stdout)

    def test_ephemeral_artifact_can_be_captured_and_deleted(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Use ephemeral plan",
                "--acceptance",
                "captured result is enough to resume",
            )
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "task",
                "--title",
                "Temporary agent task",
                "--summary",
                "Do this once, then capture the result.",
                "--lifecycle",
                "ephemeral",
                "--story",
                "story-1",
                "--eval",
            ).stdout.strip()
            self.assertIn("PASS artifact-", run_cmd("eval", "run", "--root", str(root)).stdout)
            capture = run_cmd(
                "artifact",
                "capture",
                "--root",
                str(root),
                "--path",
                artifact,
                "--story",
                "story-1",
                "--summary",
                "The temporary task was captured into story state.",
                "--delete",
            ).stdout
            pack = run_cmd("context", "pack", "--root", str(root)).stdout.strip()

            self.assertIn("Captured:", capture)
            self.assertFalse((root / artifact).exists())
            self.assertIn("Artifact verification passed.", run_cmd("artifact", "verify", "--root", str(root)).stdout)
            self.assertIn("Audit passed.", run_cmd("audit", "--root", str(root)).stdout)
            self.assertIn("captured/ephemeral", (Path(pack)).read_text(encoding="utf-8"))

    def test_gate_requires_evals_and_writes_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            missing = run_cmd("gate", "--root", str(root), "--require-evals", check=False)
            self.assertNotEqual(missing.returncode, 0)
            self.assertIn("eval: no evals configured", missing.stdout)

            run_cmd(
                "workflow",
                "create",
                "--root",
                str(root),
                "--id",
                "gate-flow",
                "--title",
                "Gate Flow",
                "--trigger",
                "quality gate requested",
                "--input",
                "project state",
                "--step",
                "run objective checks",
                "--output",
                "gate evidence",
                "--done",
                "gate passes",
                "--blocked",
                "required checks fail",
                "--handoff",
                "preserve failures and next action",
                "--eval-query",
                "run quality gate",
            )
            passed = run_cmd(
                "gate",
                "--root",
                str(root),
                "--require-evals",
                "--summary",
                "Quality gate passed.",
                "--context-pack",
            ).stdout
            evidence_files = list((root / ".forge-method" / "evidence").glob("*gate*.md"))

            self.assertIn("Gate passed.", passed)
            self.assertIn("Evals: 2/2 passed", passed)
            self.assertIn("Evidence:", passed)
            self.assertTrue(evidence_files)
            self.assertTrue((root / ".forge-method" / "context" / "current-pack.md").exists())

    def test_context_pack_respects_max_chars(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Large spec",
                "--summary",
                "x" * 1000,
            )
            run_cmd("context", "pack", "--root", str(root), "--max-chars", "400")
            pack = root / ".forge-method" / "context" / "current-pack.md"

            self.assertLessEqual(len(pack.read_text(encoding="utf-8")), 400)

    def test_checkpoint_updates_memory_and_context_pack(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "plan",
                "--title",
                "Recovery plan",
                "--summary",
                "Use checkpoint memory before reading old chat.",
            ).stdout.strip()
            checkpoint = run_cmd(
                "checkpoint",
                "--root",
                str(root),
                "--title",
                "After routing work",
                "--summary",
                "Start route is implemented and verified.",
                "--decision",
                "Use durable checkpoints instead of conversation replay.",
                "--check",
                "unit tests passed",
                "--failed-check",
                "none",
                "--touched",
                "skills/forge-method/scripts/forge_method_runtime.py",
                "--artifact",
                artifact,
                "--next-action",
                "continue with context memory hardening",
            ).stdout.strip()
            latest = root / ".forge-method" / "context" / "latest-checkpoint.md"
            pack = root / ".forge-method" / "context" / "current-pack.md"
            status = run_cmd("status", "--root", str(root)).stdout

            self.assertTrue((root / checkpoint).exists())
            self.assertTrue(latest.exists())
            self.assertTrue(pack.exists())
            self.assertIn("Use durable checkpoints", latest.read_text(encoding="utf-8"))
            self.assertIn("Latest Checkpoint", pack.read_text(encoding="utf-8"))
            self.assertIn("Use checkpoint memory before reading old chat.", pack.read_text(encoding="utf-8"))
            self.assertIn("continue with context memory hardening", status)

    def test_context_recover_writes_resume_brief_with_failure_signals(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "checkpoint",
                "--root",
                str(root),
                "--title",
                "After failed check",
                "--summary",
                "A check failed and must be visible after context reset.",
                "--failed-check",
                "unit test failed: expected route",
                "--touched",
                "skills/forge-method/scripts/forge_method_runtime.py",
                "--next-action",
                "fix failed route check",
            )
            recovery = run_cmd("context", "recover", "--root", str(root)).stdout.strip()
            pack = root / ".forge-method" / "context" / "current-pack.md"
            recovery_text = Path(recovery).read_text(encoding="utf-8")
            pack_text = pack.read_text(encoding="utf-8")

            self.assertIn("unit test failed: expected route", recovery_text)
            self.assertIn("skills/forge-method/scripts/forge_method_runtime.py", recovery_text)
            self.assertIn("Resume Commands", recovery_text)
            self.assertIn("Recovery Signals", pack_text)
            self.assertIn("unit test failed: expected route", pack_text)

    def test_compact_context_recover_preserves_resume_under_budget(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-1",
                "--title",
                "Build thing",
                "--acceptance",
                "thing works",
            )

            recovery = run_cmd("context", "recover", "--root", str(root), "--compact", "--max-chars", "1400").stdout.strip()
            recovery_path = Path(recovery)
            text = recovery_path.read_text(encoding="utf-8")
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(recovery_path.name, "recovery-compact.md")
            self.assertLessEqual(len(text), 1400)
            self.assertIn("# Forge Method Compact Recovery", text)
            self.assertIn("## State", text)
            self.assertIn("## Resume", text)
            self.assertIn("- action: start_next_story", text)
            self.assertIn("## Read First", text)
            self.assertIn("## Commands", text)
            self.assertIn("story start", text)
            self.assertEqual(snapshot["context"]["compact_recovery"], ".forge-method/context/recovery-compact.md")

    def test_context_health_is_read_only_when_budget_is_clear(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            health = json.loads(
                run_cmd("context", "health", "--root", str(root), "--max-chars", "100000", "--json").stdout
            )
            text = run_cmd("context", "health", "--root", str(root), "--max-chars", "100000").stdout

            self.assertEqual(health["level"], "ok")
            self.assertFalse(health["over_budget"])
            self.assertEqual(health["commands"][0]["name"], "context-plan")
            self.assertIn("Context health: ok", text)
            self.assertFalse((root / ".forge-method" / "context" / "load-plan.json").exists())

    def test_context_health_blocks_when_required_context_exceeds_budget(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            health = json.loads(run_cmd("context", "health", "--root", str(root), "--max-chars", "10", "--json").stdout)
            preflight = json.loads(run_cmd("preflight", "--root", str(root), "--max-chars", "10", "--json").stdout)

            self.assertEqual(health["level"], "blocked")
            self.assertTrue(health["over_budget"])
            self.assertIn("compact-recovery", [command["name"] for command in health["commands"]])
            self.assertEqual(preflight["context_health"]["level"], "blocked")
            self.assertIn("context-health", [command["name"] for command in preflight["commands"]])


if __name__ == "__main__":
    unittest.main()

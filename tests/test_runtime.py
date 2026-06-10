import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "skills" / "forge-method" / "scripts" / "forge_method_runtime.py"


def run_cmd(*args: str, cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        [sys.executable, str(RUNTIME), *args],
        cwd=str(cwd or ROOT),
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
            run_cmd("ready", "--root", str(root), "--summary", "Ready.")

            status = run_cmd("status", "--root", str(root)).stdout
            self.assertIn("Phase: 5-ready-operate", status)
            self.assertIn("Readiness: ready", status)

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

    def test_packaged_modules_and_workflows_validate(self) -> None:
        modules = run_cmd("module", "list").stdout
        validation = run_cmd("workflow", "validate").stdout
        version = run_cmd("version").stdout

        self.assertIn("core-runtime", modules)
        self.assertIn("software-builder", modules)
        self.assertIn("Workflow validation passed.", validation)
        self.assertEqual(version.strip(), "1.2.0")

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
            self.assertIn("PASS market-scan-routing", run_cmd("eval", "run", "--root", str(root)).stdout)

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


if __name__ == "__main__":
    unittest.main()

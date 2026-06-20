import json
import os
import hashlib
import importlib.util
import contextlib
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "skills" / "forge-method" / "scripts" / "forge_method_runtime.py"
CURRENT_VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
GUIDANCE_FIXTURES = ROOT / "skills" / "forge-method" / "fixtures" / "guidance-parity-replay.json"
GUIDANCE_BENCHMARK = ROOT / ".forge-method" / "artifacts" / "guidance-engine-benchmark.md"
WORKFLOW_CATALOG = ROOT / "skills" / "forge-method" / "catalog" / "workflows.json"
PARITY_REQUIRED_FAMILIES = {
    "help",
    "confusion",
    "brainstorm",
    "research",
    "spec",
    "prd",
    "ux",
    "visual",
    "architecture",
    "quick-dev",
    "story-cycle",
    "correct-course",
    "builder",
    "config",
    "persona",
    "cis",
    "enterprise",
    "platform",
    "collaboration",
    "game",
    "tea",
    "lifecycle",
    "document-utility",
}


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


def add_decision_source(root: Path, *, title: str = "Approved spec") -> str:
    return run_cmd(
        "artifact",
        "add",
        "--root",
        str(root),
        "--kind",
        "spec",
        "--title",
        title,
        "--summary",
        "Approved decision source for implementation-ready story tests.",
        "--path",
        ".forge-method/artifacts/test-decision-source.md",
    ).stdout.strip()


def load_runtime_module():
    spec = importlib.util.spec_from_file_location("forge_runtime_under_test", RUNTIME)
    if not spec or not spec.loader:
        raise AssertionError("Could not load Forge runtime module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ledger_events(root: Path, event: str) -> list[dict[str, object]]:
    ledger = root / ".forge-method" / "ledger.ndjson"
    if not ledger.exists():
        return []
    events = []
    for line in ledger.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        item = json.loads(line)
        if item.get("event") == event:
            events.append(item)
    return events


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare_guidance_fixture(root: Path, state_kind: str) -> None:
    if state_kind == "none":
        return
    run_cmd("init", "--project", "Guidance Fixture", "--root", str(root))
    if state_kind == "discovery":
        run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
        return
    if state_kind == "ready":
        run_cmd(
            "transition",
            "--root",
            str(root),
            "--phase",
            "5-ready-operate",
            "--status",
            "story-done",
            "--workflow",
            "ready-release",
            "--next-action",
            "publish current batch",
            "--force",
        )
        return
    if state_kind == "evolve_runtime":
        run_cmd(
            "transition",
            "--root",
            str(root),
            "--phase",
            "6-evolve",
            "--status",
            "evolution-intake",
            "--workflow",
            "evolve-project",
            "--next-action",
            "compare and implement guided-flow parity gaps",
            "--force",
        )
        return
    if state_kind == "build_story_ready":
        for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
            run_cmd("transition", "--root", str(root), "--phase", phase)
        add_decision_source(root)
        run_cmd(
            "story",
            "add",
            "--root",
            str(root),
            "--id",
            "story-guidance",
            "--title",
            "Build guidance target",
            "--acceptance",
            "target works",
        )
        return
    raise AssertionError(f"unknown guidance fixture state: {state_kind}")


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

            self.assertIn("Forge setup: choose an existing project or start a new one", output)
            self.assertIn("Known projects:", output)
            self.assertIn("Client Project", output)
            self.assertIn("Next question: Which known project should be opened", output)
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

    def test_reload_reports_bootstrap_contract_without_writing_context(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            text = run_cmd("reload", "--root", str(root)).stdout
            payload = json.loads(run_cmd("reload", "--root", str(root), "--json").stdout)

            self.assertIn("Forge Reload", text)
            self.assertIn("Contract: current filesystem and launcher output override prior Forge chat state.", text)
            self.assertIn("Next: run resume --json", text)
            self.assertEqual(payload["route"], "existing-method-project")
            self.assertEqual(payload["project_root"], str(root.resolve()))
            self.assertTrue(payload["bootstrap_contract"]["do_not_replay_chat_state"])
            self.assertIn("Fresh chat boundary:", text)
            self.assertEqual(payload["context_boundary"]["mode"], "resume-first")
            self.assertEqual(payload["context_boundary"]["current_workflow"], "start-runtime")
            self.assertIn(".forge-method/state.yaml", payload["context_boundary"]["read_first"])
            self.assertIn("resume", [command["name"] for command in payload["commands"]])
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
            self.assertIn("Next question: Which existing project should be opened", text)
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
            maintainer_dir = root / ".forge-method"
            maintainer_dir.mkdir()
            (maintainer_dir / "core-dev.local").write_text("", encoding="utf-8")
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
            self.assertFalse((root / ".forge-method" / "state.yaml").exists())

            blocked = run_cmd(
                "project",
                "create",
                "--root",
                str(root),
                "--path",
                str(root),
                "--name",
                "Forge Method Core",
                "--module",
                "runtime-builder",
                "--objective",
                "Improve the runtime itself",
                "--brownfield",
                check=False,
            )
            self.assertNotEqual(blocked.returncode, 0)
            self.assertIn("--allow-runtime-state", blocked.stderr)

            allowed = run_cmd(
                "project",
                "create",
                "--root",
                str(root),
                "--path",
                str(root),
                "--name",
                "Forge Method Core",
                "--module",
                "runtime-builder",
                "--objective",
                "Improve the runtime itself",
                "--brownfield",
                "--allow-runtime-state",
            )
            self.assertIn("Project type: brownfield", allowed.stdout)
            self.assertTrue((root / ".forge-method" / "state.yaml").exists())

    def test_installed_runtime_package_hides_core_state_for_public_users(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            manifest_dir = root / ".codex-plugin"
            state_dir = root / ".forge-method"
            manifest_dir.mkdir()
            state_dir.mkdir()
            (manifest_dir / "plugin.json").write_text(
                json.dumps({"name": "forge-method-core", "version": CURRENT_VERSION}),
                encoding="utf-8-sig",
            )
            (state_dir / "state.yaml").write_text(
                "\n".join(
                    [
                        'runtime: "forge-method"',
                        f'runtime_version: "{CURRENT_VERSION}"',
                        'project: "forge-method-core"',
                        'phase: "5-ready-operate"',
                        'status: "published"',
                        'active_workflow: "operate-support"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            text = run_cmd("preflight", "--root", str(root)).stdout
            payload = json.loads(run_cmd("preflight", "--root", str(root), "--json").stdout)
            start = run_cmd("start", "--root", str(root)).stdout
            status = run_cmd("status", "--root", str(root)).stdout

            self.assertIn("Route: installed-runtime-package", text)
            self.assertIn("installed Forge package", text)
            self.assertNotIn("Continue current project", text)
            self.assertEqual(payload["route"], "installed-runtime-package")
            self.assertFalse(payload["runtime_repo"])
            self.assertTrue(payload["installed_runtime_package"])
            self.assertEqual(payload["project_state"], "missing")
            self.assertEqual(payload["known_projects"], [])
            self.assertEqual(payload["decision"]["options"][0]["action"], "choose_external_workspace")
            self.assertNotIn("continue_current_project", json.dumps(payload))
            self.assertIn("Known projects: not scanned inside installed Forge package", start)
            self.assertIn("Installed Forge package:", status)

            blocked = run_cmd(
                "project",
                "create",
                "--root",
                str(root),
                "--path",
                str(root),
                "--name",
                "Accidental Core Edit",
                "--module",
                "runtime-builder",
                "--objective",
                "edit Forge core by accident",
                "--brownfield",
                "--allow-runtime-state",
                check=False,
            )
            self.assertNotEqual(blocked.returncode, 0)
            self.assertIn("Maintainers must set FORGE_METHOD_CORE_DEV=1", blocked.stderr)

    def test_preflight_empty_workspace_returns_create_decision(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)

            text = run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game").stdout
            payload = json.loads(
                run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game", "--json").stdout
            )
            start = run_cmd("start", "--root", str(root)).stdout
            guide = run_cmd("guide", "--root", str(root), "--question", "build a mobile game").stdout
            reload = run_cmd("reload", "--root", str(root)).stdout

            self.assertEqual(payload["route"], "empty-workspace")
            self.assertTrue(payload["decision_required"])
            self.assertEqual(payload["decision"]["type"], "project-route")
            self.assertEqual(payload["decision"]["default_option"], "create-new-project")
            self.assertEqual(payload["decision"]["options"][0]["action"], "create_new_project")
            self.assertIn("project objective", payload["decision"]["options"][0]["requires"])
            self.assertIn("--objective", payload["decision"]["options"][0]["command"]["command"])
            self.assertEqual(
                payload["human_experience"]["prompt"],
                "Me manda um nome e um objetivo em linguagem normal. Eu transformo isso em estado, trilha e próximos passos.",
            )
            self.assertEqual(payload["reality_evidence_gate"]["status"], "needs-evidence")
            self.assertIn("Forge Method pega uma ideia", text)
            self.assertIn("Bora começar direito", start)
            self.assertIn("Me manda um nome e um objetivo", start)
            self.assertNotIn("Welcome the human", start)
            self.assertNotIn("Project state: missing", start)
            self.assertIn("Forge setup: ready to create the first Forge project here", start)
            self.assertLess(start.index("Bora começar direito"), start.index("Forge setup: ready"))
            self.assertIn("Reality/Evidence Gate: needs-evidence", guide)
            self.assertIn("Decision options:", text)
            self.assertIn("Next question: Create a new method project in this workspace?", reload)
            self.assertFalse((root / ".forge-method").exists())

    def test_preflight_existing_codebase_returns_brownfield_decision(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "src").mkdir()
            (root / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")

            text = run_cmd("preflight", "--root", str(root), "--objective", "continue existing app").stdout
            payload = json.loads(
                run_cmd("preflight", "--root", str(root), "--objective", "continue existing app", "--json").stdout
            )
            start = run_cmd("start", "--root", str(root)).stdout

            self.assertEqual(payload["route"], "existing-codebase")
            self.assertTrue(payload["decision_required"])
            self.assertEqual(payload["decision"]["default_option"], "initialize-brownfield-project")
            self.assertEqual(payload["decision"]["options"][0]["action"], "initialize_brownfield_project")
            self.assertIn("--brownfield", payload["decision"]["options"][0]["command"]["command"])
            self.assertEqual(
                payload["human_experience"]["route_summary"],
                "Achei código aqui, mas ainda não achei estado Forge. Isso parece brownfield: primeiro entendo o que já existe, depois mexo.",
            )
            self.assertIn("Initialize Forge Method for this existing project as brownfield", text)
            self.assertIn("Isso parece brownfield", start)
            self.assertNotIn("This looks like brownfield work", start)
            self.assertNotIn("Project state: missing", start)
            self.assertIn("Forge setup: ready for brownfield discovery", start)
            self.assertLess(start.index("Isso parece brownfield"), start.index("Forge setup: ready"))
            self.assertIn("Route: existing-codebase", start)
            self.assertFalse((root / ".forge-method").exists())

    def test_reality_evidence_gate_blocks_impossible_and_cruel_ideas(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            dog_question = "Build a product that turns my dog into a delegate that gives speeches."
            cat_question = "Build a tower that sprays water on a cat when it jumps on tables."

            dog_text = run_cmd("guide", "--root", str(root), "--question", dog_question).stdout
            dog_payload = runtime.build_guide_payload(root, question=dog_question, max_chars=12000)
            cat_payload = runtime.build_guide_payload(root, question=cat_question, max_chars=12000)

            self.assertEqual(dog_payload["reality_evidence_gate"]["status"], "blocked")
            self.assertEqual(dog_payload["reality_evidence_gate"]["score"], 0)
            self.assertIn("Reality/Evidence Gate: blocked (0/10)", dog_text)
            self.assertIn("Physical or biological impossibility", dog_text)
            self.assertEqual(cat_payload["reality_evidence_gate"]["status"], "blocked")
            self.assertEqual(cat_payload["reality_evidence_gate"]["score"], 0)
            self.assertIn("Animal-welfare", cat_payload["reality_evidence_gate"]["summary"])

    def test_guidance_engine_routes_transcript_fixtures(self) -> None:
        runtime = load_runtime_module()
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        required_keys = {
            "intent_classification",
            "signals",
            "recommended_phase",
            "recommended_workflow",
            "recommended_action",
            "human_prompt",
            "alternatives",
            "state_update_required",
            "commands",
            "workflow_metadata",
            "facilitation_pack",
            "persona_lens",
        }
        payload_required_keys = required_keys | {
            "council_recommended",
            "codex_goal_handoff",
            "mechanical_work_order",
        }
        replay = runtime.run_parity_replay(fixture_path=GUIDANCE_FIXTURES, max_chars=12000)

        self.assertEqual(replay["failed"], 0)
        self.assertEqual(replay["missing_families"], [])
        self.assertEqual(replay["passed"], replay["total"])

        method_case = next(case for case in fixtures if case["id"] == "method_frustration_ready")
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, method_case["state"])
            payload = runtime.build_guide_payload(root, question=method_case["question"], max_chars=12000)

        self.assertTrue(payload_required_keys <= payload.keys())
        self.assertTrue(required_keys <= payload["guidance_engine"].keys())
        self.assertEqual([], runtime.parity_case_failures(method_case, payload))
        self.assertNotIn("publish current batch", payload["recommended_action"])
        command_names = [item["name"] for item in payload["commands"]]
        self.assertIn("transition-evolve", command_names)
        self.assertIn("correct-course", command_names)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            runtime.print_guidance_engine_summary(payload)
        text = output.getvalue()
        self.assertIn("Guidance Engine: correct-course -> correct-course / 6-evolve", text)
        self.assertIn("State update: required", text)

    def test_guidance_engine_benchmark_artifact_covers_fixture_targets(self) -> None:
        text = GUIDANCE_BENCHMARK.read_text(encoding="utf-8")
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))

        self.assertIn("Forge parity targets", text)
        self.assertIn("Correct-course", text)
        self.assertIn("Broad ideas", text)
        self.assertIn("Confusion", text)
        self.assertIn("Mechanical build", text)
        for workflow in {case["expected_workflow"] for case in fixtures}:
            self.assertIn(workflow, text)

    def test_guidance_human_lede_and_runtime_builder_contract(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "evolve_runtime")

            polish_question = "quero melhorar a experiencia humana e compactar os docs agenticos"
            polish = runtime.build_guide_payload(root, question=polish_question, max_chars=12000)
            polish_text = run_cmd("guide", "--root", str(root), "--question", polish_question).stdout

            self.assertEqual(polish["intent_classification"], "builder-flow")
            self.assertEqual(polish["recommended_workflow"], "runtime-builder")
            self.assertEqual(polish["facilitation_pack"], "skill:facilitation/runtime-builder.md")
            self.assertEqual(polish["reality_evidence_gate"]["status"], "not-applicable")
            human = polish["human_experience"]
            for key in [
                "decision_summary",
                "next_move",
                "human_question",
                "guardrail",
                "compact_contract",
                "contract_split",
            ]:
                self.assertIn(key, human)
            self.assertIn("Isto e trabalho no motor do Forge", polish_text)
            self.assertIn("A conversa pode ser rica", polish_text)
            self.assertIn("Guidance: Let's use `runtime-builder` as the guided path.", polish_text)
            self.assertIn(
                "First question: what human behavior should improve, what compact agent contract should exist, and which test would catch regression?",
                polish_text,
            )
            self.assertNotIn("Prompt: Let's use `runtime-builder`", polish_text)
            self.assertLess(polish_text.index("Isto e trabalho no motor do Forge"), polish_text.index("Workspace:"))
            self.assertNotIn("Reality/Evidence Gate", polish_text)
            self.assertLess(len(json.dumps(human, sort_keys=True)), 1800)

            failed_guidance_question = (
                "bmad funciona com skills e md, mas o Forge ainda esta com experiencia humana ruim, "
                "nao facilita, nao faz alinhamento humano, nao pergunta qual problema resolver nem como a pessoa quer se sentir; isso foi mentira?"
            )
            failed_guidance = runtime.build_guide_payload(root, question=failed_guidance_question, max_chars=12000)

            self.assertEqual(failed_guidance["intent_classification"], "correct-course")
            self.assertEqual(failed_guidance["recommended_workflow"], "correct-course")
            self.assertEqual(failed_guidance["facilitation_pack"], "skill:facilitation/correct-course.md")
            self.assertTrue(failed_guidance["state_update_required"])

            frustration_question = "estou frustrado, nao sei se o Forge esta guiando de verdade"
            frustration = runtime.build_guide_payload(root, question=frustration_question, max_chars=12000)
            frustration_text = run_cmd("guide", "--root", str(root), "--question", frustration_question).stdout

            self.assertEqual(frustration["intent_classification"], "correct-course")
            self.assertEqual(frustration["recommended_workflow"], "correct-course")
            self.assertEqual(frustration["reality_evidence_gate"]["status"], "not-applicable")
            self.assertIn("Isto e correcao de rota", frustration_text)
            self.assertLess(frustration_text.index("Isto e correcao de rota"), frustration_text.index("Workspace:"))
            self.assertNotIn("Reality/Evidence Gate", frustration_text)

            stuck_question = "estou travado com restricoes conflitantes e nao sei se o problema e escopo, arquitetura ou teste"
            stuck = runtime.build_guide_payload(root, question=stuck_question, max_chars=12000)
            stuck_text = run_cmd("guide", "--root", str(root), "--question", stuck_question).stdout

            self.assertEqual(stuck["intent_classification"], "confusion")
            self.assertEqual(stuck["recommended_workflow"], "problem-solving")
            self.assertEqual(stuck["workflow_metadata"].get("template"), "problem-solving-artifact")
            self.assertIn("problema observavel", stuck_text)
            self.assertIn("probe reversivel", stuck_text)

    def test_guidance_parity_replay_fixture_covers_required_families(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        catalog = json.loads(WORKFLOW_CATALOG.read_text(encoding="utf-8"))
        workflows = {item["id"]: item for item in catalog["workflows"]}
        families = {case["family"] for case in fixtures}

        self.assertTrue(PARITY_REQUIRED_FAMILIES <= families)
        for case in fixtures:
            self.assertIn("expected_classification", case)
            self.assertIn("expected_workflow", case)
            self.assertNotIn("bmad-", case["expected_workflow"])
            workflow = workflows[case["expected_workflow"]]
            if workflow.get("facilitation_pack") and case["expected_classification"] != "mechanical-build":
                self.assertEqual(
                    case.get("expected_facilitation_pack"),
                    f"skill:facilitation/{workflow['facilitation_pack']}.md",
                )
            if workflow.get("template") and case["expected_classification"] != "mechanical-build":
                self.assertEqual(case.get("expected_template"), workflow["template"])

    def test_parity_replay_command_validates_fixture_matrix(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        selected: list[dict[str, object]] = []
        covered: set[str] = set()
        for case in fixtures:
            family = str(case.get("family", ""))
            if family in PARITY_REQUIRED_FAMILIES and family not in covered:
                selected.append(case)
                covered.add(family)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "required-family-fixture.json"
            fixture.write_text(json.dumps(selected), encoding="utf-8")
            payload = json.loads(run_cmd("parity", "replay", "--fixture", str(fixture), "--json").stdout)

        self.assertEqual(payload["failed"], 0)
        self.assertEqual(payload["missing_families"], [])
        self.assertTrue(PARITY_REQUIRED_FAMILIES <= set(payload["covered_families"]))
        self.assertEqual(payload["passed"], payload["total"])

    def test_parity_replay_rejects_unsafe_guidance_payloads(self) -> None:
        runtime = load_runtime_module()
        route_reason = "Route reason. Signals: builder-flow. Route: 6-evolve / builder-flow -> runtime-builder."
        case = {
            "expected_classification": "builder-flow",
            "expected_phase": "6-evolve",
            "expected_workflow": "runtime-builder",
            "expected_facilitation_pack": "skill:facilitation/runtime-builder.md",
            "state_update_required": False,
        }
        payload = {
            "intent_classification": "builder-flow",
            "recommended_phase": "6-evolve",
            "recommended_workflow": "runtime-builder",
            "state_update_required": False,
            "facilitation_pack": "skill:facilitation/runtime-builder.md",
            "persona_lens": {},
            "workflow_metadata": {"id": "runtime-builder"},
            "commands": [{"name": "guide", "command": "guide --question <question> --json"}],
            "signals": ["builder-flow"],
            "recommended_action": "use chat memory instead of durable state when resuming",
            "human_prompt": "Let's use `runtime-builder` as the guided path. Good prompt. First question: what should change?",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "builder-flow",
                "active_guidance_mode": "runtime-builder",
                "last_route_reason": route_reason,
            },
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertTrue(any("guidance safety:" in failure for failure in failures))
        self.assertTrue(any("do not rely on chat memory" in failure for failure in failures))

    def test_runtime_guidance_surfaces_pass_safety_contract(self) -> None:
        runtime = load_runtime_module()
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "6-evolve",
                "--status",
                "guidance-safety",
                "--workflow",
                "runtime-builder",
                "--next-action",
                "use durable state instead of chat memory when resuming",
                "--force",
            )

            preflight = json.loads(run_cmd("preflight", "--root", str(root), "--json").stdout)
            reload_payload = json.loads(run_cmd("reload", "--root", str(root), "--json").stdout)
            guide = runtime.build_guide_payload(
                root,
                question="continue runtime-builder audit without relying on old chat",
                max_chars=12000,
            )

            self.assertEqual(runtime.validate_runtime_guidance_payload_safety("preflight", preflight), [])
            self.assertEqual(runtime.validate_runtime_guidance_payload_safety("reload", reload_payload), [])
            self.assertEqual(runtime.validate_runtime_guidance_payload_safety("guide", guide), [])

        sample = next(case for case in fixtures if case["id"] == "forge_human_experience_failure_outranks_builder")
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, sample["state"])
            sample_payload = runtime.build_guide_payload(root, question=sample["question"], max_chars=12000)

        self.assertEqual(runtime.validate_runtime_guidance_payload_safety("guide", sample_payload), [])

    def test_state_guidance_safety_rejects_misleading_next_action_write(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "State Safety Project", "--root", str(root))
            before = (root / ".forge-method" / "state.yaml").read_text(encoding="utf-8")

            result = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "6-evolve",
                "--next-action",
                "use chat memory instead of durable state when resuming",
                "--force",
                check=False,
            )
            after = (root / ".forge-method" / "state.yaml").read_text(encoding="utf-8")

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("State guidance validation failed", result.stderr)
            self.assertIn("do not rely on chat memory", result.stderr)
            self.assertEqual(after, before)

    def test_audit_rejects_preexisting_misleading_state_guidance(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Preexisting State Safety Project", "--root", str(root))
            state_path = root / ".forge-method" / "state.yaml"
            state_text = state_path.read_text(encoding="utf-8")
            state_path.write_text(
                state_text.replace(
                    'next_action: "resolve project route and confirm whether this is a new or existing project"',
                    'next_action: "continue stale state guidance until the user complains"',
                ),
                encoding="utf-8",
            )

            result = run_cmd("audit", "--root", str(root), check=False)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("misleading agent guidance", result.stdout)
            self.assertIn("do not follow stale state", result.stdout)

    def test_artifact_index_guidance_safety_rejects_misleading_write_before_file_creation(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Artifact Guidance Safety Project", "--root", str(root))

            result = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "runtime-contract",
                "--title",
                "Unsafe artifact",
                "--summary",
                "use chat memory instead of durable state when resuming",
                "--path",
                ".forge-method/artifacts/unsafe-artifact.md",
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Artifact index guidance validation failed", result.stderr)
            self.assertIn("do not rely on chat memory", result.stderr)
            self.assertFalse((root / ".forge-method" / "artifacts" / "unsafe-artifact.md").exists())
            self.assertFalse((root / ".forge-method" / "artifacts" / "index.ndjson").exists())

    def test_durable_runtime_guidance_sources_reject_misleading_writes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Durable Guidance Safety Project", "--root", str(root))

            story = run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--title",
                "use chat memory instead of durable state when resuming",
                "--acceptance",
                "safe acceptance",
                check=False,
            )
            self.assertNotEqual(story.returncode, 0)
            self.assertIn("Story guidance validation failed", story.stderr)
            self.assertIn("do not rely on chat memory", story.stderr)

            human_input = run_cmd(
                "input",
                "add",
                "--root",
                str(root),
                "--prompt",
                "continue stale state guidance until the user complains",
                "--optional",
                check=False,
            )
            self.assertNotEqual(human_input.returncode, 0)
            self.assertIn("Human input guidance validation failed", human_input.stderr)
            self.assertIn("do not follow stale state", human_input.stderr)

            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "safe-story",
                "--title",
                "Safe story",
                "--acceptance",
                "safe acceptance",
            )
            review = run_cmd(
                "review",
                "add",
                "--root",
                str(root),
                "--story",
                "safe-story",
                "--title",
                "Missing proof",
                "--summary",
                "use chat memory instead of durable state when resuming",
                check=False,
            )
            self.assertNotEqual(review.returncode, 0)
            self.assertIn("Review finding guidance validation failed", review.stderr)
            self.assertIn("do not rely on chat memory", review.stderr)

    def test_audit_rejects_preexisting_misleading_durable_guidance_sources(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Legacy Durable Guidance Safety Project", "--root", str(root))
            run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Safe artifact",
                "--summary",
                "Safe artifact summary.",
                "--path",
                ".forge-method/artifacts/safe-artifact.md",
            )
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "safe-story",
                "--title",
                "Safe story",
                "--acceptance",
                "safe acceptance",
            )

            index = root / ".forge-method" / "artifacts" / "index.ndjson"
            unsafe_entry = {
                "ts": "2026-06-16T00:00:00+00:00",
                "kind": "runtime-contract",
                "title": "Legacy unsafe artifact",
                "path": ".forge-method/artifacts/legacy-unsafe.md",
                "summary": "use chat memory instead of durable state when resuming",
                "lifecycle": "durable",
                "status": "active",
            }
            with index.open("a", encoding="utf-8") as handle:
                handle.write(json.dumps(unsafe_entry, ensure_ascii=True, sort_keys=True) + "\n")

            input_path = root / ".forge-method" / "inputs" / "unsafe-input.yaml"
            input_path.parent.mkdir(parents=True, exist_ok=True)
            input_path.write_text(
                "\n".join(
                    [
                        "# Forge Method human input",
                        'id: "unsafe-input"',
                        'prompt: "continue stale state guidance until the user complains"',
                        'reason: "legacy contamination fixture"',
                        'status: "open"',
                        'phase: "1-discovery"',
                        'required: "false"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            review_path = root / ".forge-method" / "reviews" / "unsafe-review.yaml"
            review_path.parent.mkdir(parents=True, exist_ok=True)
            review_path.write_text(
                "\n".join(
                    [
                        "# Forge Method review finding",
                        'id: "unsafe-review"',
                        'story: "safe-story"',
                        'title: "Unsafe review summary"',
                        'severity: "medium"',
                        'status: "open"',
                        'summary: "use chat memory instead of durable state when resuming"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            story_path = root / ".forge-method" / "stories" / "safe-story.yaml"
            story_path.write_text(
                story_path.read_text(encoding="utf-8").replace(
                    'acceptance_criteria: "safe acceptance"',
                    'acceptance_criteria: "continue stale state guidance until the user complains"',
                ),
                encoding="utf-8",
            )

            audit = run_cmd("audit", "--root", str(root), check=False)

            self.assertNotEqual(audit.returncode, 0)
            self.assertIn(".forge-method/artifacts/index.ndjson:.forge-method/artifacts/legacy-unsafe.md:summary", audit.stdout)
            self.assertIn(".forge-method/inputs/unsafe-input.yaml:prompt", audit.stdout)
            self.assertIn(".forge-method/reviews/unsafe-review.yaml:summary", audit.stdout)
            self.assertIn(".forge-method/stories/safe-story.yaml:acceptance_criteria", audit.stdout)
            self.assertIn("do not rely on chat memory", audit.stdout)
            self.assertIn("do not follow stale state", audit.stdout)

    def test_runtime_audit_rejects_product_docs_that_describe_forge_as_benchmark_fork(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            plugin_dir = root / ".codex-plugin"
            plugin_dir.mkdir(parents=True)
            (plugin_dir / "plugin.json").write_text(json.dumps({"name": "forge-method-core"}), encoding="utf-8")
            maintainer_dir = root / ".forge-method"
            maintainer_dir.mkdir()
            (maintainer_dir / "core-dev.local").write_text("", encoding="utf-8")
            run_cmd(
                "init",
                "--project",
                "Runtime Docs Safety",
                "--root",
                str(root),
                "--allow-runtime-state",
                "--no-project-guidance",
            )
            (root / "README.md").write_text("Forge Method is a BMAD fork with nicer prompts.\n", encoding="utf-8")

            audit = run_cmd("audit", "--root", str(root), check=False)

            self.assertNotEqual(audit.returncode, 0)
            self.assertIn("product-facing docs must not describe Forge as a clone, fork, or variant", audit.stdout)
            self.assertIn("README.md:1", audit.stdout)

    def test_runtime_product_docs_guard_allows_git_clone_and_negative_policy_language(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            plugin_dir = root / ".codex-plugin"
            plugin_dir.mkdir(parents=True)
            (plugin_dir / "plugin.json").write_text(json.dumps({"name": "forge-method-core"}), encoding="utf-8")
            maintainer_dir = root / ".forge-method"
            maintainer_dir.mkdir()
            (maintainer_dir / "core-dev.local").write_text("", encoding="utf-8")
            run_cmd(
                "init",
                "--project",
                "Runtime Docs Safe",
                "--root",
                str(root),
                "--allow-runtime-state",
                "--no-project-guidance",
            )
            (root / "README.md").write_text(
                "\n".join(
                    [
                        "This guide goes from a fresh clone to a working Forge Method project.",
                        "git clone https://github.com/example/forge-method-core.git",
                        "Do not describe Forge Method as a clone, fork, or variant of another framework.",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            audit = run_cmd("audit", "--root", str(root))

            self.assertIn("Audit passed.", audit.stdout)

    def test_product_docs_guard_does_not_apply_to_user_projects(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "User Project", "--root", str(root))
            (root / "README.md").write_text("This user project is a fork of a framework.\n", encoding="utf-8")

            audit = run_cmd("audit", "--root", str(root))

            self.assertIn("Audit passed.", audit.stdout)

    def test_parity_replay_requires_pack_assertions_for_human_guidance(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "help_next_step_orientation").copy()
        case.pop("expected_facilitation_pack", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_facilitation_pack", failures)

    def test_parity_replay_requires_template_assertions_for_human_guidance(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "forge_experience_not_example_project").copy()
        case.pop("expected_template", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_template", failures)

    def test_parity_replay_requires_persona_lens_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "architecture_after_prd_request").copy()
        case.pop("expected_persona_lens", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_persona_lens", failures)

    def test_parity_replay_requires_persona_lens_route_reason_marker(self) -> None:
        runtime = load_runtime_module()
        case = {
            "expected_classification": "persona-lens",
            "expected_phase": "1-discovery",
            "expected_workflow": "design-thinking",
            "expected_persona_lens": "design-thinking-coach",
            "expected_facilitation_pack": "skill:facilitation/design-thinking.md",
            "expected_template": "design-thinking-artifact",
            "state_update_required": True,
        }
        route_reason = "The message asks for Design Thinking Coach guidance. Signals: persona-lens. Route: 1-discovery / persona-lens -> design-thinking."
        payload = {
            "intent_classification": "persona-lens",
            "recommended_phase": "1-discovery",
            "recommended_workflow": "design-thinking",
            "state_update_required": True,
            "facilitation_pack": "skill:facilitation/design-thinking.md",
            "persona_lens": {"id": "design-thinking-coach"},
            "workflow_metadata": {"id": "design-thinking", "template": "design-thinking-artifact"},
            "commands": [{"name": "transition-workflow"}],
            "signals": ["persona-lens"],
            "recommended_action": "run design-thinking",
            "human_prompt": "Let's use `design-thinking` as the guided path. Use a design-thinking lens. First question: what must stay true for the user?",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "persona-lens",
                "active_guidance_mode": "design-thinking",
                "last_route_reason": route_reason,
            },
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertIn("route_reason must include persona lens selected marker", failures)

    def test_parity_replay_requires_state_update_handoff_coherence(self) -> None:
        runtime = load_runtime_module()
        route_reason = "Route reason. Signals: product-flow. Route: 2-specification / product-flow -> write-spec."
        case = {
            "expected_classification": "product-flow",
            "expected_phase": "2-specification",
            "expected_workflow": "write-spec",
            "expected_facilitation_pack": "skill:facilitation/product-planning.md",
            "expected_template": "spec-kernel-artifact",
            "state_update_required": True,
        }
        payload = {
            "intent_classification": "product-flow",
            "recommended_phase": "2-specification",
            "recommended_workflow": "write-spec",
            "state_update_required": True,
            "facilitation_pack": "skill:facilitation/product-planning.md",
            "persona_lens": {},
            "workflow_metadata": {"id": "write-spec", "template": "spec-kernel-artifact"},
            "commands": [{"name": "transition-workflow"}],
            "signals": ["product-flow"],
            "recommended_action": "write the spec",
            "human_prompt": "Let's use `write-spec` as the guided path. Use product planning. First question: what must stay true for the user?",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "product-flow",
                "active_guidance_mode": "architecture",
                "last_route_reason": route_reason,
            },
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertIn("state_updates.active_guidance_mode: expected 'write-spec', got 'architecture'", failures)

    def test_parity_replay_requires_human_facing_facilitated_prompt(self) -> None:
        runtime = load_runtime_module()
        route_reason = "Route reason. Signals: brainstorm. Route: 1-discovery / brainstorm -> brainstorming."
        case = {
            "expected_classification": "brainstorm",
            "expected_phase": "1-discovery",
            "expected_workflow": "brainstorming",
            "expected_facilitation_pack": "skill:facilitation/brainstorming.md",
            "expected_template": "brainstorming-artifact",
            "state_update_required": True,
        }
        payload = {
            "intent_classification": "brainstorm",
            "recommended_phase": "1-discovery",
            "recommended_workflow": "brainstorming",
            "state_update_required": True,
            "facilitation_pack": "skill:facilitation/brainstorming.md",
            "persona_lens": {},
            "workflow_metadata": {"id": "brainstorming", "template": "brainstorming-artifact"},
            "commands": [{"name": "transition-workflow"}],
            "signals": ["brainstorm"],
            "recommended_action": "run brainstorming",
            "human_prompt": "I should keep this divergent until options exist.",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "brainstorm",
                "active_guidance_mode": "brainstorming",
                "last_route_reason": route_reason,
            },
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertIn("human_prompt must include a human-facing first question for facilitated guidance", failures)
        self.assertIn("human_prompt must not start as an internal agent note", failures)
        self.assertIn("human_prompt must not contain internal 'I should' phrasing", failures)

    def test_parity_replay_requires_route_reason_specificity(self) -> None:
        runtime = load_runtime_module()
        route_reason = "The message asks for orientation or indicates uncertainty."
        case = {
            "expected_classification": "confusion",
            "expected_phase": "1-discovery",
            "expected_workflow": "problem-solving",
            "expected_facilitation_pack": "skill:facilitation/problem-solving.md",
            "expected_template": "problem-solving-artifact",
            "state_update_required": True,
        }
        payload = {
            "intent_classification": "confusion",
            "recommended_phase": "1-discovery",
            "recommended_workflow": "problem-solving",
            "state_update_required": True,
            "facilitation_pack": "skill:facilitation/problem-solving.md",
            "persona_lens": {},
            "workflow_metadata": {"id": "problem-solving", "template": "problem-solving-artifact"},
            "commands": [{"name": "transition-workflow"}],
            "signals": ["confusion"],
            "recommended_action": "run problem-solving",
            "human_prompt": "Let's use `problem-solving` as the guided path. Diagnose first. First question: what symptom should anchor the diagnosis?",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "confusion",
                "active_guidance_mode": "problem-solving",
                "last_route_reason": route_reason,
            },
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertIn("route_reason must include Signals and Route summary", failures)

    def test_first_guidance_questions_are_workflow_specific(self) -> None:
        runtime = load_runtime_module()
        workflows = [
            "write-spec",
            "discover-intent",
            "ux-plan",
            "architecture",
            "quick-dev",
            "game-brief",
            "engine-setup",
            "game-sprint-status",
            "test-strategy",
            "test-automation",
            "module-ideation",
            "module-validate",
            "doc-index",
            "doc-shard",
            "session-prep",
            "checkpoint-preview",
        ]

        questions = [runtime.first_guidance_question("product-flow", workflow) for workflow in workflows]

        self.assertEqual(len(questions), len(set(questions)))
        self.assertIn("close discovery", runtime.first_guidance_question("operate-support", "discover-intent"))
        self.assertIn("spec kernel", runtime.first_guidance_question("product-flow", "write-spec"))
        self.assertIn("dump the whole game", runtime.first_guidance_question("game-flow", "game-brief"))
        self.assertIn("what are we brainstorming about", runtime.first_guidance_question("brainstorm", "brainstorming"))
        self.assertIn("engine profile", runtime.first_guidance_question("game-flow", "engine-setup"))
        self.assertIn("install", runtime.first_guidance_question("builder-flow", "module-distribution"))

    def test_human_experience_stress_routes_and_style_contracts(self) -> None:
        runtime = load_runtime_module()
        cases = [
            {
                "question": "quero criar um VTT bonito e imersivo pra jogar RPG online, com IA, mapas, musica, automacoes e talvez absorver livros de RPG",
                "classification": "game-flow",
                "workflow": "game-brief",
                "pace": "coaching",
                "prompt": "dump the whole game",
            },
            {
                "question": "to com pressa, so quero um app pequeno de checklist hoje, sem discovery longo",
                "classification": "product-flow",
                "workflow": "quick-dev",
                "pace": "fast-path",
                "prompt": "tiny scope",
            },
            {
                "question": "nao sei direito o que quero, estou perdido, me ajuda a destravar a ideia",
                "classification": "confusion",
                "workflow": "problem-solving",
                "pace": "diagnostic",
                "prompt": "symptom",
            },
            {
                "question": "quero brainstorm de direcoes para um jogo de mesa online antes de decidir",
                "classification": "brainstorm",
                "workflow": "brainstorming",
                "pace": "divergent",
                "prompt": "what are we brainstorming about",
            },
            {
                "question": "preciso pesquisar Foundry Fantasy Grounds e VTTs antes de escolher direcao",
                "classification": "research-needed",
                "workflow": "market-scan",
                "pace": "evidence-first",
                "prompt": "adopt or switch",
            },
            {
                "question": "isso ta frio e quadrado, nao esta me guiando como facilitador",
                "classification": "correct-course",
                "workflow": "correct-course",
                "pace": "repair",
                "prompt": "recover trust",
            },
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            for case in cases:
                payload = runtime.build_guide_payload(root, question=case["question"], max_chars=12000)
                style = payload["human_experience"]["style_contract"]

                self.assertEqual(payload["intent_classification"], case["classification"], case["question"])
                self.assertEqual(payload["recommended_workflow"], case["workflow"], case["question"])
                self.assertEqual(style["pace"], case["pace"], case["question"])
                self.assertIn(case["prompt"], payload["human_prompt"], case["question"])
                self.assertNotIn("I should ", payload["human_prompt"], case["question"])

    def test_game_mda_lens_routes_to_game_studio_workflows(self) -> None:
        runtime = load_runtime_module()
        cases = [
            (
                "quero usar MDA Lens para decidir qual experiencia o jogador deve sentir e como provar isso",
                "game-brief",
            ),
            (
                "as dinamicas e mecanicas do jogador nao fecham com a estetica alvo",
                "mechanics-design",
            ),
            (
                "como provar se a diversao e a imersao apareceram no playtest",
                "playtest-plan",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            for question, workflow in cases:
                payload = runtime.build_guide_payload(root, question=question, max_chars=12000)

                self.assertEqual(payload["intent_classification"], "game-flow", question)
                self.assertEqual(payload["recommended_workflow"], workflow, question)
                self.assertIn("game-flow", payload["signals"], question)

    def test_standalone_stack_conversation_stays_in_research_guidance(self) -> None:
        runtime = load_runtime_module()
        cases = [
            (
                "entao, eu quero fazer um fork, manter o forge-method-core plugin do codex, e criar um clone dele pra testar com o APP em volta dele, "
                "podemos chamar de forge-standalone-app assim nao tem o perigo de confundir e to pensando em qual linguagem faria ele mais rapido e performativo, "
                "como referencia tenho gostado muito do pi.dev, codex/zed, rust, lua, odin e elixir tbm me interessam, entao tudo vai depender de uma grande pesquisa, "
                "muito estudo e uma longa conversa"
            ),
            (
                "sim, a pergunta aqui fica, qual seria a melhor linguagem pra interface, parece que ja decidimos algumas coisas, "
                "rust no coracao, pi.dev como inspiracao, humanos guiados, ainda quero iterar mais"
            ),
            (
                "gostaria de encontrar alguma empresa reconhecida por codebase impecavel em rust, "
                "e nos espelharmos nos padroes que usam na codebase"
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "ready")
            for question in cases:
                payload = runtime.build_guide_payload(root, question=question, max_chars=12000)
                style = payload["human_experience"]["style_contract"]

                self.assertEqual(payload["intent_classification"], "research-needed", question)
                self.assertEqual(payload["recommended_phase"], "6-evolve", question)
                self.assertEqual(payload["recommended_workflow"], "technical-feasibility-scan", question)
                self.assertEqual(style["pace"], "evidence-first", question)
                self.assertIn("research-needed", payload["signals"], question)
                self.assertIn("technical promise", payload["human_prompt"], question)
                self.assertTrue(payload["state_update_required"], question)
                self.assertIn("transition-evolve", [item["name"] for item in payload["commands"]])
                self.assertNotIn("fast-path", json.dumps(style), question)

    def test_guideline_audit_routes_before_permanent_implementation(self) -> None:
        runtime = load_runtime_module()
        question = (
            "antes de implementacao permanente, crie uma guideline e uma work order "
            "com acceptance evidence para agentes do forge standalone"
        )
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "evolve_runtime")
            payload = runtime.build_guide_payload(root, question=question, max_chars=12000)

            self.assertEqual(payload["intent_classification"], "builder-flow")
            self.assertEqual(payload["recommended_workflow"], "guideline-audit")
            self.assertEqual(payload["workflow_metadata"]["template"], "guideline-audit-artifact")
            self.assertEqual(payload["facilitation_pack"], "skill:facilitation/guideline-audit.md")
            self.assertIn("builder-flow", payload["signals"])

    def test_ready_project_guideline_audit_enters_evolve_directly(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "ready")
            payload = runtime.build_guide_payload(
                root,
                question="adicione um guideline audit gate antes de criar Rust crates permanentes",
                max_chars=12000,
            )

            self.assertEqual(payload["intent_classification"], "builder-flow")
            self.assertEqual(payload["recommended_phase"], "6-evolve")
            self.assertEqual(payload["recommended_workflow"], "guideline-audit")
            self.assertTrue(payload["state_update_required"])
            self.assertIn("transition-evolve", [item["name"] for item in payload["commands"]])

    def test_parity_replay_requires_mechanical_build_status_prompt(self) -> None:
        runtime = load_runtime_module()
        route_reason = "A build-ready story exists. Signals: mechanical-build. Route: 4-build-verify / mechanical-build -> build-story."
        case = {
            "expected_classification": "mechanical-build",
            "expected_phase": "4-build-verify",
            "expected_workflow": "build-story",
            "expected_facilitation_pack": "skill:facilitation/story-lifecycle.md",
            "expected_template": "build-story-work-order",
            "state_update_required": False,
            "expected_codex_goal_handoff_recommended": True,
            "expected_mechanical_work_order_autonomous": True,
        }
        payload = {
            "intent_classification": "mechanical-build",
            "recommended_phase": "4-build-verify",
            "recommended_workflow": "build-story",
            "state_update_required": False,
            "facilitation_pack": "skill:facilitation/story-lifecycle.md",
            "persona_lens": {},
            "workflow_metadata": {"id": "build-story", "template": "build-story-work-order"},
            "commands": [{"name": "guide"}],
            "signals": ["mechanical-build"],
            "recommended_action": "implement and validate story story-1",
            "human_prompt": "The approved decision work is done; I should continue mechanically and write evidence.",
            "alternatives": [],
            "guidance_engine": {"route_reason": route_reason},
            "state_updates": {
                "last_intent_classification": "mechanical-build",
                "active_guidance_mode": "build-story",
                "last_route_reason": route_reason,
            },
            "codex_goal_handoff": {"recommended": True},
            "mechanical_work_order": {"autonomous": True},
        }

        failures = runtime.parity_case_failures(case, payload)

        self.assertIn("mechanical-build human_prompt must be status wording, not facilitation or internal notes", failures)

    def test_parity_replay_requires_council_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "lifecycle_party_mode_council_request").copy()
        case.pop("expected_council_recommended", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_council_recommended", failures)

    def test_parity_replay_requires_codex_goal_handoff_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "mechanical_build").copy()
        case.pop("expected_codex_goal_handoff_recommended", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_codex_goal_handoff_recommended", failures)

    def test_parity_replay_requires_mechanical_work_order_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "mechanical_build").copy()
        case.pop("expected_mechanical_work_order_autonomous", None)

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_mechanical_work_order_autonomous", failures)

    def test_parity_replay_requires_multi_command_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "method_frustration_ready").copy()
        case.pop("expected_commands", None)
        case["expected_command"] = "correct-course"

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("fixture must declare expected_commands", failures)

    def test_parity_replay_rejects_partial_mutating_command_assertions(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        case = next(item for item in fixtures if item["id"] == "method_frustration_ready").copy()
        case["expected_commands"] = ["correct-course"]

        with tempfile.TemporaryDirectory() as raw:
            fixture = Path(raw) / "fixture.json"
            fixture.write_text(json.dumps([case]), encoding="utf-8")

            result = run_cmd("parity", "replay", "--fixture", str(fixture), "--json", check=False)

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        failures = "\n".join("\n".join(item["failures"]) for item in payload["failures"])
        self.assertIn("mutating_commands: expected", failures)

    def test_packaged_reality_workflows_are_available(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            workflow_list = run_cmd("workflow", "list", "--root", str(root)).stdout

            self.assertIn("reality-evidence-gate", workflow_list)
            self.assertIn("market-scan", workflow_list)
            self.assertIn("domain-scan", workflow_list)
            self.assertIn("technical-feasibility-scan", workflow_list)

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
            add_decision_source(root)
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

            self.assertEqual(snapshot["runtime_version"], CURRENT_VERSION)
            self.assertEqual(snapshot["state"]["phase"], "4-build-verify")
            self.assertEqual(snapshot["stories"]["next"]["id"], "story-1")
            self.assertEqual(snapshot["route"]["recommendation"], "start_next_story")
            self.assertEqual(snapshot["resume"]["action"], "start_next_story")
            self.assertTrue(snapshot["resume"]["autonomous"])
            self.assertTrue(snapshot["resume"]["mechanical_work_order"]["goal_recommended"])
            self.assertTrue(snapshot["resume"]["codex_goal_handoff"]["recommended"])
            self.assertEqual(snapshot["help_oracle"]["required_next_workflow"], "build-story")
            self.assertEqual(snapshot["resume"]["help_oracle"]["required_next_workflow"], "build-story")
            self.assertIn("implementation story", snapshot["help_oracle"]["reason"])
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

    def test_story_lifecycle_guard_requires_decision_source_for_build_story(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            blocked_add = run_cmd(
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
                check=False,
            )

            blocked = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertNotEqual(blocked_add.returncode, 0)
            self.assertIn("requires an approved decision artifact", blocked_add.stderr)
            self.assertTrue(blocked["quality"]["audit"]["passed"])
            self.assertEqual(blocked["stories"]["counts"]["ready"], 0)

            source = add_decision_source(root)
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
            released = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            story_text = (root / ".forge-method" / "stories" / "story-1.yaml").read_text(encoding="utf-8")

            self.assertTrue(released["quality"]["audit"]["passed"])
            self.assertEqual(released["route"]["recommendation"], "start_next_story")
            self.assertEqual(released["help_oracle"]["required_next_workflow"], "build-story")
            self.assertIn(f'decision_sources: "{source}"', story_text)

            second_source = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "architecture",
                "--title",
                "Second decision source",
                "--summary",
                "A second accepted decision source makes story source selection ambiguous.",
                "--path",
                ".forge-method/artifacts/second-decision-source.md",
            ).stdout.strip()
            ambiguous_add = run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-2",
                "--title",
                "Build second thing",
                "--acceptance",
                "second thing works",
                check=False,
            )

            self.assertNotEqual(ambiguous_add.returncode, 0)
            self.assertIn("multiple decision artifacts exist", ambiguous_add.stderr)

            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "story-2",
                "--title",
                "Build second thing",
                "--acceptance",
                "second thing works",
                "--source",
                second_source,
            )
            story_2_text = (root / ".forge-method" / "stories" / "story-2.yaml").read_text(encoding="utf-8")
            self.assertIn(f'decision_sources: "{second_source}"', story_2_text)

    def test_status_brief_surfaces_actionable_runtime_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            add_decision_source(root)
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
            next_json = json.loads(run_cmd("next", "--root", str(root), "--json").stdout)
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
            self.assertEqual(resume["help_oracle"]["required_next_workflow"], "discover-intent")
            self.assertIn("Required human input", resume["help_oracle"]["reason"])
            self.assertIn("Action: answer_required_input", resume_text)
            self.assertIn("Help Oracle:", resume_text)
            self.assertIn("answer human input target-user", next_text)
            self.assertIn("Next required workflow: discover-intent", next_text)
            self.assertEqual(next_json["action"], "answer_required_input")
            self.assertFalse(next_json["autonomous"])
            self.assertEqual(next_json["required_next_workflow"], "discover-intent")
            self.assertIn("Required human input", next_json["reason"])
            self.assertEqual(next_json["context_boundary"]["mode"], "resume-first")
            self.assertIn("input-list", [command["name"] for command in next_json["commands"]])
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

    def test_help_oracle_overrides_ready_state_stale_next_action(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "5-ready-operate",
                "--status",
                "ready",
                "--workflow",
                "ready-release",
                "--next-action",
                "publish current batch",
                "--force",
            )

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            next_text = run_cmd("next", "--root", str(root)).stdout
            next_json = json.loads(run_cmd("next", "--root", str(root), "--json").stdout)

            self.assertEqual(snapshot["resume"]["action"], "operate_or_evolve")
            self.assertEqual(snapshot["help_oracle"]["required_next_workflow"], "guidance-engine")
            self.assertEqual(resume["help_oracle"]["required_next_workflow"], "guidance-engine")
            self.assertEqual(snapshot["help_oracle"]["context_boundary"]["mode"], "resume-first")
            self.assertIn(".forge-method/state.yaml", snapshot["help_oracle"]["context_boundary"]["read_first"])
            self.assertIn("Ready projects must route", snapshot["help_oracle"]["reason"])
            self.assertIn("Next required workflow: guidance-engine", next_text)
            self.assertNotIn("publish current batch", next_text)
            self.assertEqual(next_json["required_next_workflow"], "guidance-engine")
            self.assertIn("Ready projects must route", next_json["reason"])
            self.assertEqual(next_json["context_boundary"]["mode"], "resume-first")
            self.assertNotIn("publish current batch", next_json["human_next_step"])

    def test_help_oracle_respects_active_evolve_workflow_even_when_ready(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "6-evolve",
                "--status",
                "parity-audit-recorded",
                "--workflow",
                "runtime-builder",
                "--next-action",
                "Implement Help Oracle invariant",
                "--force",
            )

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            next_text = run_cmd("next", "--root", str(root)).stdout
            next_json = json.loads(run_cmd("next", "--root", str(root), "--json").stdout)

            self.assertEqual(snapshot["resume"]["action"], "continue_current_workflow")
            self.assertEqual(snapshot["help_oracle"]["required_next_workflow"], "runtime-builder")
            self.assertIn("Continue the active workflow", snapshot["help_oracle"]["reason"])
            self.assertIn("Implement Help Oracle invariant", next_text)
            self.assertIn("Next required workflow: runtime-builder", next_text)
            self.assertEqual(next_json["required_next_workflow"], "runtime-builder")
            self.assertIn("Continue the active workflow", next_json["reason"])
            self.assertEqual(next_json["context_boundary"]["current_workflow"], "runtime-builder")

    def test_mutating_commands_record_and_emit_post_command_help_oracle(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            init_text = run_cmd("init", "--project", "Example Project", "--root", str(root)).stdout

            self.assertIn("Help Oracle:", init_text)
            self.assertIn("stale_state_guard", init_text)

            transition_text = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "1-discovery",
                "--status",
                "discovery-ready",
                "--workflow",
                "discover-intent",
            ).stdout

            self.assertIn("Help Oracle:", transition_text)
            self.assertIn("required_next_workflow: discover-intent", transition_text)
            self.assertIn("alternatives:", transition_text)

            artifact_path = add_decision_source(root)
            self.assertEqual(artifact_path, ".forge-method/artifacts/test-decision-source.md")

            events = ledger_events(root, "help_oracle.recorded")
            self.assertGreaterEqual(len(events), 3)
            latest = events[-1]["payload"]
            self.assertEqual(latest["required_next_workflow"], "discover-intent")
            self.assertIn("stale_state_guard", latest)
            self.assertEqual(latest["context_boundary"]["mode"], "resume-first")
            self.assertTrue(latest["alternatives"])

    def test_story_block_routes_without_fake_human_input(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd("transition", "--root", str(root), "--phase", "2-specification")
            run_cmd("transition", "--root", str(root), "--phase", "3-plan")
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            add_decision_source(root)
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
            add_decision_source(root)
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

    def test_invalid_done_transition_does_not_write_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            add_decision_source(root)
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

            result = run_cmd(
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

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Invalid story transition", result.stderr + result.stdout)
            self.assertEqual([], list((root / ".forge-method" / "evidence").glob("*story-1-done.md")))

    def test_story_start_preserves_discovery_phase_workflow(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")
            run_cmd(
                "story",
                "add",
                "--root",
                str(root),
                "--id",
                "discover-existing-system",
                "--title",
                "Discover existing system",
                "--acceptance",
                "inventory exists",
            )

            run_cmd("story", "start", "--root", str(root), "--id", "discover-existing-system")
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(snapshot["state"]["phase"], "1-discovery")
            self.assertEqual(snapshot["state"]["active_workflow"], "discover-intent")
            self.assertEqual(snapshot["state"]["active_story"], "discover-existing-system")
            self.assertIn("run discovery", snapshot["state"]["next_action"])

    def test_review_findings_block_done_until_resolved(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            add_decision_source(root)
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
            add_decision_source(root)
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
                json.dumps({"name": "forge-method-core", "version": CURRENT_VERSION, "skills": "./skills/"}),
                encoding="utf-8",
            )
            skill_path.write_text("---\nname: forge-method\n---\n", encoding="utf-8")
            env = {"HOME": str(home), "USERPROFILE": str(home)}

            payload = json.loads(run_cmd("doctor", "--root", str(home), "--json", env=env).stdout)
            text = run_cmd("doctor", "--root", str(home), env=env).stdout

            plugin = payload["plugin_installation"]
            self.assertTrue(plugin["available"])
            self.assertEqual(plugin["status"], "ready")
            self.assertEqual(plugin["installed_version"], CURRENT_VERSION)
            self.assertEqual(plugin["plugin_path"], str(plugin_root.resolve()))
            self.assertEqual(plugin["repair_commands"]["windows"], [])
            self.assertIn("codex://plugins/forge-method-core?marketplacePath=", plugin["codex_deeplink"])
            self.assertIn("Plugin installation:", text)
            self.assertIn("Status: ready", text)
            self.assertIn("Open in Codex:", text)

    def test_doctor_suggests_repair_for_stale_plugin_installation(self) -> None:
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
            self.assertFalse(plugin["available"])
            self.assertEqual(plugin["status"], "plugin version mismatch")
            self.assertEqual(plugin["installed_version"], "1.22.0")
            self.assertIn(
                "powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1",
                plugin["repair_commands"]["windows"],
            )
            self.assertIn("Status: plugin version mismatch", text)
            self.assertIn("Repair: powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1", text)

    def test_snapshot_reports_plugin_installation_diagnostics_without_blocking_quality(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            project = Path(raw) / "project"
            home.mkdir()
            project.mkdir()
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
            run_cmd("init", "--project", "Snapshot Diagnostics", "--root", str(project), env=env)

            snapshot = json.loads(run_cmd("snapshot", "--root", str(project), env=env).stdout)

            plugin = snapshot["diagnostics"]["plugin_installation"]
            self.assertFalse(plugin["available"])
            self.assertEqual(plugin["status"], "plugin version mismatch")
            self.assertEqual(plugin["expected_version"], CURRENT_VERSION)
            self.assertEqual(plugin["installed_version"], "1.22.0")
            self.assertIn(
                "powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1",
                plugin["repair_commands"]["windows"],
            )
            self.assertTrue(snapshot["quality"]["audit"]["passed"])
            resume = json.loads(run_cmd("resume", "--root", str(project), "--json", env=env).stdout)
            context_plan = json.loads(
                run_cmd("context", "plan", "--root", str(project), "--json", env=env).stdout
            )
            context_health = json.loads(
                run_cmd("context", "health", "--root", str(project), "--json", env=env).stdout
            )
            preflight = json.loads(run_cmd("preflight", "--root", str(project), "--json", env=env).stdout)
            reload = json.loads(run_cmd("reload", "--root", str(project), "--json", env=env).stdout)
            resume_text = run_cmd("resume", "--root", str(project), env=env).stdout
            context_health_text = run_cmd("context", "health", "--root", str(project), env=env).stdout
            preflight_text = run_cmd("preflight", "--root", str(project), env=env).stdout
            reload_text = run_cmd("reload", "--root", str(project), env=env).stdout

            self.assertEqual(
                resume["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertEqual(
                context_plan["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertEqual(
                context_health["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertEqual(
                preflight["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertEqual(
                preflight["context_health"]["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertEqual(
                reload["diagnostics"]["plugin_installation"]["status"],
                "plugin version mismatch",
            )
            self.assertIn("Diagnostics:", resume_text)
            self.assertIn("plugin_installation: plugin version mismatch", resume_text)
            self.assertIn(
                "plugin_repair: powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1",
                resume_text,
            )
            self.assertIn("plugin_installation: plugin version mismatch", context_health_text)
            self.assertIn("plugin_installation: plugin version mismatch", preflight_text)
            self.assertIn("plugin_installation: plugin version mismatch", reload_text)

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

    def test_artifact_verify_warns_on_stale_parity_guidance_markers(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifact = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "internal-parity-audit",
                "--title",
                "Parity Audit",
                "--summary",
                "Parity audit.",
            ).stdout.strip()
            (root / artifact).write_text(
                "\n".join(
                    [
                        "# Parity Audit",
                        "",
                        "Next implementation batch: real-use transcript hardening for the remaining partial/strong-ish audit rows.",
                    ]
                ),
                encoding="utf-8",
            )

            result = run_cmd("artifact", "verify", "--root", str(root))

            self.assertIn("Artifact verification warnings:", result.stdout)
            self.assertIn("artifact guidance may be stale", result.stdout)
            self.assertIn("remaining partial/strong-ish", result.stdout)

    def test_document_utility_doc_check_detects_stale_sources_and_shard_ambiguity(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            docs = root / "docs"
            docs.mkdir()
            source = docs / "guide.md"
            source.write_text("# Guide\n\nCurrent truth.\n", encoding="utf-8")
            artifact = root / ".forge-method" / "artifacts" / "doc-index-proof.md"
            artifact.write_text(
                "\n".join(
                    [
                        "# Document Utility Artifact",
                        "workflow: doc-index",
                        "audience: future agent",
                        "doc_job: navigation",
                        "target_docs: docs",
                        "indexed_docs: docs/guide.md",
                        "source_of_truth: docs/guide.md",
                        f"source_fingerprint: {sha256(source)}",
                        f"source_last_modified: {source.stat().st_mtime}",
                        "navigation_rules: read docs/guide.md first",
                        "stale_check: source hash and mtime verified",
                        "validation: artifact doc-check --path .forge-method/artifacts/doc-index-proof.md",
                        "next_workflow: editorial-review",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            passed = run_cmd("artifact", "doc-check", "--root", str(root), "--path", str(artifact)).stdout
            self.assertIn("Document utility check passed.", passed)

            source.write_text("# Guide\n\nChanged truth.\n", encoding="utf-8")
            os.utime(source, (artifact.stat().st_mtime + 5, artifact.stat().st_mtime + 5))
            stale = run_cmd("artifact", "doc-check", "--root", str(root), "--path", str(artifact), check=False)
            self.assertNotEqual(stale.returncode, 0)
            self.assertIn("source_of_truth is newer than artifact", stale.stdout)

            shard_index = docs / "guide" / "index.md"
            shard_index.parent.mkdir()
            shard_index.write_text("# Guide shards\n", encoding="utf-8")
            source.write_text("# Guide\n\nRestored truth.\n", encoding="utf-8")
            shard_artifact = root / ".forge-method" / "artifacts" / "doc-shard-proof.md"
            shard_artifact.write_text(
                "\n".join(
                    [
                        "# Document Utility Artifact",
                        "workflow: doc-shard",
                        "audience: future agent",
                        "doc_job: split large markdown",
                        "target_docs: docs/guide.md",
                        "source_of_truth: docs/guide.md",
                        f"source_fingerprint: {sha256(source)}",
                        f"source_last_modified: {source.stat().st_mtime}",
                        "generated_or_derived_docs: docs/guide/index.md",
                        "shard_index: docs/guide/index.md",
                        "original_doc_decision: keep",
                        "precedence_rule: whole source document wins until archive decision",
                        "stale_check: source hash and shard index verified",
                        "validation: artifact doc-check --path .forge-method/artifacts/doc-shard-proof.md",
                        "next_workflow: doc-index",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            ambiguous = run_cmd("artifact", "doc-check", "--root", str(root), "--path", str(shard_artifact), check=False)
            self.assertNotEqual(ambiguous.returncode, 0)
            self.assertIn("keeping the original source requires stale_waiver", ambiguous.stdout)

    def test_artifact_document_generators_create_index_and_shard_with_freshness_proof(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            docs = root / "docs"
            docs.mkdir()
            source = docs / "guide.md"
            source.write_text("# Guide\n\nCurrent source of truth.\n", encoding="utf-8")
            shard_index = docs / "guide" / "index.md"
            shard_index.parent.mkdir()
            shard_index.write_text("# Guide shards\n\n- [Overview](overview.md)\n", encoding="utf-8")

            index_output = run_cmd(
                "artifact",
                "doc-index",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/doc-index-generated.md",
                "--target-docs",
                "docs",
                "--indexed-docs",
                "docs/guide.md",
                "--source-of-truth",
                "docs/guide.md",
                "--navigation-rules",
                "read docs/guide.md first, then follow linked shards",
                "--changes-or-findings",
                "guide.md is the current owner of product navigation",
                "--stale-or-duplicate-notes",
                "no duplicate owner found",
                "--stale-check",
                "source hash and mtime verified by artifact doc-check",
                "--next-workflow",
                "editorial-review",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/doc-index-generated.md", index_output)
            self.assertIn("Document utility check passed.", index_output)
            index_text = (root / ".forge-method" / "artifacts" / "doc-index-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: doc-index", index_text)
            self.assertIn(f"source_fingerprint: {sha256(source)}", index_text)
            self.assertIn("source_last_modified: docs/guide.md=", index_text)
            self.assertIn("validation: artifact doc-check --path .forge-method/artifacts/doc-index-generated.md", index_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-doc-index-generated-md-exists.yaml").exists())

            shard_output = run_cmd(
                "artifact",
                "doc-shard",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/doc-shard-generated.md",
                "--target-docs",
                "docs/guide.md",
                "--source-of-truth",
                "docs/guide.md",
                "--generated-or-derived-docs",
                "docs/guide/index.md",
                "--shard-index",
                "docs/guide/index.md",
                "--original-doc-decision",
                "keep",
                "--precedence-rule",
                "whole source document wins until archive decision",
                "--changes-or-findings",
                "created shard index for future context loading",
                "--stale-or-duplicate-notes",
                "original retained with explicit waiver",
                "--stale-check",
                "source hash and shard index verified",
                "--stale-waiver",
                "owner keeps whole source for review while shards prove navigation",
                "--next-workflow",
                "doc-index",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/doc-shard-generated.md", shard_output)
            self.assertIn("Document utility check passed.", shard_output)
            shard_text = (root / ".forge-method" / "artifacts" / "doc-shard-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: doc-shard", shard_text)
            self.assertIn("stale_waiver: owner keeps whole source for review while shards prove navigation", shard_text)
            self.assertIn("validation: artifact doc-check --path .forge-method/artifacts/doc-shard-generated.md", shard_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-doc-shard-generated-md-exists.yaml").exists())
            check = run_cmd("artifact", "doc-check", "--root", str(root), "--path", ".forge-method/artifacts/doc-shard-generated.md").stdout
            self.assertIn("Document utility check passed.", check)

    def test_artifact_spec_check_validates_kernel_contract(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifacts_dir = root / ".forge-method" / "artifacts"
            spec_artifact = artifacts_dir / "spec-kernel.md"
            spec_artifact.write_text(
                "\n".join(
                    [
                        "# Spec Kernel Artifact",
                        "workflow: write-spec",
                        "mode: distill",
                        "spec_id: SPEC-example",
                        "source_artifacts: .forge-method/artifacts/discovery-intent.md",
                        "companions: glossary.md",
                        "absorbed_sources: discovery-intent.md",
                        "decision_log: .decision-log.md",
                        "why: Operators need one source of truth before architecture and stories.",
                        "capabilities: CAP-1 intent: user can import a source brief; success: spec-check validates the resulting kernel",
                        "constraints: Must preserve source claims and avoid implementation details in the kernel.",
                        "non_goals: Does not choose UI framework or implementation architecture.",
                        "success_signal: A future agent can create stories from CAP-1 without reading chat history.",
                        "assumptions: Source brief is authoritative until a later addendum replaces it.",
                        "open_questions: none blocking",
                        "preservation_map: source claim absorbed into CAP-1; glossary moved to companion.",
                        "validation_verdict: coherent and preservation-complete",
                        "next_workflow: product-requirements",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            passed = run_cmd("artifact", "spec-check", "--root", str(root), "--path", str(spec_artifact)).stdout
            self.assertIn("Spec kernel check passed.", passed)

            broken = artifacts_dir / "spec-kernel-broken.md"
            broken.write_text(spec_artifact.read_text(encoding="utf-8").replace("non_goals: Does not choose UI framework or implementation architecture.", "non_goals: none"), encoding="utf-8")
            result = run_cmd("artifact", "spec-check", "--root", str(root), "--path", str(broken), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("non_goals must be explicit", result.stdout)

    def test_artifact_verify_snapshot_and_gate_run_semantic_artifact_checks(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            bad_spec = root / ".forge-method" / "artifacts" / "bad-spec.md"
            bad_spec.write_text(
                "\n".join(
                    [
                        "# Bad Spec",
                        "workflow: write-spec",
                        "non_goals: none",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "spec",
                "--title",
                "Bad Spec",
                "--summary",
                "Malformed spec artifact should be caught by semantic validation.",
                "--path",
                ".forge-method/artifacts/bad-spec.md",
            )

            verify = run_cmd("artifact", "verify", "--root", str(root), check=False)
            self.assertNotEqual(verify.returncode, 0)
            self.assertIn(".forge-method/artifacts/bad-spec.md: spec kernel requires source_artifacts", verify.stdout)

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            artifact_errors = snapshot["quality"]["artifacts"]["errors"]
            self.assertTrue(
                any("bad-spec.md: spec kernel requires source_artifacts" in error for error in artifact_errors)
            )

            gate = run_cmd("gate", "--root", str(root), check=False)
            self.assertNotEqual(gate.returncode, 0)
            self.assertIn(
                "artifact: .forge-method/artifacts/bad-spec.md: spec kernel requires source_artifacts",
                gate.stdout,
            )

    def test_artifact_spec_kernel_generates_and_validates_kernel(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            output = run_cmd(
                "artifact",
                "spec-kernel",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/spec-kernel-generated.md",
                "--source-artifacts",
                ".forge-method/artifacts/discovery-intent.md",
                "--why",
                "Operators need a compact WHAT contract before architecture and story planning.",
                "--capabilities",
                "CAP-1 intent: user can distill accepted discovery into a spec kernel; success: spec-check validates the kernel",
                "--constraints",
                "Keep the kernel compact and preserve load-bearing source claims.",
                "--non-goals",
                "Does not choose implementation architecture or create sprint stories.",
                "--success-signal",
                "A future agent can plan architecture from CAP-1 without reading chat history.",
                "--preservation-map",
                "source claim absorbed into CAP-1; bulky examples moved to companion if needed",
                "--next-workflow",
                "architecture",
                "--eval",
            ).stdout
            artifact = root / ".forge-method" / "artifacts" / "spec-kernel-generated.md"
            text = artifact.read_text(encoding="utf-8")
            check = run_cmd("artifact", "spec-check", "--root", str(root), "--path", ".forge-method/artifacts/spec-kernel-generated.md").stdout

            self.assertIn(".forge-method/artifacts/spec-kernel-generated.md", output)
            self.assertIn("Spec kernel check passed.", output)
            self.assertIn("workflow: write-spec", text)
            self.assertIn("CAP-1 intent", text)
            self.assertIn("next_workflow: architecture", text)
            self.assertIn("Spec kernel check passed.", check)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-spec-kernel-generated-md-exists.yaml").exists())

    def test_artifact_research_check_validates_scan_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifacts_dir = root / ".forge-method" / "artifacts"
            market_artifact = artifacts_dir / "market-scan-proof.md"
            market_artifact.write_text(
                "\n".join(
                    [
                        "# Research Scan Artifact",
                        "workflow: market-scan",
                        "mode: market",
                        "research_question: Would teams switch from spreadsheets for this workflow?",
                        "decision_to_unlock: decide whether this idea deserves PRD scope",
                        "claim: Teams have adoption pain worth solving.",
                        "sources: primary interviews, competitor docs, pricing pages",
                        "source_gaps: no paid analyst report available",
                        "evidence_grade: recency current, authority mixed, directness high, bias noted",
                        "findings: alternatives exist but switching cost is high.",
                        "contradictions_or_falsifiers: if interviews show no switching pain, shrink scope.",
                        "uncertainty: pricing willingness remains weak.",
                        "stance: continue to PRD with adoption risk explicit",
                        "alternatives: spreadsheets, generic task tools, incumbent SaaS",
                        "adoption_friction: migration cost and trust barrier",
                        "demand_signal: repeated manual workaround in interviews",
                        "validation: artifact research-check --path .forge-method/artifacts/market-scan-proof.md",
                        "next_workflow: research-closeout",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            passed = run_cmd("artifact", "research-check", "--root", str(root), "--path", str(market_artifact)).stdout
            self.assertIn("Research scan check passed.", passed)

            broken = artifacts_dir / "market-scan-broken.md"
            broken.write_text(
                market_artifact.read_text(encoding="utf-8").replace(
                    "contradictions_or_falsifiers: if interviews show no switching pain, shrink scope.",
                    "contradictions_or_falsifiers: none",
                ),
                encoding="utf-8",
            )
            result = run_cmd("artifact", "research-check", "--root", str(root), "--path", str(broken), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("contradictions_or_falsifiers must name", result.stdout)

    def test_artifact_research_scan_generates_market_domain_and_technical_scans(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            market = run_cmd(
                "artifact",
                "research-scan",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/market-scan-generated.md",
                "--workflow",
                "market-scan",
                "--research-question",
                "Would teams switch from spreadsheets for this workflow?",
                "--decision-to-unlock",
                "decide whether this idea deserves PRD scope",
                "--claim",
                "Teams have adoption pain worth solving.",
                "--sources",
                "primary interviews, competitor docs, pricing pages",
                "--source-gaps",
                "no paid analyst report available",
                "--evidence-grade",
                "recency current, authority mixed, directness high, bias noted",
                "--findings",
                "alternatives exist but switching cost is high.",
                "--contradictions-or-falsifiers",
                "if interviews show no switching pain, shrink scope.",
                "--uncertainty",
                "pricing willingness remains weak.",
                "--stance",
                "continue to PRD with adoption risk explicit",
                "--alternatives",
                "spreadsheets, generic task tools, incumbent SaaS",
                "--adoption-friction",
                "migration cost and trust barrier",
                "--demand-signal",
                "repeated manual workaround in interviews",
                "--next-workflow",
                "research-closeout",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/market-scan-generated.md", market)
            self.assertIn("Research scan check passed.", market)
            market_text = (root / ".forge-method" / "artifacts" / "market-scan-generated.md").read_text(encoding="utf-8")
            self.assertIn("mode: market", market_text)
            self.assertIn("validation: artifact research-check --path .forge-method/artifacts/market-scan-generated.md", market_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-market-scan-generated-md-exists.yaml").exists())

            domain = run_cmd(
                "artifact",
                "research-scan",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/domain-scan-generated.md",
                "--workflow",
                "domain-scan",
                "--research-question",
                "Which domain constraints shape safe use?",
                "--decision-to-unlock",
                "decide whether expert review blocks product requirements",
                "--claim",
                "The workflow can be supported if domain duties stay explicit.",
                "--sources",
                "primary policy docs, expert notes, operator transcript",
                "--source-gaps",
                "no formal legal opinion in this pass",
                "--evidence-grade",
                "recency current, authority high, directness medium, bias noted",
                "--findings",
                "domain duties constrain automation and require review checkpoints.",
                "--contradictions-or-falsifiers",
                "if the rules forbid private use, block downstream planning.",
                "--uncertainty",
                "edge cases need qualified review before release.",
                "--stance",
                "continue with review needs explicit",
                "--domain-constraints",
                "privacy duties, consent boundaries, and source material limits",
                "--risks-or-harms",
                "incorrect advice, leakage, and misplaced trust",
                "--expert-review-needed",
                "qualified review required before public release",
                "--next-workflow",
                "research-closeout",
            ).stdout
            self.assertIn("Research scan check passed.", domain)
            domain_text = (root / ".forge-method" / "artifacts" / "domain-scan-generated.md").read_text(encoding="utf-8")
            self.assertIn("mode: domain", domain_text)
            self.assertIn("expert_review_needed: qualified review required before public release", domain_text)

            technical = run_cmd(
                "artifact",
                "research-scan",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/technical-scan-generated.md",
                "--workflow",
                "technical-feasibility-scan",
                "--research-question",
                "Can the riskiest automation promise be proven cheaply?",
                "--decision-to-unlock",
                "decide whether architecture should include a prototype slice",
                "--claim",
                "The core automation is technically plausible with bounded inputs.",
                "--sources",
                "official API docs, prototype notes, vendor limits",
                "--source-gaps",
                "no scale benchmark yet",
                "--evidence-grade",
                "recency current, authority high, directness high, bias noted",
                "--findings",
                "the tools support the narrow flow but not broad unattended automation.",
                "--contradictions-or-falsifiers",
                "if the API cannot preserve citations, shrink the automation scope.",
                "--uncertainty",
                "latency and cost remain unproven.",
                "--stance",
                "continue to prototype the narrow proof path",
                "--feasibility-stance",
                "plausible for a bounded prototype",
                "--riskiest-unknowns",
                "latency, citation fidelity, and failure recovery",
                "--proof-path",
                "build a fixture replay against official API limits",
                "--next-workflow",
                "quick-prototype",
            ).stdout
            self.assertIn("Research scan check passed.", technical)
            technical_text = (root / ".forge-method" / "artifacts" / "technical-scan-generated.md").read_text(encoding="utf-8")
            self.assertIn("mode: technical", technical_text)
            self.assertIn("next_workflow: quick-prototype", technical_text)
            check = run_cmd(
                "artifact",
                "research-check",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/technical-scan-generated.md",
            ).stdout
            self.assertIn("Research scan check passed.", check)

    def test_artifact_test_check_validates_test_automation_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifacts_dir = root / ".forge-method" / "artifacts"

            framework_artifact = artifacts_dir / "test-framework-proof.md"
            framework_artifact.write_text(
                "\n".join(
                    [
                        "# Test Framework Artifact",
                        "workflow: test-framework",
                        "detected_framework: Playwright",
                        "framework_detection: package.json has @playwright/test and playwright.config.ts",
                        "pure_helpers: data builders for users and orders",
                        "framework_wrappers: Playwright fixtures wrap login/session setup",
                        "composition_surface: test.extend composes auth, page objects, and seeded data",
                        "cleanup_lifecycle: per-test database cleanup after assertions",
                        "command_contract: npm run test:e2e",
                        "commands: npm run test:e2e",
                        "evidence_links: .forge-method/evidence/playwright-run.md",
                        "failure_repair_policy: fix flaky setup or assertions before widening coverage",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            framework_pass = run_cmd("artifact", "test-check", "--root", str(root), "--path", str(framework_artifact))
            self.assertIn("Test utility check passed.", framework_pass.stdout)

            automation_artifact = artifacts_dir / "test-automation-proof.md"
            automation_artifact.write_text(
                "\n".join(
                    [
                        "# Test Automation Artifact",
                        "workflow: test-automation",
                        "selected_scenarios: checkout success, payment decline, cart recovery",
                        "risk_priority: checkout revenue path first",
                        "api_checks: create cart and payment intent contract checks",
                        "e2e_workflows: browser checkout with saved card and visible receipt",
                        "semantic_locator_policy: roles, labels, and visible text",
                        "visible_outcome_assertions: receipt heading and order id are visible",
                        "independent_test_policy: each scenario creates its own data",
                        "no_hardcoded_waits: true",
                        "run_and_fix_result: npm run test:e2e passed after selector repair",
                        "commands: npm run test:e2e",
                        "evidence_links: .forge-method/evidence/e2e-run.md",
                        "failure_repair_policy: repair failing test or record waiver before gate",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            automation_pass = run_cmd("artifact", "test-check", "--root", str(root), "--path", str(automation_artifact))
            self.assertIn("Test utility check passed.", automation_pass.stdout)

            automation_artifact.write_text(
                automation_artifact.read_text(encoding="utf-8").replace(
                    "no_hardcoded_waits: true", "no_hardcoded_waits: false"
                ),
                encoding="utf-8",
            )
            automation_fail = run_cmd(
                "artifact", "test-check", "--root", str(root), "--path", str(automation_artifact), check=False
            )
            self.assertNotEqual(automation_fail.returncode, 0)
            self.assertIn("no_hardcoded_waits must reject sleeps or document a waiver", automation_fail.stdout)

            game_e2e_artifact = artifacts_dir / "game-e2e-proof.md"
            game_e2e_artifact.write_text(
                "\n".join(
                    [
                        "# Game E2E Artifact",
                        "workflow: game-e2e-scaffold",
                        "launch_command: npm run game:test",
                        "setup_action_assertion_teardown: launch scene, start encounter, assert win banner, reset save",
                        "observable_success_signal: win banner and score event are captured",
                        "evidence_mode: screenshot plus command log",
                        "commands: npm run game:test",
                        "evidence_links: .forge-method/evidence/game-e2e.md",
                        "release_gate_link: release-readiness playable smoke gate",
                        "failure_repair_policy: fix launch/action/assertion before marking readiness",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            game_pass = run_cmd("artifact", "test-check", "--root", str(root), "--path", str(game_e2e_artifact))
            self.assertIn("Test utility check passed.", game_pass.stdout)

    def test_artifact_test_generators_create_framework_automation_and_game_e2e(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            framework = run_cmd(
                "artifact",
                "test-framework",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/test-framework-generated.md",
                "--stack",
                "TypeScript web app",
                "--detected-framework",
                "Playwright",
                "--framework-detection",
                "package.json has @playwright/test and playwright.config.ts",
                "--package-or-config-files",
                "package.json, playwright.config.ts",
                "--test-levels",
                "unit, API, E2E",
                "--fixture-architecture",
                "pure builders feed Playwright fixtures",
                "--pure-helpers",
                "data builders for users and orders",
                "--framework-wrappers",
                "Playwright fixtures wrap login/session setup",
                "--composition-surface",
                "test.extend composes auth, page objects, and seeded data",
                "--cleanup-lifecycle",
                "per-test database cleanup after assertions",
                "--data-strategy",
                "seed isolated records per scenario",
                "--semantic-locator-policy",
                "roles, labels, and visible text",
                "--command-contract",
                "npm run test:e2e",
                "--commands",
                "npm run test:e2e",
                "--first-checks",
                "checkout smoke and payment decline",
                "--evidence-links",
                ".forge-method/evidence/playwright-run.md",
                "--failure-repair-policy",
                "fix flaky setup or assertions before widening coverage",
                "--maintenance-rules",
                "keep helpers pure and wrappers thin",
                "--limitations",
                "visual diffs remain manual",
                "--next-workflow",
                "test-automation",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/test-framework-generated.md", framework)
            self.assertIn("Test utility check passed.", framework)
            framework_text = (root / ".forge-method" / "artifacts" / "test-framework-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: test-framework", framework_text)
            self.assertIn("validation: artifact test-check --path .forge-method/artifacts/test-framework-generated.md", framework_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-test-framework-generated-md-exists.yaml").exists())

            automation = run_cmd(
                "artifact",
                "test-automation",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/test-automation-generated.md",
                "--framework",
                "Playwright",
                "--target-behaviors",
                "checkout success and payment decline",
                "--selected-scenarios",
                "checkout success, payment decline, cart recovery",
                "--risk-reason",
                "checkout is the revenue path",
                "--risk-priority",
                "checkout revenue path first",
                "--test-level",
                "API plus browser E2E",
                "--api-checks",
                "create cart and payment intent contract checks",
                "--e2e-workflows",
                "browser checkout with saved card and visible receipt",
                "--fixtures",
                "seeded user, cart, payment method",
                "--data-setup",
                "fresh cart per test",
                "--semantic-locator-policy",
                "roles, labels, and visible text",
                "--assertions",
                "receipt heading and order id",
                "--visible-outcome-assertions",
                "receipt heading and order id are visible",
                "--independent-test-policy",
                "each scenario creates its own data",
                "--no-hardcoded-waits",
                "true",
                "--commands",
                "npm run test:e2e",
                "--evidence-links",
                ".forge-method/evidence/e2e-run.md",
                "--run-and-fix-result",
                "npm run test:e2e passed after selector repair",
                "--failure-repair-policy",
                "repair failing test or record waiver before gate",
                "--manual-remainders",
                "visual polish remains manual",
                "--gate-impact",
                "release gate consumes E2E evidence",
                "--next-workflow",
                "test-review",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/test-automation-generated.md", automation)
            self.assertIn("Test utility check passed.", automation)
            automation_text = (root / ".forge-method" / "artifacts" / "test-automation-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: test-automation", automation_text)
            self.assertIn("no_hardcoded_waits: true", automation_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-test-automation-generated-md-exists.yaml").exists())

            game_e2e = run_cmd(
                "artifact",
                "game-e2e-scaffold",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/game-e2e-generated.md",
                "--playable-slice",
                "first arena encounter",
                "--engine-profile",
                "browser canvas with deterministic seed",
                "--launch-command",
                "npm run game:test",
                "--smoke-path",
                "launch scene, start encounter, win, reset save",
                "--setup-action-assertion-teardown",
                "launch scene, start encounter, assert win banner, reset save",
                "--observable-success-signal",
                "win banner and score event are captured",
                "--evidence-mode",
                "screenshot plus command log",
                "--commands",
                "npm run game:test",
                "--evidence-links",
                ".forge-method/evidence/game-e2e.md",
                "--release-gate-link",
                "release-readiness playable smoke gate",
                "--failure-repair-policy",
                "fix launch/action/assertion before marking readiness",
                "--manual-remainders",
                "feel tuning remains playtest",
                "--next-workflow",
                "game-qa-review",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/game-e2e-generated.md", game_e2e)
            self.assertIn("Test utility check passed.", game_e2e)
            game_e2e_text = (root / ".forge-method" / "artifacts" / "game-e2e-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: game-e2e-scaffold", game_e2e_text)
            self.assertIn("release_gate_link: release-readiness playable smoke gate", game_e2e_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-game-e2e-generated-md-exists.yaml").exists())
            check = run_cmd("artifact", "test-check", "--root", str(root), "--path", ".forge-method/artifacts/game-e2e-generated.md").stdout
            self.assertIn("Test utility check passed.", check)

    def test_artifact_game_check_validates_brief_and_sprint_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifacts_dir = root / ".forge-method" / "artifacts"
            brief_artifact = artifacts_dir / "game-brief-proof.md"
            brief_artifact.write_text(
                "\n".join(
                    [
                        "# Game Brief Artifact",
                        "workflow: game-brief",
                        "mode: validate",
                        "source_material: discovery transcript and reference notes",
                        "player_fantasy: Be a tactical GM running a living tabletop battle.",
                        "core_loop: prepare scene, adjudicate player decisions, roll outcomes, reveal consequences, earn campaign progress",
                        "player_verbs: prepare, place, roll, adjudicate, reveal",
                        "target_player: remote tabletop RPG group and GM",
                        "platform_or_engine: browser-first web app",
                        "pillars: fast table flow, cited rules support, GM control",
                        "references: Foundry, Fantasy Grounds, tabletop maps",
                        "first_visual_preview: table scene with map, initiative, character sheet, and rules citation side panel",
                        "mvp_playable_proof: one GM hosts a scene and resolves one cited rules interaction",
                        "dream_game: every sourcebook becomes a reviewed rules assistant",
                        "vertical_slice: one system, one map, one combat turn",
                        "playable_slice: GM can host a room and resolve one turn",
                        "parked_scope: universal book ingestion and full automation",
                        "rejected_directions: clone every VTT feature before rules proof",
                        "decision_log: .forge-method/artifacts/game-brief-decisions.md",
                        "assumptions: private legal source use only",
                        "open_questions: first open-license system",
                        "research_needed: licensing and technical feasibility",
                        "validation: artifact game-check --path .forge-method/artifacts/game-brief-proof.md",
                        "validation_verdict: coherent living brief",
                        "next_workflow: game-sprint-planning",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            brief_pass = run_cmd("artifact", "game-check", "--root", str(root), "--path", str(brief_artifact))
            self.assertIn("Game artifact check passed.", brief_pass.stdout)
            self.assertIn("legacy game artifact has no mda_trace", brief_pass.stdout)

            sprint_artifact = artifacts_dir / "game-sprint-plan-proof.md"
            sprint_artifact.write_text(
                "\n".join(
                    [
                        "# Game Sprint Plan Artifact",
                        "workflow: game-sprint-planning",
                        "mode: plan",
                        "source_material: .forge-method/artifacts/game-brief-proof.md",
                        "player_fantasy: Be a tactical GM running a living tabletop battle.",
                        "playable_slice: GM can host a room and resolve one turn",
                        "playable_slice_goal: first playable remote table scene",
                        "decision_sources: game brief, prototype notes, engine setup",
                        "story_batch: room setup, map placement, dice outcome, rules citation",
                        "player_value_order: host room before rules citation polish",
                        "risk_order: realtime state before visual polish",
                        "dependencies: room state before dice outcome",
                        "engine_or_asset_constraints: browser canvas with placeholder tokens",
                        "validation_plan: manual playtest plus smoke command",
                        "manual_playtest_plan: GM creates scene and resolves one turn",
                        "deferred_scope: universal sourcebook ingestion",
                        "blocked_items: none blocking",
                        "next_story: story-room-setup",
                        "sprint_update: set active slice sprint with first story ready",
                        "validation: artifact game-check --path .forge-method/artifacts/game-sprint-plan-proof.md",
                        "next_workflow: game-story-creation",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            sprint_pass = run_cmd("artifact", "game-check", "--root", str(root), "--path", str(sprint_artifact))
            self.assertIn("Game artifact check passed.", sprint_pass.stdout)

            broken = artifacts_dir / "game-brief-broken.md"
            broken.write_text(
                brief_artifact.read_text(encoding="utf-8").replace(
                    "mvp_playable_proof: one GM hosts a scene and resolves one cited rules interaction",
                    "mvp_playable_proof: none",
                ),
                encoding="utf-8",
            )
            result = run_cmd("artifact", "game-check", "--root", str(root), "--path", str(broken), check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("mvp_playable_proof must name", result.stdout)

            broken_mda = artifacts_dir / "game-brief-broken-mda.md"
            broken_mda.write_text(
                brief_artifact.read_text(encoding="utf-8").replace(
                    "references: Foundry, Fantasy Grounds, tabletop maps",
                    "\n".join(
                        [
                            "mda_trace:",
                            "  target_aesthetics: tactical tension and GM confidence",
                            "  player_experience_hypothesis:",
                            "  desired_dynamics: deliberate turn tradeoffs",
                            "  supporting_mechanics: initiative, movement, dice, cited rules",
                            "  feedback_and_ui_signals: map highlights and citation panel",
                            "  proof_or_playtest: one GM resolves one turn with players",
                            "  unresolved_risks: licensing and automation limits",
                            "references: Foundry, Fantasy Grounds, tabletop maps",
                        ]
                    ),
                ),
                encoding="utf-8",
            )
            mda_result = run_cmd("artifact", "game-check", "--root", str(root), "--path", str(broken_mda), check=False)
            self.assertNotEqual(mda_result.returncode, 0)
            self.assertIn("mda_trace.player_experience_hypothesis is required", mda_result.stdout)

    def test_artifact_game_generators_create_brief_and_sprint_plan(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            brief = run_cmd(
                "artifact",
                "game-brief",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/game-brief-generated.md",
                "--source-material",
                "discovery transcript and reference notes",
                "--player-fantasy",
                "Be a tactical GM running a living tabletop battle.",
                "--core-loop",
                "prepare scene, adjudicate player decisions, roll outcomes, reveal consequences, earn campaign progress",
                "--player-verbs",
                "prepare, place, roll, adjudicate, reveal",
                "--target-player",
                "remote tabletop RPG group and GM",
                "--platform-or-engine",
                "browser-first web app",
                "--pillars",
                "fast table flow, cited rules support, GM control",
                "--references",
                "Foundry, Fantasy Grounds, tabletop maps",
                "--mvp-playable-proof",
                "one GM hosts a scene and resolves one cited rules interaction",
                "--dream-game",
                "every sourcebook becomes a reviewed rules assistant",
                "--vertical-slice",
                "one system, one map, one combat turn",
                "--playable-slice",
                "GM can host a room and resolve one turn",
                "--parked-scope",
                "universal sourcebook ingestion and full automation",
                "--rejected-directions",
                "clone every VTT feature before rules proof",
                "--decision-log",
                ".forge-method/artifacts/game-brief-decisions.md",
                "--assumptions",
                "private legal source use only",
                "--open-questions",
                "first open-license system",
                "--research-needed",
                "licensing and technical feasibility",
                "--next-workflow",
                "game-sprint-planning",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/game-brief-generated.md", brief)
            self.assertIn("Game artifact check passed.", brief)
            brief_text = (root / ".forge-method" / "artifacts" / "game-brief-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: game-brief", brief_text)
            self.assertIn("mda_trace:", brief_text)
            self.assertIn("target_aesthetics: Be a tactical GM running a living tabletop battle.", brief_text)
            self.assertIn("desired_dynamics: prepare scene, adjudicate player decisions, roll outcomes, reveal consequences, earn campaign progress", brief_text)
            self.assertIn("proof_or_playtest: one GM hosts a scene and resolves one cited rules interaction", brief_text)
            self.assertIn("validation: artifact game-check --path .forge-method/artifacts/game-brief-generated.md", brief_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-game-brief-generated-md-exists.yaml").exists())
            check_brief = run_cmd("artifact", "game-check", "--root", str(root), "--path", ".forge-method/artifacts/game-brief-generated.md").stdout
            self.assertIn("Game artifact check passed.", check_brief)
            self.assertNotIn("legacy game artifact has no mda_trace", check_brief)
            verify_brief = run_cmd("artifact", "verify", "--root", str(root)).stdout
            self.assertIn("Artifact verification passed.", verify_brief)

            sprint = run_cmd(
                "artifact",
                "game-sprint-plan",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/game-sprint-plan-generated.md",
                "--source-material",
                ".forge-method/artifacts/game-brief-generated.md",
                "--player-fantasy",
                "Be a tactical GM running a living tabletop battle.",
                "--playable-slice",
                "GM can host a room and resolve one turn",
                "--playable-slice-goal",
                "first playable remote table scene",
                "--decision-sources",
                "game brief, prototype notes, engine setup",
                "--story-batch",
                "room setup, map placement, dice outcome, rules citation",
                "--player-value-order",
                "host room before rules citation polish",
                "--risk-order",
                "realtime state before visual polish",
                "--dependencies",
                "room state before dice outcome",
                "--engine-or-asset-constraints",
                "browser canvas with placeholder tokens",
                "--validation-plan",
                "manual playtest plus smoke command",
                "--manual-playtest-plan",
                "GM creates scene and resolves one turn",
                "--deferred-scope",
                "universal sourcebook ingestion",
                "--blocked-items",
                "none blocking",
                "--next-story",
                "story-room-setup",
                "--sprint-update",
                "set active slice sprint with first story ready",
                "--next-workflow",
                "game-story-creation",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/game-sprint-plan-generated.md", sprint)
            self.assertIn("Game artifact check passed.", sprint)
            sprint_text = (root / ".forge-method" / "artifacts" / "game-sprint-plan-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: game-sprint-planning", sprint_text)
            self.assertIn("next_workflow: game-story-creation", sprint_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-game-sprint-plan-generated-md-exists.yaml").exists())
            check_sprint = run_cmd("artifact", "game-check", "--root", str(root), "--path", ".forge-method/artifacts/game-sprint-plan-generated.md").stdout
            self.assertIn("Game artifact check passed.", check_sprint)

    def test_artifact_enterprise_check_validates_track_and_readiness_maps(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            artifacts_dir = root / ".forge-method" / "artifacts"
            required = (
                "risk-register, security-plan, privacy-data-plan, test-strategy, ci-quality-pipeline, "
                "nfr-evidence-audit, traceability-gate, release-readiness"
            )
            conditional = "devops-deployment-plan when deployment matters, compliance-checklist when obligations exist, observability-plan before operate"

            track_artifact = artifacts_dir / "enterprise-track-map.md"
            track_artifact.write_text(
                "\n".join(
                    [
                        "# Track Decision Artifact",
                        "workflow: track-decision",
                        "selected_track: enterprise",
                        "selected_module: test-architect",
                        f"track_required_artifacts: {required}",
                        f"enterprise_required_artifacts: {required}",
                        f"enterprise_conditional_artifacts: {conditional}",
                        "artifact_evidence_map: each artifact names evidence, owner, gate consumer, and path",
                        "readiness_gate: readiness-check then traceability-gate and release-readiness",
                        "waiver_policy: owner, rationale, revisit trigger, and release impact required",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            track_pass = run_cmd("artifact", "enterprise-check", "--root", str(root), "--path", str(track_artifact))
            self.assertIn("Enterprise artifact check passed.", track_pass.stdout)

            readiness_artifact = artifacts_dir / "enterprise-readiness.md"
            readiness_artifact.write_text(
                "\n".join(
                    [
                        "# Readiness Matrix Artifact",
                        "workflow: readiness-check",
                        "scope: enterprise checkout release",
                        "selected_track: enterprise",
                        f"track_required_artifacts: {required}",
                        f"enterprise_required_artifacts: {required}",
                        f"enterprise_conditional_artifacts: {conditional}",
                        "enterprise_evidence_status: security/privacy/quality evidence present, compliance waived with owner",
                        "nfr_evidence: nfr-evidence-audit linked to thresholds and release claims",
                        "release_gate_impact: traceability-gate blocks release on missing P0 evidence",
                        "waivers: compliance-checklist waived by owner until SOC2 scope starts",
                        "missing_or_weak_sources: none blocking; conditional observability deferred to operate",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            readiness_pass = run_cmd("artifact", "enterprise-check", "--root", str(root), "--path", str(readiness_artifact))
            self.assertIn("Enterprise artifact check passed.", readiness_pass.stdout)

            broken = artifacts_dir / "enterprise-broken.md"
            broken.write_text(track_artifact.read_text(encoding="utf-8").replace("privacy-data-plan, ", ""), encoding="utf-8")
            broken_result = run_cmd("artifact", "enterprise-check", "--root", str(root), "--path", str(broken), check=False)
            self.assertNotEqual(broken_result.returncode, 0)
            self.assertIn("enterprise required artifacts must include privacy-data-plan", broken_result.stdout)

    def test_artifact_enterprise_generators_create_track_readiness_and_release_gates(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            track_output = run_cmd(
                "artifact",
                "enterprise-track-map",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/enterprise-track-generated.md",
                "--selected-module",
                "software-builder",
                "--scope",
                "enterprise checkout release",
                "--artifact-evidence-map",
                "each required artifact names owner, evidence path, gate consumer, and waiver status",
                "--readiness-gate",
                "readiness-check then traceability-gate and release-readiness",
                "--waiver-policy",
                "waiver owner, rationale, revisit trigger, and release impact required",
                "--next-workflow",
                "readiness-check",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/enterprise-track-generated.md", track_output)
            self.assertIn("Enterprise artifact check passed.", track_output)
            track_text = (root / ".forge-method" / "artifacts" / "enterprise-track-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: track-decision", track_text)
            self.assertIn("enterprise_required_artifacts: risk-register, security-plan, privacy-data-plan", track_text)
            self.assertIn("validation: artifact enterprise-check --path .forge-method/artifacts/enterprise-track-generated.md", track_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-enterprise-track-generated-md-exists.yaml").exists())

            readiness_output = run_cmd(
                "artifact",
                "enterprise-readiness",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/enterprise-readiness-generated.md",
                "--scope",
                "enterprise checkout release",
                "--enterprise-evidence-status",
                "security privacy risk quality NFR and traceability evidence present",
                "--nfr-evidence",
                "nfr-evidence-audit links thresholds to release claims",
                "--release-gate-impact",
                "missing P0 evidence blocks release",
                "--waivers",
                "compliance-checklist waived by owner until SOC2 scope starts",
                "--missing-or-weak-sources",
                "none blocking; observability details continue before operate",
                "--next-workflow",
                "traceability-gate",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/enterprise-readiness-generated.md", readiness_output)
            self.assertIn("Enterprise artifact check passed.", readiness_output)
            readiness_text = (root / ".forge-method" / "artifacts" / "enterprise-readiness-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: readiness-check", readiness_text)
            self.assertIn("enterprise_required_artifacts: risk-register, security-plan, privacy-data-plan", readiness_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-enterprise-readiness-generated-md-exists.yaml").exists())

            release_output = run_cmd(
                "artifact",
                "enterprise-release-gate",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/enterprise-release-generated.md",
                "--scope",
                "enterprise checkout release",
                "--enterprise-evidence-status",
                "required evidence passed, compliance waiver accepted with owner",
                "--gate-decision",
                "hold until traceability evidence is attached",
                "--release-gate-impact",
                "release is blocked if traceability evidence remains missing",
                "--waivers",
                "compliance-checklist waived by owner with revisit date",
                "--next-workflow",
                "ready-release",
                "--eval",
            ).stdout
            self.assertIn(".forge-method/artifacts/enterprise-release-generated.md", release_output)
            self.assertIn("Enterprise artifact check passed.", release_output)
            release_text = (root / ".forge-method" / "artifacts" / "enterprise-release-generated.md").read_text(encoding="utf-8")
            self.assertIn("workflow: release-readiness", release_text)
            self.assertIn("gate_decision: hold until traceability evidence is attached", release_text)
            self.assertIn("validation: artifact enterprise-check --path .forge-method/artifacts/enterprise-release-generated.md", release_text)
            self.assertTrue((root / ".forge-method" / "evals" / "artifact-forge-method-artifacts-enterprise-release-generated-md-exists.yaml").exists())
            check = run_cmd("artifact", "enterprise-check", "--root", str(root), "--path", ".forge-method/artifacts/enterprise-release-generated.md").stdout
            self.assertIn("Enterprise artifact check passed.", check)

    def test_facilitation_specificity_guard_rejects_generic_packs(self) -> None:
        runtime = load_runtime_module()
        base = """# facilitation: sample

purpose:
  Shape a human-facing workflow.

open_floor:
  "What are we trying to improve?"

source_material:
  Ask for transcript, state, artifacts, and proof.

follow_up_batches:
  - behavior: "What should change?"
  - proof: "What catches regression?"

conversation_stages:
  - frame: "Name the situation."
  - handoff: "Persist the compact result."

elicitation_options:
  - contrast: "Compare two paths."
  - probe: "Pick one reversible probe."

facilitator_moves:
  - "Guide the human."
  - "Preserve the agent contract."

quality_bar:
  - "The human gets a useful next move."
  - "The agent gets a compact handoff."

anti_patterns:
  - "Do not dump a catalog."
  - "Do not rely on chat memory."

paths:
  fast_path: "Patch and prove."
  deep_path: "Plan, grill, patch, and prove."

checkpoint_options:
  - continue
  - correct-course

"""
        suffix = """
artifact_rules:
  Persist behavior, proof, state impact, and next workflow.

headless:
  Continue only when proof is clear.
"""
        with tempfile.TemporaryDirectory() as raw:
            pack = Path(raw) / "sample.md"
            pack.write_text(base + suffix, encoding="utf-8")
            missing_errors = runtime.validate_facilitation_pack(pack)
            self.assertIn("sample.md: missing facilitation section `domain_examples:`", missing_errors)
            self.assertTrue(any("too generic" in error for error in missing_errors))

            pack.write_text(
                base
                + """domain_examples:
  route_bug: "A transcript routes wrong; add replay proof."
  handoff_gap: "Agent state is unclear; add compact JSON proof."

"""
                + suffix,
                encoding="utf-8",
            )
            thin_errors = runtime.validate_facilitation_pack(pack)
            self.assertTrue(any("too generic" in error for error in thin_errors))

            pack.write_text(
                base
                + """domain_examples:
  route_bug: "A transcript routes wrong; add replay proof."
  handoff_gap: "Agent state is unclear; add compact JSON proof."
  human_gap: "The conversation feels generic; add a pack-specific guided move."

"""
                + suffix,
                encoding="utf-8",
            )
            specific_errors = runtime.validate_facilitation_pack(pack)
            self.assertFalse([error for error in specific_errors if "domain_examples" in error or "too generic" in error])

    def test_workflow_validation_errors_include_catalog_surface(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            catalog = Path(raw) / "workflows.json"
            catalog.write_text(
                json.dumps(
                    {
                        "schema_version": "forge-workflow-catalog.v1",
                        "workflows": [
                            {
                                "id": "catalog-gap",
                                "phase": "1-discovery",
                                "required": False,
                                "reference": "discover-intent",
                                "outputs": ["catalog proof"],
                                "template": "missing-template",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            original_catalog = runtime.WORKFLOW_CATALOG_PATH
            runtime.WORKFLOW_CATALOG_PATH = catalog
            try:
                errors = runtime.workflow_validation_errors()
            finally:
                runtime.WORKFLOW_CATALOG_PATH = original_catalog

        self.assertTrue(any("references missing template: missing-template" in error for error in errors))

    def test_workflow_guidance_safety_rejects_stale_agent_instructions(self) -> None:
        runtime = load_runtime_module()
        base = """# workflow: sample

trigger:
  - user asks for a sample workflow

inputs:
  - state
  - artifact

steps:
  1. {step}

outputs:
  - compact artifact

done_when:
  - artifact validates

blocked_when:
  - evidence is missing

handoff:
  - preserve artifact path and next workflow
"""
        with tempfile.TemporaryDirectory() as raw:
            workflow = Path(raw) / "workflow-sample.md"

            workflow.write_text(base.format(step="rely on chat memory before reading durable state"), encoding="utf-8")
            chat_errors = runtime.validate_workflow_file(workflow)
            self.assertTrue(any("do not rely on chat memory" in error for error in chat_errors))

            workflow.write_text(base.format(step="follow stale state guidance until the user complains"), encoding="utf-8")
            stale_errors = runtime.validate_workflow_file(workflow)
            self.assertTrue(any("do not follow stale state" in error for error in stale_errors))

            workflow.write_text(base.format(step="ask for procedural ok/continue between mechanical steps"), encoding="utf-8")
            procedural_errors = runtime.validate_workflow_file(workflow)
            self.assertTrue(any("procedural continue confirmations" in error for error in procedural_errors))

            workflow.write_text(base.format(step="dump the catalog before choosing a next workflow"), encoding="utf-8")
            catalog_errors = runtime.validate_workflow_file(workflow)
            self.assertTrue(any("do not dump workflow catalogs" in error for error in catalog_errors))

            workflow.write_text(
                base.format(step="never ask for procedural ok/continue between mechanical steps"),
                encoding="utf-8",
            )
            safe_errors = runtime.validate_workflow_file(workflow)
            self.assertFalse([error for error in safe_errors if "misleading agent guidance" in error])

            workflow.write_text(
                base.format(step="use durable state instead of chat memory when resuming"),
                encoding="utf-8",
            )
            durable_first_errors = runtime.validate_workflow_file(workflow)
            self.assertFalse([error for error in durable_first_errors if "misleading agent guidance" in error])

            workflow.write_text(
                base.format(step="use chat memory instead of durable state when resuming"),
                encoding="utf-8",
            )
            chat_first_errors = runtime.validate_workflow_file(workflow)
            self.assertTrue(any("do not rely on chat memory" in error for error in chat_first_errors))

    def test_help_oracle_guidance_safety_rejects_unsafe_runtime_output(self) -> None:
        runtime = load_runtime_module()

        bad_oracle = {
            "source": "help-oracle",
            "human_next_step": "use chat memory instead of durable state when resuming",
            "reason": "Continue the active workflow selected by durable state.",
            "stale_state_guard": "Help Oracle is derived from current state; do not follow stale chat next steps.",
            "context_boundary": {
                "do_not": [
                    "do not replay prior chat as source of truth",
                    "do not follow stale next_action when the human supplied fresh intent",
                ],
            },
        }
        self.assertTrue(
            any("do not rely on chat memory" in error for error in runtime.validate_help_oracle_safety(bad_oracle))
        )

        safe_oracle = {
            **bad_oracle,
            "human_next_step": "use durable state instead of chat memory when resuming",
        }
        self.assertEqual(runtime.validate_help_oracle_safety(safe_oracle), [])

    def test_help_oracle_outputs_pass_guidance_safety_contract(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "6-evolve",
                "--status",
                "oracle-safety",
                "--workflow",
                "runtime-builder",
                "--next-action",
                "Audit runtime help output",
                "--force",
            )

            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)

            self.assertEqual(runtime.validate_help_oracle_safety(snapshot["help_oracle"]), [])
            self.assertEqual(runtime.validate_help_oracle_safety(resume["help_oracle"]), [])
            self.assertEqual(run_cmd("audit", "--root", str(root)).returncode, 0)

    def test_audit_rejects_unsafe_help_oracle_next_action(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "6-evolve",
                "--status",
                "oracle-safety",
                "--workflow",
                "runtime-builder",
                "--next-action",
                "audit runtime help output",
                "--force",
            )
            state_path = root / ".forge-method" / "state.yaml"
            state_text = state_path.read_text(encoding="utf-8")
            state_path.write_text(
                state_text.replace(
                    'next_action: "audit runtime help output"',
                    'next_action: "use chat memory instead of durable state when resuming"',
                ),
                encoding="utf-8",
            )

            result = run_cmd("audit", "--root", str(root), check=False)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("do not rely on chat memory", result.stdout + result.stderr)

    def test_packaged_modules_and_workflows_validate(self) -> None:
        runtime = load_runtime_module()
        modules = run_cmd("module", "list").stdout
        modules_json = json.loads(run_cmd("module", "list", "--json").stdout)
        module_recommendation = json.loads(
            run_cmd("module", "recommend", "--objective", "build a web app with an API", "--json").stdout
        )
        validation = run_cmd("workflow", "validate").stdout
        compactness_text = run_cmd("workflow", "compactness").stdout
        compactness = json.loads(run_cmd("workflow", "compactness", "--json").stdout)
        workflow_list = run_cmd("workflow", "list").stdout
        with tempfile.TemporaryDirectory() as raw:
            guide = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    raw,
                    "--question",
                    "implementar workflow metadata e facilitation packs do Forge",
                    "--json",
                ).stdout
            )
        version = run_cmd("version").stdout

        self.assertIn("core-runtime", modules)
        self.assertIn("software-builder", modules)
        self.assertTrue(modules_json["modules"])
        self.assertEqual(module_recommendation["recommended"][0]["id"], "software-builder")
        self.assertIn("Workflow validation passed.", validation)
        self.assertIn("Compactness guard passed.", compactness_text)
        self.assertGreaterEqual(compactness["workflow_count"], 90)
        self.assertGreaterEqual(compactness["facilitation_pack_count"], 20)
        self.assertLessEqual(
            compactness["workflow_max"]["lines"],
            compactness["workflow_limits"]["max_lines"],
        )
        self.assertLessEqual(
            compactness["workflow_max"]["words"],
            compactness["workflow_limits"]["max_words"],
        )
        self.assertFalse(compactness["errors"])
        self.assertIn("workflow-validate", workflow_list)
        for workflow_id in [
            "game-story-creation",
            "game-sprint-planning",
            "game-context",
            "gdd",
            "narrative-design",
            "mechanics-design",
            "engine-setup",
            "engine-architecture",
            "quick-prototype",
            "playtest-plan",
            "performance-plan",
            "game-qa-review",
            "game-test-framework",
            "test-strategy",
            "test-engagement-model",
            "test-framework",
            "ci-quality-pipeline",
            "atdd-plan",
            "test-automation",
            "test-review",
            "traceability-gate",
            "teach-testing",
            "nfr-evidence-audit",
            "workflow-analyze",
            "skill-convert",
            "module-ideation",
            "agent-builder",
            "workflow-builder",
            "module-builder",
            "module-distribution",
            "module-validate",
            "doc-index",
            "spec-distillation",
            "product-requirements",
            "working-backwards-challenge",
            "ux-plan",
            "quick-dev",
            "story-creation",
            "sprint-status",
            "track-decision",
            "project-context",
            "session-prep",
            "checkpoint-preview",
            "code-review",
            "retrospective",
            "research-closeout",
            "investigation",
            "problem-solving",
            "correct-course",
            "adversarial-review",
            "team-operating-model",
            "product-area-map",
            "trunk-based-plan",
            "collaboration-handoff",
            "repo-split-plan",
        ]:
            self.assertIn(workflow_id, workflow_list)
        for template_path in [
            ROOT / "skills" / "forge-method" / "templates" / "game-lifecycle-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-brief-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-sprint-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-story-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-sprint-status-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-context-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "engine-setup-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "engine-architecture-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "quick-prototype-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "playtest-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "performance-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-qa-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-e2e-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "brainstorming-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "problem-solving-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "correct-course-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-architecture-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "teach-testing-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-strategy-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-engagement-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-framework-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "ci-quality-pipeline-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "atdd-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-automation-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "test-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "nfr-evidence-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "traceability-gate-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "risk-register.md",
            ROOT / "skills" / "forge-method" / "templates" / "security-plan.md",
            ROOT / "skills" / "forge-method" / "templates" / "privacy-data-plan.md",
            ROOT / "skills" / "forge-method" / "templates" / "devops-deployment-plan.md",
            ROOT / "skills" / "forge-method" / "templates" / "compliance-checklist.md",
            ROOT / "skills" / "forge-method" / "templates" / "observability-plan.md",
            ROOT / "skills" / "forge-method" / "templates" / "release-readiness-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "builder-utility-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "builder-factory-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "module-builder-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "module-distribution-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "module-validation-report.md",
            ROOT / "skills" / "forge-method" / "templates" / "document-utility-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "discovery-closeout-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "spec-kernel-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "research-scan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "product-requirements-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "working-backwards-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "ux-design-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "architecture-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "quick-dev-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "story-creation-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "sprint-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "sprint-status-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "build-story-work-order.md",
            ROOT / "skills" / "forge-method" / "templates" / "track-decision-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "council-decision-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "project-context-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "session-prep-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "checkpoint-preview-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "code-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "retrospective-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "readiness-matrix-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "research-closeout-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "investigation-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "editorial-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "edge-case-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "adversarial-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "context-recovery-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "design-thinking-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "innovation-strategy-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "storytelling-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "team-operating-model-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "product-area-map-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "trunk-based-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "collaboration-handoff-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "repo-split-plan-artifact.md",
        ]:
            self.assertTrue(template_path.exists())
        for pack_path in [
            ROOT / "skills" / "forge-method" / "facilitation" / "game-lifecycle.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "test-architecture.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "builder-utility.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "builder-factory.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "document-utility.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "product-planning.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "ux-design.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "architecture-planning.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "council-decision.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "context-boundary.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "brainstorming.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "evidence-research.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "design-thinking.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "innovation-strategy.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "storytelling.md",
            ROOT / "skills" / "forge-method" / "facilitation" / "collaboration.md",
        ]:
            self.assertIn("domain_examples:", pack_path.read_text(encoding="utf-8"))
        catalog = json.loads((ROOT / "skills" / "forge-method" / "catalog" / "workflows.json").read_text(encoding="utf-8"))
        required_facilitation_sections = [
            "purpose:",
            "open_floor:",
            "source_material:",
            "follow_up_batches:",
            "conversation_stages:",
            "elicitation_options:",
            "facilitator_moves:",
            "quality_bar:",
            "anti_patterns:",
            "paths:",
            "checkpoint_options:",
            "domain_examples:",
            "artifact_rules:",
            "headless:",
        ]
        pack_ids = sorted(
            {
                item["facilitation_pack"]
                for item in catalog["workflows"]
                if item.get("facilitation_pack")
            }
        )
        self.assertGreaterEqual(len(pack_ids), 10)
        self.assertEqual(compactness["facilitation_limits"]["min_domain_examples"], 3)
        for pack_id in pack_ids:
            pack_path = ROOT / "skills" / "forge-method" / "facilitation" / f"{pack_id}.md"
            pack_text = pack_path.read_text(encoding="utf-8")
            for section in required_facilitation_sections:
                self.assertIn(section, pack_text, pack_id)
            self.assertGreaterEqual(
                runtime.markdown_section_entry_count(pack_text, "domain_examples"),
                compactness["facilitation_limits"]["min_domain_examples"],
                pack_id,
            )
        human_facing_required = {
            "product-requirements",
            "working-backwards-challenge",
            "ux-plan",
            "quick-dev",
            "brainstorming",
            "problem-solving",
            "design-thinking",
            "innovation-strategy",
            "storytelling",
            "creative-session",
            "story-creation",
            "sprint-status",
            "architecture",
            "create-epics",
            "plan-sprint",
            "readiness-check",
            "investigation",
            "correct-course",
            "gdd",
            "test-strategy",
            "teach-testing",
            "test-engagement-model",
            "test-framework",
            "ci-quality-pipeline",
            "atdd-plan",
            "test-automation",
            "test-review",
            "nfr-evidence-audit",
            "traceability-gate",
            "risk-register",
            "security-plan",
            "privacy-data-plan",
            "devops-deployment-plan",
            "compliance-checklist",
            "observability-plan",
            "release-readiness",
            "security-plan",
            "module-ideation",
            "agent-builder",
            "workflow-builder",
            "module-builder",
            "module-distribution",
            "module-validate",
            "config-customization",
            "track-decision",
            "council-decision",
            "project-context",
            "session-prep",
            "checkpoint-preview",
            "code-review",
            "retrospective",
            "research-closeout",
            "doc-index",
            "doc-shard",
            "editorial-review",
            "edge-case-review",
            "adversarial-review",
            "context-recovery",
            "write-spec",
            "market-scan",
            "domain-scan",
            "technical-feasibility-scan",
            "game-context",
            "game-brief",
            "gdd",
            "narrative-design",
            "mechanics-design",
            "engine-setup",
            "engine-architecture",
            "quick-prototype",
            "game-sprint-planning",
            "playtest-plan",
            "performance-plan",
            "game-qa-review",
            "game-test-framework",
            "game-test-automation",
            "game-e2e-scaffold",
            "team-operating-model",
            "product-area-map",
            "trunk-based-plan",
            "collaboration-handoff",
            "repo-split-plan",
        }
        by_id = {item["id"]: item for item in catalog["workflows"]}
        for workflow_id in human_facing_required:
            self.assertIn("facilitation_pack", by_id[workflow_id], workflow_id)
        self.assertEqual(by_id["discover-intent"].get("template"), "discovery-closeout-artifact")
        discover_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "discover-intent.md"
        ).read_text(encoding="utf-8")
        self.assertIn("artifact discovery-closeout", discover_pack)
        self.assertIn("non_goals", discover_pack)
        self.assertIn("success_signal", discover_pack)
        self.assertIn("open_questions", discover_pack)
        product_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "product-planning.md"
        ).read_text(encoding="utf-8")
        write_spec_ref = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-write-spec.md"
        ).read_text(encoding="utf-8")
        self.assertIn("artifact spec-kernel", product_pack)
        self.assertIn("source_artifacts", product_pack)
        self.assertIn("preservation_map", product_pack)
        self.assertIn("artifact spec-kernel", write_spec_ref)
        self.assertIn("artifact spec-check", write_spec_ref)
        self.assertEqual(by_id["write-spec"].get("template"), "spec-kernel-artifact")
        self.assertEqual(by_id["market-scan"].get("template"), "research-scan-artifact")
        self.assertEqual(by_id["domain-scan"].get("template"), "research-scan-artifact")
        self.assertEqual(by_id["technical-feasibility-scan"].get("template"), "research-scan-artifact")
        self.assertEqual(by_id["product-requirements"].get("template"), "product-requirements-artifact")
        self.assertEqual(by_id["working-backwards-challenge"].get("template"), "working-backwards-artifact")
        self.assertEqual(by_id["ux-plan"].get("template"), "ux-design-artifact")
        self.assertEqual(by_id["architecture"].get("template"), "architecture-artifact")
        self.assertEqual(by_id["quick-dev"].get("template"), "quick-dev-artifact")
        self.assertEqual(by_id["brainstorming"].get("template"), "brainstorming-artifact")
        self.assertEqual(by_id["problem-solving"].get("template"), "problem-solving-artifact")
        self.assertEqual(by_id["correct-course"].get("template"), "correct-course-artifact")
        self.assertEqual(by_id["design-thinking"].get("template"), "design-thinking-artifact")
        self.assertEqual(by_id["innovation-strategy"].get("template"), "innovation-strategy-artifact")
        self.assertEqual(by_id["storytelling"].get("template"), "storytelling-artifact")
        self.assertEqual(by_id["story-creation"].get("template"), "story-creation-artifact")
        self.assertEqual(by_id["plan-sprint"].get("template"), "sprint-plan-artifact")
        self.assertEqual(by_id["sprint-status"].get("template"), "sprint-status-artifact")
        self.assertEqual(by_id["build-story"].get("template"), "build-story-work-order")
        self.assertEqual(by_id["module-ideation"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["agent-builder"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["workflow-builder"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["module-builder"].get("template"), "module-builder-artifact")
        self.assertEqual(by_id["module-distribution"].get("template"), "module-distribution-artifact")
        self.assertEqual(by_id["module-validate"].get("template"), "module-validation-report")
        self.assertEqual(by_id["config-customization"].get("template"), "config-customization-artifact")
        self.assertEqual(by_id["track-decision"].get("template"), "track-decision-artifact")
        self.assertEqual(by_id["council-decision"].get("template"), "council-decision-artifact")
        self.assertEqual(by_id["project-context"].get("template"), "project-context-artifact")
        self.assertEqual(by_id["session-prep"].get("template"), "session-prep-artifact")
        self.assertEqual(by_id["checkpoint-preview"].get("template"), "checkpoint-preview-artifact")
        self.assertEqual(by_id["code-review"].get("template"), "code-review-artifact")
        self.assertEqual(by_id["retrospective"].get("template"), "retrospective-artifact")
        self.assertEqual(by_id["readiness-check"].get("template"), "readiness-matrix-artifact")
        self.assertEqual(by_id["research-closeout"].get("template"), "research-closeout-artifact")
        self.assertEqual(by_id["doc-index"].get("template"), "document-utility-artifact")
        self.assertEqual(by_id["doc-shard"].get("template"), "document-utility-artifact")
        self.assertEqual(by_id["investigation"].get("template"), "investigation-artifact")
        self.assertEqual(by_id["editorial-review"].get("template"), "editorial-review-artifact")
        self.assertEqual(by_id["edge-case-review"].get("template"), "edge-case-review-artifact")
        self.assertEqual(by_id["adversarial-review"].get("template"), "adversarial-review-artifact")
        self.assertEqual(by_id["context-recovery"].get("template"), "context-recovery-artifact")
        self.assertEqual(by_id["test-strategy"].get("template"), "test-strategy-artifact")
        self.assertEqual(by_id["teach-testing"].get("template"), "teach-testing-artifact")
        self.assertEqual(by_id["test-engagement-model"].get("template"), "test-engagement-artifact")
        self.assertEqual(by_id["test-framework"].get("template"), "test-framework-artifact")
        self.assertEqual(by_id["ci-quality-pipeline"].get("template"), "ci-quality-pipeline-artifact")
        self.assertEqual(by_id["atdd-plan"].get("template"), "atdd-plan-artifact")
        self.assertEqual(by_id["test-automation"].get("template"), "test-automation-artifact")
        self.assertEqual(by_id["test-review"].get("template"), "test-review-artifact")
        self.assertEqual(by_id["nfr-evidence-audit"].get("template"), "nfr-evidence-artifact")
        self.assertEqual(by_id["traceability-gate"].get("template"), "traceability-gate-artifact")
        self.assertEqual(by_id["risk-register"].get("template"), "risk-register")
        self.assertEqual(by_id["security-plan"].get("template"), "security-plan")
        self.assertEqual(by_id["privacy-data-plan"].get("template"), "privacy-data-plan")
        self.assertEqual(by_id["devops-deployment-plan"].get("template"), "devops-deployment-plan")
        self.assertEqual(by_id["compliance-checklist"].get("template"), "compliance-checklist")
        self.assertEqual(by_id["observability-plan"].get("template"), "observability-plan")
        self.assertEqual(by_id["release-readiness"].get("template"), "release-readiness-artifact")
        self.assertEqual(by_id["game-context"].get("template"), "game-context-artifact")
        self.assertEqual(by_id["game-brief"].get("template"), "game-brief-artifact")
        self.assertEqual(by_id["gdd"].get("template"), "gdd")
        self.assertEqual(by_id["narrative-design"].get("template"), "narrative-bible")
        self.assertEqual(by_id["mechanics-design"].get("template"), "mechanics-matrix")
        self.assertEqual(by_id["engine-setup"].get("template"), "engine-setup-artifact")
        self.assertEqual(by_id["engine-architecture"].get("template"), "engine-architecture-artifact")
        self.assertEqual(by_id["quick-prototype"].get("template"), "quick-prototype-artifact")
        self.assertEqual(by_id["playtest-plan"].get("template"), "playtest-plan-artifact")
        self.assertEqual(by_id["performance-plan"].get("template"), "performance-plan-artifact")
        self.assertEqual(by_id["game-sprint-planning"].get("template"), "game-sprint-plan-artifact")
        self.assertEqual(by_id["game-story-creation"].get("template"), "game-story-artifact")
        self.assertEqual(by_id["game-sprint-status"].get("template"), "game-sprint-status-artifact")
        self.assertEqual(by_id["game-qa-review"].get("template"), "game-qa-review-artifact")
        self.assertEqual(by_id["team-operating-model"].get("template"), "team-operating-model-artifact")
        self.assertEqual(by_id["product-area-map"].get("template"), "product-area-map-artifact")
        self.assertEqual(by_id["trunk-based-plan"].get("template"), "trunk-based-plan-artifact")
        self.assertEqual(by_id["collaboration-handoff"].get("template"), "collaboration-handoff-artifact")
        self.assertEqual(by_id["repo-split-plan"].get("template"), "repo-split-plan-artifact")
        self.assertEqual(by_id["repo-split-plan"].get("facilitation_pack"), "collaboration")
        game_brief_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "game-brief.md"
        ).read_text(encoding="utf-8")
        game_lifecycle_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "game-lifecycle.md"
        ).read_text(encoding="utf-8")
        game_brief_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-game-brief.md"
        ).read_text(encoding="utf-8")
        game_sprint_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-game-sprint-planning.md"
        ).read_text(encoding="utf-8")
        self.assertIn("artifact game-brief", game_brief_pack)
        self.assertIn("mvp_playable_proof", game_brief_pack)
        self.assertIn("first_visual_preview", game_brief_pack)
        self.assertIn("artifact game-sprint-plan", game_lifecycle_pack)
        self.assertIn("visible slice proof", game_lifecycle_pack)
        self.assertIn("playable_slice_goal", game_sprint_workflow)
        self.assertIn("artifact game-brief", game_brief_workflow)
        self.assertIn("first_visual_preview", game_brief_workflow)
        self.assertIn("artifact game-sprint-plan", game_sprint_workflow)
        self.assertIn("validate", by_id["product-requirements"].get("modes", []))
        self.assertIn("prfaq", by_id["working-backwards-challenge"].get("modes", []))
        self.assertIn("validate", by_id["ux-plan"].get("modes", []))
        self.assertIn("tradeoff", by_id["architecture"].get("modes", []))
        self.assertIn("readiness-check", by_id["architecture"].get("followed_by", []))
        self.assertIn("distill", by_id["write-spec"].get("modes", []))
        self.assertIn("validate", by_id["write-spec"].get("modes", []))
        self.assertIn("product-requirements", by_id["write-spec"].get("followed_by", []))
        self.assertIn("visual-alignment-prototype", by_id["discover-intent"].get("followed_by", []))
        self.assertIn("visual-alignment-prototype", by_id["write-spec"].get("followed_by", []))
        self.assertIn("visual-alignment-prototype", by_id["brainstorming"].get("followed_by", []))
        self.assertIn("visual-alignment-prototype", by_id["game-brief"].get("followed_by", []))
        self.assertIn("visual-alignment-prototype", by_id["plan-sprint"].get("followed_by", []))
        self.assertIn("market", by_id["market-scan"].get("modes", []))
        self.assertIn("domain", by_id["domain-scan"].get("modes", []))
        self.assertIn("technical", by_id["technical-feasibility-scan"].get("modes", []))
        self.assertIn("research-closeout", by_id["market-scan"].get("followed_by", []))
        research_scan_template = (
            ROOT / "skills" / "forge-method" / "templates" / "research-scan-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("decision_to_unlock", research_scan_template)
        self.assertIn("contradictions_or_falsifiers", research_scan_template)
        self.assertIn("proof_path", research_scan_template)
        self.assertIn("visual_reference_examples", research_scan_template)
        discovery_closeout_template = (
            ROOT / "skills" / "forge-method" / "templates" / "discovery-closeout-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("source_input", discovery_closeout_template)
        self.assertIn("visible_or_operational_proof", discovery_closeout_template)
        self.assertIn("early_visual_feedback_loop", discovery_closeout_template)
        self.assertIn("grill_gate_handoff", discovery_closeout_template)
        self.assertIn("next_workflow", discovery_closeout_template)
        self.assertIn("spec-lite", by_id["quick-dev"].get("modes", []))
        self.assertIn("converge", by_id["brainstorming"].get("modes", []))
        self.assertIn("concept-selection", by_id["brainstorming"].get("followed_by", []))
        self.assertIn("root-cause", by_id["problem-solving"].get("modes", []))
        self.assertIn("probe", by_id["problem-solving"].get("modes", []))
        self.assertIn("impact", by_id["correct-course"].get("modes", []))
        self.assertIn("insert", by_id["correct-course"].get("modes", []))
        self.assertIn("prototype", by_id["design-thinking"].get("modes", []))
        self.assertIn("evidence", by_id["innovation-strategy"].get("modes", []))
        self.assertIn("payoff", by_id["storytelling"].get("modes", []))
        storytelling_template = (
            ROOT / "skills" / "forge-method" / "templates" / "storytelling-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("medium", storytelling_template)
        self.assertIn("presentation_outline", storytelling_template)
        self.assertIn("call_to_action", storytelling_template)
        self.assertIn("validate", by_id["story-creation"].get("modes", []))
        self.assertIn("rebalance", by_id["plan-sprint"].get("modes", []))
        self.assertIn("status", by_id["sprint-status"].get("modes", []))
        self.assertIn("evidence", by_id["build-story"].get("modes", []))
        self.assertIn("ideate", by_id["module-ideation"].get("modes", []))
        self.assertIn("create", by_id["agent-builder"].get("modes", []))
        self.assertIn("create", by_id["workflow-builder"].get("modes", []))
        self.assertIn("package", by_id["module-builder"].get("modes", []))
        self.assertIn("plugin", by_id["module-distribution"].get("modes", []))
        self.assertIn("standalone", by_id["module-distribution"].get("modes", []))
        self.assertIn("validate", by_id["module-validate"].get("modes", []))
        module_distribution_template = (
            ROOT / "skills" / "forge-method" / "templates" / "module-distribution-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("shared_config", module_distribution_template)
        self.assertIn("anti_stale_registration", module_distribution_template)
        self.assertIn("legacy_cleanup", module_distribution_template)
        track_decision_template = (
            ROOT / "skills" / "forge-method" / "templates" / "track-decision-artifact.md"
        ).read_text(encoding="utf-8")
        lifecycle_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "lifecycle-closure.md"
        ).read_text(encoding="utf-8")
        track_decision_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-track-decision.md"
        ).read_text(encoding="utf-8")
        readiness_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-readiness-check.md"
        ).read_text(encoding="utf-8")
        release_readiness_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-release-readiness.md"
        ).read_text(encoding="utf-8")
        self.assertIn("enterprise_required_artifacts", track_decision_template)
        self.assertIn("artifact_evidence_map", track_decision_template)
        self.assertIn("waiver_policy", track_decision_template)
        self.assertIn("artifact enterprise-track-map", lifecycle_pack)
        self.assertIn("artifact enterprise-track-map", track_decision_workflow)
        self.assertIn("artifact enterprise-readiness", readiness_workflow)
        self.assertIn("artifact enterprise-release-gate", release_readiness_workflow)
        readiness_template = (
            ROOT / "skills" / "forge-method" / "templates" / "readiness-matrix-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("track_required_artifacts", readiness_template)
        self.assertIn("enterprise_evidence_status", readiness_template)
        self.assertIn("release_gate_impact", readiness_template)
        release_template = (
            ROOT / "skills" / "forge-method" / "templates" / "release-readiness-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("enterprise_required_artifacts", release_template)
        self.assertIn("gate_decision", release_template)
        self.assertIn("publish_or_hold", release_template)
        self.assertIn("index", by_id["config-customization"].get("modes", []))
        self.assertIn("decide", by_id["track-decision"].get("modes", []))
        self.assertIn("enterprise-map", by_id["track-decision"].get("modes", []))
        self.assertIn("parallel", by_id["council-decision"].get("modes", []))
        self.assertIn("document", by_id["project-context"].get("modes", []))
        self.assertIn("prep", by_id["session-prep"].get("modes", []))
        self.assertIn("fresh-chat", by_id["context-recovery"].get("modes", []))
        self.assertIn("preview", by_id["checkpoint-preview"].get("modes", []))
        self.assertIn("review", by_id["code-review"].get("modes", []))
        self.assertIn("create", by_id["retrospective"].get("modes", []))
        self.assertIn("matrix", by_id["readiness-check"].get("modes", []))
        self.assertIn("enterprise", by_id["readiness-check"].get("modes", []))
        self.assertIn("closeout", by_id["research-closeout"].get("modes", []))
        self.assertIn("source-map", by_id["doc-index"].get("modes", []))
        self.assertIn("stale-check", by_id["doc-index"].get("modes", []))
        self.assertIn("archive-original", by_id["doc-shard"].get("modes", []))
        self.assertIn("stale-check", by_id["doc-shard"].get("modes", []))
        document_utility_template = (
            ROOT / "skills" / "forge-method" / "templates" / "document-utility-artifact.md"
        ).read_text(encoding="utf-8")
        document_utility_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "document-utility.md"
        ).read_text(encoding="utf-8")
        doc_index_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-doc-index.md"
        ).read_text(encoding="utf-8")
        doc_shard_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-doc-shard.md"
        ).read_text(encoding="utf-8")
        self.assertIn("source_fingerprint", document_utility_template)
        self.assertIn("source_last_modified", document_utility_template)
        self.assertIn("original_doc_decision", document_utility_template)
        self.assertIn("artifact doc-index", document_utility_pack)
        self.assertIn("artifact doc-shard", document_utility_pack)
        self.assertIn("artifact doc-index", doc_index_workflow)
        self.assertIn("artifact doc-check", doc_index_workflow)
        self.assertIn("artifact doc-shard", doc_shard_workflow)
        self.assertIn("artifact doc-check", doc_shard_workflow)
        spec_kernel_template = (
            ROOT / "skills" / "forge-method" / "templates" / "spec-kernel-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("capabilities", spec_kernel_template)
        self.assertIn("preservation_map", spec_kernel_template)
        self.assertIn("validation_verdict", spec_kernel_template)
        evidence_research = (
            ROOT / "skills" / "forge-method" / "facilitation" / "evidence-research.md"
        ).read_text(encoding="utf-8")
        market_scan_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-market-scan.md"
        ).read_text(encoding="utf-8")
        domain_scan_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-domain-scan.md"
        ).read_text(encoding="utf-8")
        technical_scan_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-technical-feasibility-scan.md"
        ).read_text(encoding="utf-8")
        self.assertIn("artifact research-scan", evidence_research)
        self.assertIn("research_question", evidence_research)
        self.assertIn("contradictions_or_falsifiers", evidence_research)
        self.assertIn("artifact research-scan", market_scan_workflow)
        self.assertIn("adoption_friction", market_scan_workflow)
        self.assertIn("artifact research-scan", domain_scan_workflow)
        self.assertIn("expert_review_needed", domain_scan_workflow)
        self.assertIn("artifact research-scan", technical_scan_workflow)
        self.assertIn("proof_path", technical_scan_workflow)
        self.assertIn("investigate", by_id["investigation"].get("modes", []))
        self.assertIn("tone", by_id["editorial-review"].get("modes", []))
        self.assertIn("failure", by_id["edge-case-review"].get("modes", []))
        self.assertIn("red-team", by_id["adversarial-review"].get("modes", []))
        self.assertIn("validate", by_id["test-strategy"].get("modes", []))
        self.assertIn("teach", by_id["teach-testing"].get("modes", []))
        self.assertIn("decide", by_id["test-engagement-model"].get("modes", []))
        self.assertIn("fixtures", by_id["test-framework"].get("modes", []))
        self.assertIn("validate", by_id["ci-quality-pipeline"].get("modes", []))
        self.assertIn("validate", by_id["atdd-plan"].get("modes", []))
        self.assertIn("validate", by_id["test-automation"].get("modes", []))
        self.assertIn("api", by_id["test-automation"].get("modes", []))
        self.assertIn("e2e", by_id["test-automation"].get("modes", []))
        self.assertIn("run-and-fix", by_id["test-automation"].get("modes", []))
        test_architecture_pack = (
            ROOT / "skills" / "forge-method" / "facilitation" / "test-architecture.md"
        ).read_text(encoding="utf-8")
        test_framework_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-test-framework.md"
        ).read_text(encoding="utf-8")
        test_automation_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-test-automation.md"
        ).read_text(encoding="utf-8")
        game_e2e_workflow = (
            ROOT / "skills" / "forge-method" / "references" / "workflow-game-e2e-scaffold.md"
        ).read_text(encoding="utf-8")
        self.assertIn("artifact test-framework", test_architecture_pack)
        self.assertIn("artifact test-automation", test_architecture_pack)
        self.assertIn("artifact test-framework", test_framework_workflow)
        self.assertIn("fixture architecture", test_framework_workflow)
        self.assertIn("artifact test-automation", test_automation_workflow)
        self.assertIn("no hardcoded waits", test_automation_workflow)
        self.assertIn("artifact game-e2e-scaffold", game_e2e_workflow)
        self.assertIn("launch-to-result", game_e2e_workflow)
        self.assertIn("review", by_id["test-review"].get("modes", []))
        self.assertIn("waiver", by_id["nfr-evidence-audit"].get("modes", []))
        self.assertIn("phase-2", by_id["traceability-gate"].get("modes", []))
        self.assertIn("evidence", by_id["risk-register"].get("modes", []))
        self.assertIn("threats", by_id["security-plan"].get("modes", []))
        self.assertIn("data-flow", by_id["privacy-data-plan"].get("modes", []))
        self.assertIn("rollback", by_id["devops-deployment-plan"].get("modes", []))
        self.assertIn("evidence", by_id["compliance-checklist"].get("modes", []))
        self.assertIn("signals", by_id["observability-plan"].get("modes", []))
        self.assertIn("gate", by_id["release-readiness"].get("modes", []))
        self.assertIn("document", by_id["game-context"].get("modes", []))
        self.assertIn("update", by_id["game-brief"].get("modes", []))
        self.assertIn("validate", by_id["game-brief"].get("modes", []))
        self.assertIn("create", by_id["gdd"].get("modes", []))
        self.assertIn("create", by_id["narrative-design"].get("modes", []))
        self.assertIn("balance", by_id["mechanics-design"].get("modes", []))
        self.assertIn("setup", by_id["engine-setup"].get("modes", []))
        self.assertIn("create", by_id["engine-architecture"].get("modes", []))
        self.assertIn("prove", by_id["quick-prototype"].get("modes", []))
        self.assertIn("run", by_id["playtest-plan"].get("modes", []))
        self.assertIn("measure", by_id["performance-plan"].get("modes", []))
        self.assertIn("rebalance", by_id["game-sprint-planning"].get("modes", []))
        game_brief_template = (
            ROOT / "skills" / "forge-method" / "templates" / "game-brief-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("player_fantasy", game_brief_template)
        self.assertIn("mvp_playable_proof", game_brief_template)
        game_sprint_template = (
            ROOT / "skills" / "forge-method" / "templates" / "game-sprint-plan-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("playable_slice_goal", game_sprint_template)
        self.assertIn("decision_sources", game_sprint_template)
        self.assertIn("headless", by_id["game-story-creation"].get("modes", []))
        self.assertIn("risk", by_id["game-sprint-status"].get("modes", []))
        self.assertIn("review", by_id["game-qa-review"].get("modes", []))
        self.assertEqual(by_id["game-e2e-scaffold"].get("template"), "game-e2e-artifact")
        self.assertIn("manual-proof", by_id["game-e2e-scaffold"].get("modes", []))
        self.assertIn("semi-automated", by_id["game-e2e-scaffold"].get("modes", []))
        test_framework_template = (
            ROOT / "skills" / "forge-method" / "templates" / "test-framework-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("detected_framework", test_framework_template)
        self.assertIn("pure_helpers", test_framework_template)
        self.assertIn("framework_wrappers", test_framework_template)
        self.assertIn("failure_repair_policy", test_framework_template)
        test_automation_template = (
            ROOT / "skills" / "forge-method" / "templates" / "test-automation-artifact.md"
        ).read_text(encoding="utf-8")
        self.assertIn("semantic_locator_policy", test_automation_template)
        self.assertIn("visible_outcome_assertions", test_automation_template)
        self.assertIn("run_and_fix_result", test_automation_template)
        self.assertIn("no_hardcoded_waits", test_automation_template)
        game_e2e_template = (ROOT / "skills" / "forge-method" / "templates" / "game-e2e-artifact.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("setup_action_assertion_teardown", game_e2e_template)
        self.assertIn("observable_success_signal", game_e2e_template)
        self.assertIn("release_gate_link", game_e2e_template)
        build_story_template = (ROOT / "skills" / "forge-method" / "templates" / "build-story-work-order.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("## Domain Context", build_story_template)
        self.assertIn("domain_checks", build_story_template)
        for pack_id in pack_ids:
            pack_text = (ROOT / "skills" / "forge-method" / "facilitation" / f"{pack_id}.md").read_text(
                encoding="utf-8"
            )
            for section in required_facilitation_sections:
                self.assertIn(section, pack_text, pack_id)
            self.assertGreaterEqual(pack_text.count("\n  - "), 12, pack_id)
        self.assertEqual(guide["recommended_workflow"], "runtime-builder")
        self.assertEqual(guide["workflow_metadata"]["id"], "runtime-builder")
        self.assertEqual(guide["facilitation_pack"], "skill:facilitation/runtime-builder.md")
        agents = run_cmd("agent", "list").stdout
        agent_validation = run_cmd("agent", "validate").stdout

        self.assertIn("facilitator", agents)
        self.assertIn("quality-reviewer", agents)
        self.assertIn("Agent profile validation passed.", agent_validation)
        self.assertEqual(version.strip(), CURRENT_VERSION)

    def test_skill_requires_launcher_on_every_invocation(self) -> None:
        skill_text = (ROOT / "skills" / "forge-method" / "SKILL.md").read_text(encoding="utf-8")

        self.assertIn("Every invocation of this skill must execute the launcher before answering.", skill_text)
        self.assertIn("Do not answer from prior chat state", skill_text)
        self.assertIn("the current filesystem and launcher output are authoritative", skill_text)
        self.assertIn("Bootstrap budget is strict", skill_text)
        self.assertIn("do not inspect project docs, source files, git history, or broad workspace context", skill_text)
        self.assertIn('For missing-state routes, do not paraphrase into "Forge Method is active"', skill_text)

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

            self.assertEqual(plan["runtime_version"], CURRENT_VERSION)
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
            add_decision_source(root)
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

    def test_persona_lens_guidance_council_and_compact_runtime_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Persona Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")

            pm_guide = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "quero uma lente de PM para validar o PRD e cortar escopo",
                    "--json",
                ).stdout
            )
            qa_guide = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "usa um QA lens para traceability gate antes do release",
                    "--json",
                ).stdout
            )
            presentation_guide = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "use presentation master to structure a pitch deck narrative with proof and call to action",
                    "--json",
                ).stdout
            )
            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            council = run_cmd(
                "council",
                "run",
                "--root",
                str(root),
                "--topic",
                "usar UX designer lens para calibrar jornada",
                "--next-action",
                "continue persona lens proof",
            ).stdout
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertEqual(pm_guide["persona_lens"]["id"], "product-manager")
            self.assertEqual(pm_guide["recommended_workflow"], "product-requirements")
            self.assertEqual(pm_guide["intent_classification"], "product-flow")
            self.assertIn("assumption-spotlight", pm_guide["persona_lens"]["techniques"])
            self.assertNotIn("persona", pm_guide["recommended_agents"][0])
            self.assertLess(len(json.dumps(pm_guide["persona_lens"], sort_keys=True)), 900)

            self.assertEqual(qa_guide["persona_lens"]["id"], "qa-strategist")
            self.assertEqual(qa_guide["recommended_workflow"], "traceability-gate")
            self.assertIn("quality-crosscheck", qa_guide["persona_lens"]["techniques"])

            self.assertEqual(presentation_guide["persona_lens"]["id"], "presentation-craft")
            self.assertEqual(presentation_guide["recommended_workflow"], "storytelling")
            self.assertEqual(presentation_guide["facilitation_pack"], "skill:facilitation/storytelling.md")
            self.assertIn("presentation", presentation_guide["human_prompt"].lower())

            lens_ids = {item["id"] for item in index_payload["persona_lenses"]}
            technique_ids = {item["id"] for item in index_payload["elicitation_techniques"]}
            self.assertTrue({"product-manager", "architect", "ux-designer", "qa-strategist", "game-designer", "builder", "tech-writer", "presentation-craft"} <= lens_ids)
            self.assertIn("risk-inversion", technique_ids)
            self.assertNotIn("persona", index_payload["agents"][0])

            self.assertIn("Persona lens: UX Designer Lens", council)
            self.assertIn("Round 1: Specialist Takes", council)
            self.assertIn("Agent Orchestration", council)
            self.assertIn("[Facilitator]", council)
            self.assertIn("[Spec Architect]", council)
            self.assertIn("[Quality Reviewer]", council)
            self.assertTrue((root / snapshot["state"]["last_council_artifact"]).exists())
            artifact_text = (root / snapshot["state"]["last_council_artifact"]).read_text(encoding="utf-8")
            self.assertIn("Orchestration:", artifact_text)
            self.assertIn("Dissent to preserve", artifact_text)

    def test_lifecycle_closure_guidance_and_compact_contracts(self) -> None:
        runtime = load_runtime_module()
        lifecycle_cases = [
            (
                "document this project and generate project context for future agents",
                "project-context",
                "project-context-artifact",
                "1-discovery",
            ),
            (
                "which track should this project use and what workflows are required",
                "track-decision",
                "track-decision-artifact",
                "1-discovery",
            ),
            (
                "create a readiness matrix linking PRD UX architecture risk stories validation and findings",
                "readiness-check",
                "readiness-matrix-artifact",
                "3-plan",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "discovery")

            for question, workflow, template, phase in lifecycle_cases:
                with self.subTest(workflow=workflow):
                    guide = runtime.build_guide_payload(root, question=question, max_chars=12000)

                    self.assertEqual(guide["intent_classification"], "lifecycle-flow")
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["recommended_phase"], phase)
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertEqual(
                        guide["facilitation_pack"],
                        "skill:facilitation/lifecycle-closure.md"
                        if workflow != "readiness-check"
                        else "skill:facilitation/story-lifecycle.md",
                    )
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])

            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            workflow_ids = {item["id"] for item in index_payload["workflows"]}
            self.assertTrue(
                {
                    "track-decision",
                    "project-context",
                    "session-prep",
                    "code-review",
                    "retrospective",
                    "research-closeout",
                    "readiness-check",
                }
                <= workflow_ids
            )

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "build_story_ready")
            review = runtime.build_guide_payload(
                root,
                question="review this code diff and create actionable findings before readiness",
                max_chars=12000,
            )

            self.assertEqual(review["intent_classification"], "lifecycle-flow")
            self.assertEqual(review["recommended_workflow"], "code-review")
            self.assertEqual(review["recommended_phase"], "4-build-verify")
            self.assertEqual(review["workflow_metadata"].get("template"), "code-review-artifact")
            self.assertEqual(review["facilitation_pack"], "skill:facilitation/lifecycle-closure.md")
            self.assertTrue(review["state_update_required"])

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "evolve_runtime")
            session = runtime.build_guide_payload(
                root,
                question="prep next session with read order blockers first command and next workflow",
                max_chars=12000,
            )
            p14 = runtime.build_guide_payload(
                root,
                question="continue P1.4 Product Context Review Retrospective Closure from the systematic parity plan",
                max_chars=12000,
            )

            self.assertEqual(session["intent_classification"], "lifecycle-flow")
            self.assertEqual(session["recommended_workflow"], "session-prep")
            self.assertEqual(session["workflow_metadata"].get("template"), "session-prep-artifact")
            self.assertTrue(session["state_update_required"])
            self.assertEqual(p14["intent_classification"], "builder-flow")
            self.assertEqual(p14["recommended_workflow"], "runtime-builder")
            self.assertEqual(p14["facilitation_pack"], "skill:facilitation/runtime-builder.md")
            self.assertFalse(p14["state_update_required"])

        for ref_name in [
            "workflow-track-decision.md",
            "workflow-project-context.md",
            "workflow-session-prep.md",
            "workflow-code-review.md",
            "workflow-retrospective.md",
            "workflow-research-closeout.md",
        ]:
            ref_text = (ROOT / "skills" / "forge-method" / "references" / ref_name).read_text(encoding="utf-8")
            self.assertIn("trigger:", ref_text, ref_name)
            self.assertIn("handoff:", ref_text, ref_name)
            self.assertLess(len(ref_text), 1400, ref_name)

    def test_game_studio_depth_guidance_and_compact_contracts(self) -> None:
        runtime = load_runtime_module()
        game_cases = [
            (
                "create and validate a living game brief with player fantasy core loop verbs pillars references mvp playable proof parked scope decision log assumptions open questions and next workflow",
                "game-brief",
                "game-brief-artifact",
                "1-discovery",
            ),
            (
                "generate game project context with player fantasy loop engine profile playable slice and next workflow",
                "game-context",
                "game-context-artifact",
                "1-discovery",
            ),
            (
                "setup Godot engine profile with folder layout first run command validation and performance budget for this game",
                "engine-setup",
                "engine-setup-artifact",
                "2-specification",
            ),
            (
                "create the GDD game design document with pillars loop systems content progression playable slice and proof",
                "gdd",
                "gdd",
                "2-specification",
            ),
            (
                "quick prototype for the first playable game action with asset stubs proof check and decision",
                "quick-prototype",
                "quick-prototype-artifact",
                "4-build-verify",
            ),
            (
                "plan a playtest for this game playable slice with target players tasks signals and decision map",
                "playtest-plan",
                "playtest-plan-artifact",
                "4-build-verify",
            ),
            (
                "performance budget for this Unity game with fps frame time memory checks and optimization story",
                "performance-plan",
                "performance-plan-artifact",
                "3-plan",
            ),
            (
                "create game stories from the accepted playable slice with player value engine notes asset assumptions acceptance checks evidence and sprint update",
                "game-story-creation",
                "game-story-artifact",
                "3-plan",
            ),
            (
                "game sprint planning for the next playable slice: order story batch by player value decision sources dependencies production risk validation plan deferred scope and next story",
                "game-sprint-planning",
                "game-sprint-plan-artifact",
                "3-plan",
            ),
            (
                "game sprint status for the playable slice: done active review blocked deferred evidence gaps scope pressure risks and next story",
                "game-sprint-status",
                "game-sprint-status-artifact",
                "4-build-verify",
            ),
            (
                "game test framework for this Unity project with mechanics content UI saves fixtures commands manual playtest boundaries and first automation target",
                "game-test-framework",
                "game-lifecycle-artifact",
                "3-plan",
            ),
            (
                "game e2e smoke scaffold from launch to playable result with setup action assertion teardown evidence mode and readiness gate",
                "game-e2e-scaffold",
                "game-e2e-artifact",
                "4-build-verify",
            ),
            (
                "game QA review this playable slice for playability feedback stability performance evidence and repair route",
                "game-qa-review",
                "game-qa-review-artifact",
                "4-build-verify",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "discovery")

            for question, workflow, template, phase in game_cases:
                with self.subTest(workflow=workflow):
                    guide = runtime.build_guide_payload(root, question=question, max_chars=12000)

                    self.assertEqual(guide["intent_classification"], "game-flow")
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["recommended_phase"], phase)
                    expected_pack = (
                        "skill:facilitation/game-brief.md"
                        if workflow == "game-brief"
                        else "skill:facilitation/game-lifecycle.md"
                    )
                    self.assertEqual(guide["facilitation_pack"], expected_pack)
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])
                    if workflow == "game-story-creation":
                        self.assertIn("slice aceito", guide["human_experience"]["decision_summary"])
                    if workflow == "game-sprint-status":
                        self.assertIn("status de producao de jogo", guide["human_experience"]["decision_summary"])

            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            workflow_ids = {item["id"] for item in index_payload["workflows"]}
            self.assertTrue(
                {
                    "game-context",
                    "engine-setup",
                    "gdd",
                    "narrative-design",
                    "mechanics-design",
                    "quick-prototype",
                    "game-story-creation",
                    "game-sprint-status",
                    "playtest-plan",
                    "performance-plan",
                    "game-qa-review",
                    "game-test-framework",
                    "game-e2e-scaffold",
                }
                <= workflow_ids
            )

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "evolve_runtime")
            p15 = runtime.build_guide_payload(
                root,
                question="continue P1.5 Game Studio Depth from the systematic parity plan",
                max_chars=12000,
            )

            self.assertEqual(p15["intent_classification"], "builder-flow")
            self.assertEqual(p15["recommended_workflow"], "runtime-builder")
            self.assertEqual(p15["facilitation_pack"], "skill:facilitation/runtime-builder.md")
            self.assertFalse(p15["state_update_required"])

        for ref_name in [
            "workflow-game-brief.md",
            "workflow-game-context.md",
            "workflow-engine-setup.md",
            "workflow-gdd.md",
            "workflow-engine-architecture.md",
            "workflow-quick-prototype.md",
            "workflow-playtest-plan.md",
            "workflow-performance-plan.md",
            "workflow-game-story-creation.md",
            "workflow-game-sprint-planning.md",
            "workflow-game-sprint-status.md",
            "workflow-game-qa-review.md",
            "workflow-game-test-framework.md",
            "workflow-game-e2e-scaffold.md",
        ]:
            ref_text = (ROOT / "skills" / "forge-method" / "references" / ref_name).read_text(encoding="utf-8")
            self.assertIn("trigger:", ref_text, ref_name)
            self.assertIn("handoff:", ref_text, ref_name)
            self.assertLess(len(ref_text), 1700, ref_name)

    def test_game_dev_story_routes_to_mechanical_build_when_ready(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "build_story_ready")
            guide = runtime.build_guide_payload(
                root,
                question="dev story for this game: implement the next playable slice with engine notes and player checks",
                max_chars=12000,
            )

            self.assertEqual(guide["intent_classification"], "mechanical-build")
            self.assertEqual(guide["recommended_workflow"], "build-story")
            self.assertEqual(guide["recommended_phase"], "4-build-verify")
            self.assertEqual(guide["workflow_metadata"].get("template"), "build-story-work-order")
            self.assertFalse(guide["state_update_required"])
            self.assertIn("playable-slice acceptance", guide["recommended_action"])

    def test_tea_depth_guidance_and_compact_contracts(self) -> None:
        runtime = load_runtime_module()
        tea_cases = [
            (
                "quality is weak and I do not know if we need advice design implementation review audit or a release gate",
                "test-engagement-model",
                "test-engagement-artifact",
                "2-specification",
            ),
            (
                "create a test strategy with risk assessment proof mix gates commands ownership and waivers",
                "test-strategy",
                "test-strategy-artifact",
                "3-plan",
            ),
            (
                "setup test framework with fixture architecture pure helpers wrappers composition cleanup and command contract",
                "test-framework",
                "test-framework-artifact",
                "3-plan",
            ),
            (
                "configure CI quality pipeline with local fast full release checks burn in selective testing artifacts and failure policy",
                "ci-quality-pipeline",
                "ci-quality-pipeline-artifact",
                "3-plan",
            ),
            (
                "create ATDD acceptance test examples with given when then edge cases and risk coverage before build",
                "atdd-plan",
                "atdd-plan-artifact",
                "3-plan",
            ),
            (
                "automate high risk QA checks with fixtures data setup assertions commands evidence links and manual remainders",
                "test-automation",
                "test-automation-artifact",
                "4-build-verify",
            ),
            (
                "review the tests against acceptance risk coverage weak assertions flaky patterns and gate recommendation",
                "test-review",
                "test-review-artifact",
                "4-build-verify",
            ),
            (
                "run NFR evidence audit for security performance reliability accessibility compliance gaps waivers and release impact",
                "nfr-evidence-audit",
                "nfr-evidence-artifact",
                "5-ready-operate",
            ),
            (
                "traceability matrix and gate decision for requirements risks checks evidence missing evidence waivers and release impact",
                "traceability-gate",
                "traceability-gate-artifact",
                "4-build-verify",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "discovery")

            for question, workflow, template, phase in tea_cases:
                with self.subTest(workflow=workflow):
                    guide = runtime.build_guide_payload(root, question=question, max_chars=12000)

                    self.assertEqual(guide["intent_classification"], "quality-flow")
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["recommended_phase"], phase)
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])

            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            workflow_ids = {item["id"] for item in index_payload["workflows"]}
            self.assertTrue(
                {
                    "test-strategy",
                    "teach-testing",
                    "test-engagement-model",
                    "test-framework",
                    "ci-quality-pipeline",
                    "atdd-plan",
                    "test-automation",
                    "test-review",
                    "nfr-evidence-audit",
                    "traceability-gate",
                }
                <= workflow_ids
            )

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "evolve_runtime")
            p16 = runtime.build_guide_payload(
                root,
                question="continue P1.6 Test Architecture Enterprise Depth from the systematic parity plan",
                max_chars=12000,
            )

            self.assertEqual(p16["intent_classification"], "builder-flow")
            self.assertEqual(p16["recommended_workflow"], "runtime-builder")
            self.assertEqual(p16["facilitation_pack"], "skill:facilitation/runtime-builder.md")
            self.assertFalse(p16["state_update_required"])

        for ref_name in [
            "workflow-test-strategy.md",
            "workflow-teach-testing.md",
            "workflow-test-engagement-model.md",
            "workflow-test-framework.md",
            "workflow-ci-quality-pipeline.md",
            "workflow-atdd-plan.md",
            "workflow-test-automation.md",
            "workflow-test-review.md",
            "workflow-nfr-evidence-audit.md",
            "workflow-traceability-gate.md",
        ]:
            ref_text = (ROOT / "skills" / "forge-method" / "references" / ref_name).read_text(encoding="utf-8")
            self.assertIn("trigger:", ref_text, ref_name)
            self.assertIn("handoff:", ref_text, ref_name)
            self.assertLess(len(ref_text), 1900, ref_name)

    def test_platform_ops_and_visual_alignment_guidance_routes(self) -> None:
        runtime = load_runtime_module()
        cases = [
            (
                "preciso planejar infra, banco de dados, secrets, deploy, rollback e observability antes do build",
                "platform-flow",
                "platform-ops-plan",
                "platform-ops-plan-artifact",
                "2-specification",
                "skill:facilitation/platform-ops.md",
            ),
            (
                "configurar CI/CD com GitHub Actions, comandos fast/full/release e politica de falha",
                "platform-flow",
                "ci-quality-pipeline",
                "ci-quality-pipeline-artifact",
                "3-plan",
                "skill:facilitation/test-architecture.md",
            ),
            (
                "quero um mockup visual da primeira tela para alinhar expectativa antes de criar stories",
                "visual-flow",
                "visual-alignment-prototype",
                "visual-alignment-prototype-artifact",
                "1-discovery",
                "skill:facilitation/visual-alignment.md",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            runtime.prepare_parity_replay_state(root, "discovery")

            for question, classification, workflow, template, phase, pack in cases:
                with self.subTest(workflow=workflow):
                    guide = runtime.build_guide_payload(root, question=question, max_chars=12000)

                    self.assertEqual(guide["intent_classification"], classification)
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["recommended_phase"], phase)
                    self.assertEqual(guide["facilitation_pack"], pack)
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])

            broad_game = runtime.build_guide_payload(
                root,
                question=(
                    "quero criar um VTT para jogar RPG online com meus amigos, com IA, mapas, animacoes, temas, "
                    "musica e boa usabilidade"
                ),
                max_chars=12000,
            )
            self.assertEqual(broad_game["intent_classification"], "game-flow")
            self.assertEqual(broad_game["recommended_workflow"], "game-brief")
            self.assertTrue(broad_game["early_visual_proof"]["required"])
            self.assertEqual(broad_game["early_visual_proof"]["workflow"], "visual-alignment-prototype")
            self.assertIn("early visual proof loop", broad_game["recommended_action"])
            self.assertIn("visual-alignment-prototype", [item["workflow"] for item in broad_game["alternatives"]])

            broad_product = runtime.build_guide_payload(
                root,
                question="quero criar um app web bonito para organizar projetos e mostrar dashboards para clientes",
                max_chars=12000,
            )
            self.assertEqual(broad_product["intent_classification"], "product-flow")
            self.assertTrue(broad_product["early_visual_proof"]["required"])
            self.assertIn("human must accept", broad_product["early_visual_proof"]["human_gate"])

            collaboration_cases = [
                (
                    "estamos desenvolvendo com amigos usando Forge e queremos um GitHub org com trunk based e PRs curtos",
                    "team-operating-model",
                    "team-operating-model-artifact",
                ),
                (
                    "precisamos separar o produto por Product Areas com owners contratos paths dependencias e criterios de split",
                    "product-area-map",
                    "product-area-map-artifact",
                ),
                (
                    "vamos usar CODEOWNERS rulesets required checks e trunk-based antes de abrir stories paralelas",
                    "trunk-based-plan",
                    "trunk-based-plan-artifact",
                ),
                (
                    "quero splitar esse Product Area em repo proprio e transformar em standalone Method Project com .forge-method",
                    "repo-split-plan",
                    "repo-split-plan-artifact",
                ),
                (
                    "handoff de colaboracao: branch PR owner Product Area checks evidence blockers e primeiro comando",
                    "collaboration-handoff",
                    "collaboration-handoff-artifact",
                ),
            ]
            for question, workflow, template in collaboration_cases:
                with self.subTest(workflow=workflow):
                    guide = runtime.build_guide_payload(root, question=question, max_chars=12000)
                    self.assertEqual(guide["intent_classification"], "collaboration-flow")
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["facilitation_pack"], "skill:facilitation/collaboration.md")
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])
                    self.assertIn("Product Area", guide["human_experience"]["guardrail"])

            github_collab = runtime.build_guide_payload(
                root,
                question="GitHub org com CODEOWNERS e CI/CD: definir checks obrigatorios e regras antes do time trabalhar em paralelo",
                max_chars=12000,
            )
            self.assertEqual(github_collab["intent_classification"], "collaboration-flow")
            self.assertEqual(github_collab["recommended_workflow"], "trunk-based-plan")
            self.assertIn("ci-quality-pipeline", [item["workflow"] for item in github_collab["alternatives"]])

            tracks = json.loads(
                run_cmd(
                    "track",
                    "recommend",
                    "--objective",
                    "web app com banco de dados, CI/CD, deploy, secrets, observability e rollback",
                    "--json",
                ).stdout
            )
            self.assertEqual(tracks["recommended"][0]["id"], "platform-ops")

            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            workflow_ids = {item["id"] for item in index_payload["workflows"]}
            self.assertTrue(
                {
                    "platform-ops-plan",
                    "visual-alignment-prototype",
                    "team-operating-model",
                    "product-area-map",
                    "trunk-based-plan",
                    "collaboration-handoff",
                    "repo-split-plan",
                }
                <= workflow_ids
            )

        for ref_name in [
            "workflow-platform-ops-plan.md",
            "workflow-visual-alignment-prototype.md",
            "workflow-team-operating-model.md",
            "workflow-product-area-map.md",
            "workflow-trunk-based-plan.md",
            "workflow-collaboration-handoff.md",
            "workflow-repo-split-plan.md",
        ]:
            ref_text = (ROOT / "skills" / "forge-method" / "references" / ref_name).read_text(encoding="utf-8")
            self.assertIn("trigger:", ref_text, ref_name)
            self.assertIn("handoff:", ref_text, ref_name)
            self.assertLess(len(ref_text), 1900, ref_name)

    def test_tracks_guide_council_builder_and_config_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))

            tracks = json.loads(
                run_cmd("track", "recommend", "--objective", "build a secure enterprise app", "--json").stdout
            )
            guide = json.loads(run_cmd("guide", "--root", str(root), "--json").stdout)
            set_track = run_cmd("track", "set", "--root", str(root), "--track", "game-studio", "--set-module").stdout
            council = run_cmd(
                "council",
                "run",
                "--root",
                str(root),
                "--topic",
                "Should this decision use a council?",
                "--eval",
            ).stdout
            council_json = json.loads(
                run_cmd(
                    "council",
                    "run",
                    "--root",
                    str(root),
                    "--topic",
                    "split architecture quality and implementation checks into parallel subagents",
                    "--mode",
                    "parallel",
                    "--json",
                ).stdout
            )
            workflow_path = run_cmd(
                "builder",
                "scaffold",
                "--root",
                str(root),
                "--kind",
                "workflow",
                "--id",
                "custom-check",
                "--title",
                "Custom Check",
            ).stdout.strip()
            builder_validation = run_cmd("builder", "validate", "--root", str(root)).stdout
            config = json.loads(run_cmd("config", "inspect", "--root", str(root), "--json").stdout)
            config_validation = run_cmd("config", "validate", "--root", str(root)).stdout
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            gate = run_cmd("gate", "--root", str(root), "--require-evals").stdout

            self.assertEqual(tracks["recommended"][0]["id"], "enterprise")
            self.assertTrue(guide["state_found"])
            self.assertEqual(guide["track"]["id"], "standard-product")
            self.assertIn("Track set: game-studio", set_track)
            self.assertIn("Forge Agent Council", council)
            self.assertIn("Decision Frame", council)
            self.assertIn("Round 2: Convergence", council)
            self.assertIn("Agent Orchestration", council)
            self.assertIn("Persisted decision artifact:", council)
            self.assertEqual(council_json["workflow"], "council-decision")
            self.assertEqual(council_json["mode"], "parallel")
            self.assertEqual(council_json["orchestration_plan"]["execution"], "parallel")
            self.assertTrue(council_json["orchestration_plan"]["workers"])
            self.assertIn("do_not_persist", council_json["orchestration_plan"]["merge"])
            self.assertEqual(snapshot["state"]["track"], "game-studio")
            self.assertEqual(snapshot["state"]["module"], "game-studio")
            self.assertTrue((root / snapshot["state"]["last_council_artifact"]).exists())
            self.assertTrue((root / workflow_path).exists())
            self.assertIn("Builder validation passed.", builder_validation)
            self.assertEqual(config["sources"], [])
            self.assertIn("Config validation passed.", config_validation)
            self.assertIn("Gate passed.", gate)

    def test_gate_and_snapshot_use_builder_extension_validation_surface(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Builder Gate Surface Project", "--root", str(root))
            skill_dir = root / ".forge-method" / "skills" / "broken"
            skill_dir.mkdir(parents=True, exist_ok=True)
            (skill_dir / "SKILL.md").write_text("# Broken Skill\n\nMissing frontmatter.\n", encoding="utf-8")

            builder_validation = run_cmd("builder", "validate", "--root", str(root), check=False)
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            gate = run_cmd("gate", "--root", str(root), check=False)

            self.assertNotEqual(builder_validation.returncode, 0)
            self.assertIn(".forge-method/skills/broken/SKILL.md: missing skill frontmatter", builder_validation.stdout)
            self.assertIn(
                ".forge-method/skills/broken/SKILL.md: missing skill frontmatter",
                snapshot["quality"]["builder"]["errors"],
            )
            self.assertNotEqual(gate.returncode, 0)
            self.assertIn(
                "builder: .forge-method/skills/broken/SKILL.md: missing skill frontmatter",
                gate.stdout,
            )

    def test_snapshot_uses_workflow_validation_surface(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Workflow Snapshot Surface Project", "--root", str(root))
            workflow_dir = root / ".forge-method" / "workflows"
            workflow_dir.mkdir(parents=True, exist_ok=True)
            (workflow_dir / "workflow-broken.md").write_text(
                "\n".join(
                    [
                        "# workflow: broken",
                        "",
                        "trigger:",
                        "  - broken request",
                        "",
                        "steps:",
                        "  1. only partial",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            workflow_validation = run_cmd("workflow", "validate", "--root", str(root), check=False)
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            start = run_cmd("start", "--root", str(root)).stdout
            status = run_cmd("status", "--root", str(root), "--brief").stdout
            preflight = run_cmd("preflight", "--root", str(root)).stdout
            preflight_json = json.loads(run_cmd("preflight", "--root", str(root), "--json").stdout)
            reload_text = run_cmd("reload", "--root", str(root)).stdout
            reload_json = json.loads(run_cmd("reload", "--root", str(root), "--json").stdout)
            resume_text = run_cmd("resume", "--root", str(root)).stdout
            resume_json = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            context_plan_json = json.loads(run_cmd("context", "plan", "--root", str(root), "--json").stdout)
            context_health_text = run_cmd("context", "health", "--root", str(root)).stdout
            context_health_json = json.loads(
                run_cmd("context", "health", "--root", str(root), "--json").stdout
            )
            next_text = run_cmd("next", "--root", str(root)).stdout
            next_json = json.loads(run_cmd("next", "--root", str(root), "--json").stdout)
            gate = run_cmd("gate", "--root", str(root), check=False)

            self.assertNotEqual(workflow_validation.returncode, 0)
            self.assertIn("workflow-broken.md: missing section `inputs:`", workflow_validation.stdout)
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                snapshot["quality"]["workflows"]["errors"],
            )
            self.assertIn("Quality: failed", start)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", start)
            self.assertIn("Quality: failed", status)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", status)
            self.assertIn("Quality: failed", preflight)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", preflight)
            self.assertFalse(preflight_json["status"]["quality"]["passed"])
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                preflight_json["status"]["quality"]["surfaces"]["workflows"]["errors"],
            )
            self.assertIn("Quality: failed", reload_text)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", reload_text)
            self.assertFalse(reload_json["quality"]["passed"])
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                reload_json["quality"]["surfaces"]["workflows"]["errors"],
            )
            self.assertIn("Quality: failed", resume_text)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", resume_text)
            self.assertFalse(resume_json["quality"]["passed"])
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                resume_json["quality"]["surfaces"]["workflows"]["errors"],
            )
            self.assertFalse(context_plan_json["quality"]["passed"])
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                context_plan_json["quality"]["surfaces"]["workflows"]["errors"],
            )
            self.assertIn("Context health: blocked", context_health_text)
            self.assertIn("Quality: failed", context_health_text)
            self.assertIn("workflows: workflow-broken.md: missing section `inputs:`", context_health_text)
            self.assertEqual(context_health_json["level"], "blocked")
            self.assertEqual(
                context_health_json["recommended_action"],
                "repair project quality before trusting context continuation",
            )
            self.assertFalse(context_health_json["quality"]["passed"])
            self.assertIn("audit", [command["name"] for command in context_health_json["commands"]])
            self.assertIn("Quality: failed", next_text)
            self.assertFalse(next_json["quality"]["passed"])
            self.assertIn("Continue the active workflow", next_json["reason"])
            self.assertEqual(next_json["context_boundary"]["mode"], "resume-first")
            self.assertIn(
                "workflow-broken.md: missing section `inputs:`",
                next_json["quality"]["surfaces"]["workflows"]["errors"],
            )
            self.assertNotEqual(gate.returncode, 0)
            self.assertIn("workflow: workflow-broken.md: missing section `inputs:`", gate.stdout)

    def test_mechanical_work_order_goal_and_commit_policy_contracts(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan"]:
                run_cmd("transition", "--root", str(root), "--phase", phase)
                phase_snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
                self.assertTrue(phase_snapshot["resume"]["grill_gate_required"])
            run_cmd("transition", "--root", str(root), "--phase", "4-build-verify")
            add_decision_source(root)
            run_cmd("story", "add", "--root", str(root), "--id", "story-a", "--title", "Build A", "--acceptance", "A works")
            run_cmd("story", "add", "--root", str(root), "--id", "story-b", "--title", "Build B", "--acceptance", "B works")
            config_dir = root / ".forge-method" / "config"
            config_dir.mkdir(parents=True, exist_ok=True)
            (config_dir / "local.yaml").write_text('commit_policy: "epic"\n', encoding="utf-8")

            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            guide = runtime.build_guide_payload(root, question="", max_chars=12000)
            guide_text = run_cmd("guide", "--root", str(root)).stdout
            next_text = run_cmd("next", "--root", str(root)).stdout
            next_json = json.loads(run_cmd("next", "--root", str(root), "--json").stdout)
            config_validation = run_cmd("config", "validate", "--root", str(root)).stdout

            work_order = resume["mechanical_work_order"]
            self.assertFalse(resume["grill_gate_required"])
            self.assertEqual(resume["action"], "start_next_story")
            self.assertTrue(work_order["autonomous"])
            self.assertTrue(work_order["goal_recommended"])
            self.assertEqual(work_order["commit_policy"], "epic")
            command_names = [item["name"] for item in work_order["commands"]]
            self.assertIn("story-start", command_names)
            self.assertIn("story-review", command_names)
            self.assertIn("evidence-add", command_names)
            self.assertIn("story-done", command_names)
            self.assertIn("run required checks", work_order["loop"])
            self.assertIn("write story evidence", work_order["loop"])
            self.assertIn("do not ask for procedural ok/continue between mechanical steps", work_order["do_not_prompt"])
            self.assertIn("evidence is written for story story-a", work_order["done_when"])
            self.assertIn("sprint/status is updated and the next ready story or ready gate is explicit", work_order["done_when"])
            self.assertIn("required check fails", work_order["self_repair_when"])
            self.assertIn("missing external credential or access", work_order["stop_only_when"])
            self.assertTrue(resume["codex_goal_handoff"]["recommended"])
            self.assertIn("/goal", resume["codex_goal_handoff"]["command"])
            self.assertIn("Do not ask for procedural ok/continue", resume["codex_goal_handoff"]["goal_text"])
            self.assertEqual(guide["mechanical_work_order"]["next_mechanical_step"], work_order["next_mechanical_step"])
            self.assertEqual(guide["workflow_metadata"].get("template"), "build-story-work-order")
            self.assertEqual(guide["facilitation_pack"], "skill:facilitation/story-lifecycle.md")
            self.assertIn("Status: Build is ready:", guide_text)
            self.assertNotIn("First question:", guide_text)
            self.assertNotIn("Prompt: Build is ready:", guide_text)
            self.assertIn("Goal recommended", next_text)
            self.assertNotIn("ok?", next_text.lower())
            self.assertNotIn("continue?", next_text.lower())
            self.assertNotIn("quer continuar", next_text.lower())
            self.assertEqual(next_json["action"], "start_next_story")
            self.assertEqual(next_json["required_next_workflow"], "build-story")
            self.assertTrue(next_json["codex_goal_handoff"]["recommended"])
            self.assertTrue(next_json["mechanical_work_order"]["autonomous"])
            self.assertIn("story-start", [command["name"] for command in next_json["commands"]])
            self.assertIn("Config validation passed.", config_validation)

    def test_project_config_override_model_and_capability_index_contracts(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Config Project", "--root", str(root))
            config_dir = root / ".forge-method" / "config"
            config_dir.mkdir(parents=True, exist_ok=True)
            (config_dir / "team.yaml").write_text(
                "\n".join(
                    [
                        'human_tone: "calm"',
                        'workflow.product-requirements.template: "quick-dev-artifact"',
                        'workflow.product-requirements.outputs: "requirements | override proof"',
                        'agent.facilitator.title: "Project Facilitator"',
                        'convention.release-notes: "short and evidence-first"',
                        'capability.config-review.summary: "Review effective Forge config."',
                        'capability.config-review.workflow: "config-customization"',
                        'capability.config-review.kind: "workflow"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (config_dir / "local.yaml").write_text(
                "\n".join(
                    [
                        'human_tone: "direct"',
                        'workflow.product-requirements.facilitation_pack: "config-customization"',
                        'workflow.product-requirements.modes: "create | validate | index"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            validation = run_cmd("config", "validate", "--root", str(root)).stdout
            inspect_payload = json.loads(run_cmd("config", "inspect", "--root", str(root), "--json").stdout)
            guide = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "preciso criar e validar um PRD com requisitos de produto",
                    "--json",
                ).stdout
            )
            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)
            written_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--write", "--json").stdout)

            self.assertIn("Config validation passed.", validation)
            self.assertEqual(inspect_payload["effective"]["human_tone"], "direct")
            self.assertEqual(inspect_payload["override_precedence"][0], "packaged defaults")
            self.assertEqual(inspect_payload["overrides"][0]["key"], "human_tone")
            self.assertEqual(guide["recommended_workflow"], "product-requirements")
            self.assertEqual(guide["workflow_metadata"]["template"], "quick-dev-artifact")
            self.assertEqual(guide["facilitation_pack"], "skill:facilitation/config-customization.md")
            self.assertIn("override proof", guide["workflow_metadata"]["outputs"])
            product_workflow = next(item for item in index_payload["workflows"] if item["id"] == "product-requirements")
            custom_capability = next(item for item in index_payload["custom_capabilities"] if item["id"] == "config-review")
            facilitator = next(item for item in index_payload["agents"] if item["id"] == "facilitator")
            route_diagnostics = index_payload["route_diagnostics"]
            surface_names = [item["name"] for item in route_diagnostics["surfaces"]]
            self.assertEqual(product_workflow["template"], "quick-dev-artifact")
            self.assertEqual(custom_capability["workflow"], "config-customization")
            self.assertEqual(facilitator["title"], "Project Facilitator")
            self.assertIn("next --json", surface_names)
            self.assertIn("guide --question --json", surface_names)
            self.assertIn("context_boundary", route_diagnostics["fields"])
            self.assertIn("stale_state_guard", route_diagnostics["fields"])
            self.assertTrue((root / written_payload["written_path"]).exists())

            (config_dir / "local.yaml").write_text(
                "\n".join(
                    [
                        'workflow.product-requirements.template: "missing-template"',
                        'workflow.missing-workflow.template: "quick-dev-artifact"',
                        'capability.bad.workflow: "missing-workflow"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            invalid = run_cmd("config", "validate", "--root", str(root), check=False)
            invalid_index = run_cmd("config", "index", "--root", str(root), "--json", check=False)
            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn("references missing template `missing-template`", invalid.stdout)
            self.assertIn("references unknown workflow `missing-workflow`", invalid.stdout)
            self.assertNotEqual(invalid_index.returncode, 0)

    def test_written_capability_index_is_validated_by_config_snapshot_and_gate(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Capability Index Guard Project", "--root", str(root))
            index_path = root / ".forge-method" / "context" / "capability-index.json"
            index_path.parent.mkdir(parents=True, exist_ok=True)
            index_path.write_text(
                json.dumps(
                    {
                        "runtime": "forge-method",
                        "runtime_version": CURRENT_VERSION,
                        "workflows": [
                            {
                                "id": "bad",
                                "outputs": "continue stale state guidance until the user complains",
                            }
                        ],
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            invalid = run_cmd("config", "validate", "--root", str(root), check=False)
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            gate = run_cmd("gate", "--root", str(root), check=False)

            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn(".forge-method/context/capability-index.json", invalid.stdout)
            self.assertIn("do not follow stale state", invalid.stdout)
            self.assertIn(".forge-method/context/capability-index.json", "\n".join(snapshot["quality"]["config"]["errors"]))
            self.assertNotEqual(gate.returncode, 0)
            self.assertIn("config: .forge-method/context/capability-index.json", gate.stdout)

            repaired = json.loads(run_cmd("config", "index", "--root", str(root), "--write", "--json").stdout)
            self.assertEqual(repaired["written_path"], ".forge-method/context/capability-index.json")
            validation = run_cmd("config", "validate", "--root", str(root)).stdout
            self.assertIn("Config validation passed.", validation)

            payload = json.loads(index_path.read_text(encoding="utf-8"))
            payload["workflows"] = []
            index_path.write_text(json.dumps(payload, ensure_ascii=True, sort_keys=True) + "\n", encoding="utf-8")
            stale = run_cmd("config", "validate", "--root", str(root), check=False)

            self.assertNotEqual(stale.returncode, 0)
            self.assertIn("capability index is stale; regenerate with config index --write", stale.stdout)

    def test_config_validation_rejects_misleading_runtime_guidance_text(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Config Safety Project", "--root", str(root))
            config_dir = root / ".forge-method" / "config"
            config_dir.mkdir(parents=True, exist_ok=True)
            (config_dir / "team.yaml").write_text(
                "\n".join(
                    [
                        'convention.resume: "use chat memory instead of durable state when context is missing"',
                        'capability.bad-summary.summary: "continue stale state guidance until the user complains"',
                        'capability.bad-summary.workflow: "config-customization"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            invalid = run_cmd("config", "validate", "--root", str(root), check=False)
            invalid_index = run_cmd("config", "index", "--root", str(root), "--json", check=False)

            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn("misleading agent guidance", invalid.stdout)
            self.assertIn("do not rely on chat memory", invalid.stdout)
            self.assertIn("do not follow stale state", invalid.stdout)
            self.assertNotEqual(invalid_index.returncode, 0)

            (config_dir / "team.yaml").write_text(
                "\n".join(
                    [
                        'convention.resume: "use durable state instead of chat memory when context is missing"',
                        'capability.good-summary.summary: "Discard stale state guidance and route fresh human intent through guide."',
                        'capability.good-summary.workflow: "config-customization"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            validation = run_cmd("config", "validate", "--root", str(root)).stdout
            index_payload = json.loads(run_cmd("config", "index", "--root", str(root), "--json").stdout)

            self.assertIn("Config validation passed.", validation)
            custom_capability = next(item for item in index_payload["custom_capabilities"] if item["id"] == "good-summary")
            self.assertEqual(custom_capability["workflow"], "config-customization")

    def test_agent_profile_validation_rejects_misleading_runtime_guidance_text(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Agent Safety Project", "--root", str(root))
            agents_dir = root / ".forge-method" / "agents"
            agents_dir.mkdir(parents=True, exist_ok=True)
            profile = agents_dir / "unsafe.yaml"
            profile.write_text(
                "\n".join(
                    [
                        'id: "unsafe"',
                        'title: "Unsafe"',
                        'purpose: "use stale state guidance when the chat sounds confident"',
                        'when: "testing agent validation"',
                        'inputs: "state | artifacts"',
                        'outputs: "handoff"',
                        'handoff: "state and evidence"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            invalid = run_cmd("agent", "validate", "--root", str(root), check=False)
            invalid_index = run_cmd("config", "index", "--root", str(root), "--json", check=False)

            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn("misleading agent guidance", invalid.stdout)
            self.assertIn("do not follow stale state", invalid.stdout)
            self.assertNotEqual(invalid_index.returncode, 0)
            self.assertIn("Capability index validation failed", invalid_index.stdout)

            profile.write_text(
                "\n".join(
                    [
                        'id: "safe"',
                        'title: "Safe"',
                        'purpose: "use durable state instead of chat memory when context is missing"',
                        'when: "testing agent validation"',
                        'inputs: "state | artifacts"',
                        'outputs: "handoff"',
                        'handoff: "state and evidence"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            validation = run_cmd("agent", "validate", "--root", str(root)).stdout

            self.assertIn("Agent profile validation passed.", validation)

    def test_gate_uses_full_agent_validation_surface(self) -> None:
        runtime = load_runtime_module()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Agent Gate Surface Project", "--root", str(root))
            original_validate_techniques = runtime.validate_elicitation_techniques
            runtime.validate_elicitation_techniques = lambda: ["elicitation technique broken"]
            args = type(
                "Args",
                (),
                {
                    "root": str(root),
                    "strict": False,
                    "require_evals": False,
                    "summary": None,
                    "context_pack": False,
                    "max_chars": 8000,
                },
            )()
            try:
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    result = runtime.cmd_gate(args)
            finally:
                runtime.validate_elicitation_techniques = original_validate_techniques

        self.assertEqual(result, 1)
        self.assertIn("agent: elicitation technique broken", output.getvalue())

    def test_correct_course_continuation_writes_artifact_without_human_block(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase)

            output = run_cmd(
                "correct-course",
                "--root",
                str(root),
                "--summary",
                "Implementation found a late contradiction in wording.",
                "--impact",
                "acceptance wording is stricter than the approved spec",
                "--next-action",
                "continue with the conservative approved-spec interpretation",
                "--eval",
            ).stdout
            snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            artifact = root / snapshot["state"]["last_correct_course_artifact"]
            self.assertIn("Correct-course artifact:", output)
            self.assertTrue(artifact.exists())
            self.assertEqual(snapshot["state"]["human_input_required"], "false")
            self.assertEqual(snapshot["state"]["status"], "correct-course-continued")
            self.assertEqual(snapshot["state"]["active_workflow"], "correct-course")
            self.assertEqual(snapshot["state"]["last_intent_classification"], "correct-course")
            self.assertEqual(snapshot["state"]["active_guidance_mode"], "correct-course")
            self.assertIn("acceptance wording", snapshot["state"]["last_route_reason"])
            self.assertIn("conservative interpretation", artifact.read_text(encoding="utf-8"))
            self.assertIn("continue with the conservative", snapshot["state"]["next_action"])

    def test_ready_gate_is_mechanical_when_stories_are_done(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase)
            add_decision_source(root)
            run_cmd("story", "add", "--root", str(root), "--id", "story-a", "--title", "Build A", "--acceptance", "A works")
            run_cmd("story", "start", "--root", str(root), "--id", "story-a")
            run_cmd("story", "done", "--root", str(root), "--id", "story-a", "--summary", "A works.", "--check", "unit")

            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)

            self.assertEqual(resume["action"], "run_ready_gate")
            self.assertTrue(resume["mechanical_work_order"]["autonomous"])
            self.assertTrue(resume["mechanical_work_order"]["goal_recommended"])
            self.assertIn("project phase is 5-ready-operate", resume["mechanical_work_order"]["done_when"])

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

            launch_root = Path(raw) / "launch-example"
            run_cmd("example", "create", "--root", str(launch_root), "--module", "launch-ops")
            launch_gate = run_cmd("gate", "--root", str(launch_root), "--require-evals").stdout
            launch_story = launch_root / ".forge-method" / "stories" / "example-start.yaml"
            launch_decision_source = launch_root / ".forge-method" / "artifacts" / "example-validation-map.md"

            self.assertTrue(launch_decision_source.exists())
            self.assertIn(".forge-method/artifacts/example-validation-map.md", launch_story.read_text(encoding="utf-8"))
            self.assertIn("Gate passed.", launch_gate)
            self.assertIn("Evals: 1/1 passed", launch_gate)

    def test_project_create_seeds_real_module_project(self) -> None:
        runtime = load_runtime_module()
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
            input_file = root / ".forge-method" / "inputs" / "initial-facilitation.yaml"
            artifact = root / ".forge-method" / "artifacts" / "project-brief.md"
            load_plan = root / ".forge-method" / "context" / "load-plan.json"
            project_list = run_cmd("project", "list", "--root", str(parent)).stdout
            gate = run_cmd("gate", "--root", str(root), "--require-evals").stdout
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            initial_state_text = state.read_text(encoding="utf-8")
            initial_input_text = input_file.read_text(encoding="utf-8")
            first_answer = (
                "Usuarios: professores independentes. Dor: organizar aulas vagas em plano testavel. "
                "Experiencia: conversa guiada com criterios claros. Restricoes: browser simples sem login. "
                "Sucesso: brief revisavel em dez minutos."
            )
            answer_text = run_cmd(
                "input",
                "answer",
                "--root",
                str(root),
                "--id",
                "initial-facilitation",
                "--answer",
                first_answer,
            ).stdout
            answered_snapshot = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)
            answered_resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)
            answer_guide = runtime.build_guide_payload(root, question=first_answer, max_chars=12000)
            answer_guide_text = run_cmd("guide", "--root", str(root), "--question", first_answer).stdout

            self.assertIn("Project created: Night Watch", create)
            self.assertTrue(state.exists())
            self.assertFalse(story.exists())
            self.assertTrue(input_file.exists())
            self.assertTrue(artifact.exists())
            self.assertTrue(load_plan.exists())
            self.assertIn('phase: "1-discovery"', initial_state_text)
            self.assertIn('status: "waiting-human-input"', initial_state_text)
            self.assertIn('human_input_required: "true"', initial_state_text)
            self.assertIn('active_workflow: "discover-intent"', initial_state_text)
            self.assertIn("answer human input initial-facilitation", initial_state_text)
            self.assertIn("Antes de criar stories ou desenvolver", initial_input_text)
            self.assertEqual(resume["action"], "answer_required_input")
            self.assertFalse(resume["autonomous"])
            self.assertIn("software-builder", artifact.read_text(encoding="utf-8"))
            self.assertIn("night-watch", project_list)
            self.assertIn("Gate passed.", gate)
            self.assertIn("Evals: 1/1 passed", gate)
            self.assertIn("required_next_workflow: discover-intent", answer_text)
            self.assertNotIn("Story added", answer_text)
            self.assertFalse(story.exists())
            self.assertEqual(answered_snapshot["stories"]["total"], 0)
            self.assertEqual(answered_snapshot["state"]["active_workflow"], "discover-intent")
            self.assertEqual(answered_snapshot["state"]["human_input_required"], "false")
            self.assertEqual(answered_snapshot["state"]["status"], "input-resolved")
            self.assertEqual(answered_resume["target"]["workflow"], "discover-intent")
            self.assertTrue(answered_resume["grill_gate_required"])
            self.assertEqual(answer_guide["intent_classification"], "operate-support")
            self.assertEqual(answer_guide["recommended_workflow"], "discover-intent")
            self.assertEqual(answer_guide["recommended_phase"], "1-discovery")
            self.assertEqual(answer_guide["facilitation_pack"], "skill:facilitation/discover-intent.md")
            self.assertIn("First question:", answer_guide["human_prompt"])
            self.assertEqual(
                answer_guide["human_experience"]["human_question"],
                "give me the whole picture first: who is it for, what should change for them, what is fixed or out, what is still open, and what visible or operational proof should close discovery?",
            )
            self.assertIn("Guidance Engine: operate-support -> discover-intent / 1-discovery", answer_guide_text)
            self.assertIn("Grill Gate: required", answer_guide_text)
            self.assertIn(
                "First question: give me the whole picture first: who is it for, what should change for them, what is fixed or out, what is still open, and what visible or operational proof should close discovery?",
                answer_guide_text,
            )
            self.assertNotIn("Prompt: Let's use", answer_guide_text)
            self.assertNotIn("build-story", answer_guide_text)
            blocked_transition = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "2-specification",
                "--status",
                "specification-ready",
                "--workflow",
                "write-spec",
                check=False,
            )
            self.assertNotEqual(blocked_transition.returncode, 0)
            self.assertIn(
                "Discovery closeout required before specification",
                blocked_transition.stderr + blocked_transition.stdout,
            )
            self.assertIn('phase: "1-discovery"', state.read_text(encoding="utf-8"))
            weak_closeout = run_cmd(
                "artifact",
                "add",
                "--root",
                str(root),
                "--kind",
                "discovery-intent",
                "--title",
                "Accepted discovery intent",
                "--summary",
                "Accepted first facilitation answer for specification.",
                "--path",
                ".forge-method/artifacts/discovery-intent.md",
            ).stdout
            weak_check = run_cmd(
                "artifact",
                "discovery-check",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/discovery-intent.md",
                check=False,
            )
            weak_transition = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "2-specification",
                "--status",
                "specification-ready",
                "--workflow",
                "write-spec",
                check=False,
            )
            self.assertIn(".forge-method/artifacts/discovery-intent.md", weak_closeout)
            self.assertNotEqual(weak_check.returncode, 0)
            self.assertIn("discovery closeout requires source_input", weak_check.stdout)
            self.assertNotEqual(weak_transition.returncode, 0)
            self.assertIn(
                "Discovery closeout quality required before specification",
                weak_transition.stderr + weak_transition.stdout,
            )
            discovery_closeout = run_cmd(
                "artifact",
                "discovery-closeout",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/discovery-intent-accepted.md",
                "--audience",
                "independent teachers planning flexible lessons",
                "--outcome",
                "create a guided planning product that turns vague class ideas into reviewable plans",
                "--constraints",
                "browser-first prototype, no login in the first pass, preserve simple language",
                "--non-goals",
                "no scheduling marketplace, no automated grading, no implementation architecture yet",
                "--success-signal",
                "a teacher can produce a reviewable brief with constraints and proof in ten minutes",
                "--open-questions",
                "none blocking; pricing and collaboration can wait",
            ).stdout
            discovery_check = run_cmd(
                "artifact",
                "discovery-check",
                "--root",
                str(root),
                "--path",
                ".forge-method/artifacts/discovery-intent-accepted.md",
            ).stdout
            transition = run_cmd(
                "transition",
                "--root",
                str(root),
                "--phase",
                "2-specification",
                "--status",
                "specification-ready",
                "--workflow",
                "write-spec",
            ).stdout
            self.assertIn(".forge-method/artifacts/discovery-intent-accepted.md", discovery_closeout)
            self.assertIn("Discovery closeout check passed.", discovery_closeout)
            self.assertIn("Discovery closeout check passed.", discovery_check)
            self.assertIn("Transition written.", transition)
            self.assertIn('phase: "2-specification"', state.read_text(encoding="utf-8"))
            accepted_text = (root / ".forge-method" / "artifacts" / "discovery-intent-accepted.md").read_text(
                encoding="utf-8"
            )
            self.assertIn("visible_or_operational_proof:", accepted_text)
            self.assertIn("early_visual_feedback_loop:", accepted_text)

    def test_project_create_brownfield_starts_with_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            parent = Path(raw)
            existing = parent / "existing-app"
            existing.mkdir()
            (existing / "package.json").write_text('{"scripts":{"test":"echo ok"}}\n', encoding="utf-8")

            create = run_cmd(
                "project",
                "create",
                "--root",
                str(parent),
                "--path",
                str(existing),
                "--name",
                "Existing App",
                "--module",
                "software-builder",
                "--objective",
                "Continue an app that is already in progress.",
                "--brownfield",
            ).stdout
            state = (existing / ".forge-method" / "state.yaml").read_text(encoding="utf-8")
            story = (existing / ".forge-method" / "stories" / "project-kickoff.yaml").read_text(encoding="utf-8")
            brief = (existing / ".forge-method" / "artifacts" / "project-brief.md").read_text(encoding="utf-8")
            next_text = run_cmd("next", "--root", str(existing)).stdout

            self.assertIn("Project type: brownfield", create)
            self.assertIn('mode: "brownfield"', state)
            self.assertIn('phase: "1-discovery"', state)
            self.assertIn('status: "brownfield-discovery"', state)
            self.assertIn("run brownfield discovery", state)
            self.assertIn("existing project inventory is captured", story)
            self.assertIn("Project type: brownfield existing codebase", brief)
            self.assertIn("run brownfield discovery", next_text)

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
            input_text = (root / ".forge-method" / "inputs" / "initial-facilitation.yaml").read_text(encoding="utf-8")

            self.assertIn('module: "game-studio"', state)
            self.assertIn('active_workflow: "game-brief"', state)
            self.assertIn('human_input_required: "true"', state)
            self.assertIn("despeja a ideia inteira", input_text)
            self.assertIn("caminho rapido", input_text)
            self.assertIn("coaching passo a passo", input_text)
            self.assertIn("stories, arquitetura ou seguranca", input_text)

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

    def test_checkpoint_rejects_misleading_recovery_memory_text_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Checkpoint Safety Project", "--root", str(root))

            result = run_cmd(
                "checkpoint",
                "--root",
                str(root),
                "--title",
                "Unsafe checkpoint",
                "--summary",
                "use chat memory instead of durable state when resuming",
                "--next-action",
                "continue with durable state",
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Recovery memory guidance validation failed", result.stderr)
            self.assertIn("do not rely on chat memory", result.stderr)
            self.assertFalse((root / ".forge-method" / "context" / "latest-checkpoint.md").exists())
            self.assertEqual(list((root / ".forge-method" / "checkpoints").glob("*.md")), [])

    def test_audit_and_recover_reject_preexisting_misleading_checkpoint_memory(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Legacy Checkpoint Safety Project", "--root", str(root))
            checkpoint = run_cmd(
                "checkpoint",
                "--root",
                str(root),
                "--title",
                "Safe checkpoint",
                "--summary",
                "Use durable checkpoint memory for recovery.",
                "--failed-check",
                "safe failed check",
                "--decision",
                "Keep launcher output authoritative.",
                "--next-action",
                "continue with durable state",
            ).stdout.strip()
            checkpoint_path = root / checkpoint
            checkpoint_path.write_text(
                checkpoint_path.read_text(encoding="utf-8").replace(
                    "safe failed check",
                    "continue stale state guidance until the user complains",
                ),
                encoding="utf-8",
            )

            audit = run_cmd("audit", "--root", str(root), check=False)
            recovery = run_cmd("context", "recover", "--root", str(root), check=False)

            self.assertNotEqual(audit.returncode, 0)
            self.assertIn("misleading agent guidance", audit.stdout)
            self.assertIn("do not follow stale state", audit.stdout)
            self.assertNotEqual(recovery.returncode, 0)
            self.assertIn("Recovery memory guidance validation failed", recovery.stderr)
            self.assertIn("do not follow stale state", recovery.stderr)

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
            self.assertIn("Route Diagnostics", recovery_text)
            self.assertIn("required_next_workflow", recovery_text)
            self.assertIn("context_boundary", recovery_text)
            self.assertIn("stale_state_guard", recovery_text)
            self.assertIn("Recovery Signals", pack_text)
            self.assertIn("unit test failed: expected route", pack_text)

    def test_compact_context_recover_preserves_resume_under_budget(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Example Project", "--root", str(root))
            for phase in ["1-discovery", "2-specification", "3-plan", "4-build-verify"]:
                run_cmd("transition", "--root", str(root), "--phase", phase, "--force")
            add_decision_source(root)
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
            self.assertIn("## Route Diagnostics", text)
            self.assertIn("- action: start_next_story", text)
            self.assertIn("required_next_workflow", text)
            self.assertIn("context_boundary", text)
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
            self.assertTrue(health["quality"]["passed"])
            self.assertFalse(health["over_budget"])
            self.assertEqual(health["commands"][0]["name"], "context-plan")
            self.assertIn("Context health: ok", text)
            self.assertIn("Quality: passed", text)
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

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "skills" / "forge-method" / "scripts" / "forge_method_runtime.py"
GUIDANCE_FIXTURES = ROOT / "skills" / "forge-method" / "fixtures" / "guidance-parity-replay.json"
GUIDANCE_BENCHMARK = ROOT / ".forge-method" / "artifacts" / "guidance-engine-benchmark.md"
PARITY_REQUIRED_FAMILIES = {
    "help",
    "confusion",
    "brainstorm",
    "research",
    "prd",
    "ux",
    "architecture",
    "quick-dev",
    "story-cycle",
    "correct-course",
    "builder",
    "config",
    "persona",
    "cis",
    "game",
    "tea",
    "lifecycle",
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

    def test_preflight_empty_workspace_returns_create_decision(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)

            text = run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game").stdout
            payload = json.loads(
                run_cmd("preflight", "--root", str(root), "--objective", "build a mobile game", "--json").stdout
            )
            start = run_cmd("start", "--root", str(root)).stdout
            guide = run_cmd("guide", "--root", str(root), "--question", "build a mobile game").stdout

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
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            dog_question = "Build a product that turns my dog into a delegate that gives speeches."
            cat_question = "Build a tower that sprays water on a cat when it jumps on tables."

            dog_text = run_cmd("guide", "--root", str(root), "--question", dog_question).stdout
            dog_payload = json.loads(run_cmd("guide", "--root", str(root), "--question", dog_question, "--json").stdout)
            cat_payload = json.loads(run_cmd("guide", "--root", str(root), "--question", cat_question, "--json").stdout)

            self.assertEqual(dog_payload["reality_evidence_gate"]["status"], "blocked")
            self.assertEqual(dog_payload["reality_evidence_gate"]["score"], 0)
            self.assertIn("Reality/Evidence Gate: blocked (0/10)", dog_text)
            self.assertIn("Physical or biological impossibility", dog_text)
            self.assertEqual(cat_payload["reality_evidence_gate"]["status"], "blocked")
            self.assertEqual(cat_payload["reality_evidence_gate"]["score"], 0)
            self.assertIn("Animal-welfare", cat_payload["reality_evidence_gate"]["summary"])

    def test_guidance_engine_routes_transcript_fixtures(self) -> None:
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
        for case in fixtures:
            with self.subTest(case=case["id"]):
                with tempfile.TemporaryDirectory() as raw:
                    root = Path(raw)
                    prepare_guidance_fixture(root, case["state"])

                    payload = json.loads(
                        run_cmd("guide", "--root", str(root), "--question", case["question"], "--json").stdout
                    )

                    self.assertTrue(required_keys <= payload.keys())
                    self.assertTrue(required_keys <= payload["guidance_engine"].keys())
                    self.assertEqual(payload["intent_classification"], case["expected_classification"])
                    self.assertEqual(payload["recommended_workflow"], case["expected_workflow"])
                    self.assertEqual(payload["state_update_required"], case["state_update_required"])
                    self.assertEqual(payload["workflow_metadata"]["id"], case["expected_workflow"])
                    if case.get("expected_phase"):
                        self.assertEqual(payload["recommended_phase"], case["expected_phase"])
                    if case.get("expected_command"):
                        command_names = [item["name"] for item in payload["commands"]]
                        self.assertIn(case["expected_command"], command_names)
                    if case.get("expected_facilitation_pack"):
                        self.assertEqual(payload["facilitation_pack"], case["expected_facilitation_pack"])
                    if case.get("expected_template"):
                        self.assertEqual(payload["workflow_metadata"].get("template"), case["expected_template"])
                    if case.get("expected_persona_lens"):
                        self.assertEqual(payload["persona_lens"].get("id"), case["expected_persona_lens"])
                    elif case["id"] in {"confused_user", "brainstorm_request", "mixed_bmad_parity_runtime_request"}:
                        self.assertTrue(payload["facilitation_pack"].startswith("skill:facilitation/"))
                    if case["id"] == "method_frustration_ready":
                        self.assertNotIn("publish current batch", payload["recommended_action"])
                        command_names = [item["name"] for item in payload["commands"]]
                        self.assertIn("transition-evolve", command_names)
                        self.assertIn("correct-course", command_names)
                        text = run_cmd("guide", "--root", str(root), "--question", case["question"]).stdout
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
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            prepare_guidance_fixture(root, "evolve_runtime")

            polish_question = "quero melhorar a experiencia humana e compactar os docs agenticos"
            polish = json.loads(
                run_cmd("guide", "--root", str(root), "--question", polish_question, "--json").stdout
            )
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
            self.assertLess(polish_text.index("Isto e trabalho no motor do Forge"), polish_text.index("Workspace:"))
            self.assertNotIn("Reality/Evidence Gate", polish_text)
            self.assertLess(len(json.dumps(human, sort_keys=True)), 1800)

            frustration_question = "estou frustrado, nao sei se o Forge esta guiando de verdade"
            frustration = json.loads(
                run_cmd("guide", "--root", str(root), "--question", frustration_question, "--json").stdout
            )
            frustration_text = run_cmd("guide", "--root", str(root), "--question", frustration_question).stdout

            self.assertEqual(frustration["intent_classification"], "correct-course")
            self.assertEqual(frustration["recommended_workflow"], "correct-course")
            self.assertEqual(frustration["reality_evidence_gate"]["status"], "not-applicable")
            self.assertIn("Isto e correcao de rota", frustration_text)
            self.assertLess(frustration_text.index("Isto e correcao de rota"), frustration_text.index("Workspace:"))
            self.assertNotIn("Reality/Evidence Gate", frustration_text)

    def test_guidance_parity_replay_fixture_covers_required_families(self) -> None:
        fixtures = json.loads(GUIDANCE_FIXTURES.read_text(encoding="utf-8"))
        families = {case["family"] for case in fixtures}

        self.assertTrue(PARITY_REQUIRED_FAMILIES <= families)
        for case in fixtures:
            self.assertIn("expected_classification", case)
            self.assertIn("expected_workflow", case)
            self.assertNotIn("bmad-", case["expected_workflow"])

    def test_parity_replay_command_validates_fixture_matrix(self) -> None:
        payload = json.loads(run_cmd("parity", "replay", "--json").stdout)

        self.assertEqual(payload["failed"], 0)
        self.assertEqual(payload["missing_families"], [])
        self.assertTrue(PARITY_REQUIRED_FAMILIES <= set(payload["covered_families"]))
        self.assertEqual(payload["passed"], payload["total"])

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

            self.assertEqual(snapshot["runtime_version"], "1.28.0")
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

            blocked = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertFalse(blocked["quality"]["audit"]["passed"])
            self.assertIn("decision artifact source", "\n".join(blocked["quality"]["audit"]["errors"]))
            self.assertEqual(blocked["route"]["recommendation"], "repair_project_state")
            self.assertEqual(blocked["resume"]["action"], "repair_project_state")
            self.assertEqual(blocked["help_oracle"]["required_next_workflow"], "context-recovery")

            add_decision_source(root)
            released = json.loads(run_cmd("snapshot", "--root", str(root)).stdout)

            self.assertTrue(released["quality"]["audit"]["passed"])
            self.assertEqual(released["route"]["recommendation"], "start_next_story")
            self.assertEqual(released["help_oracle"]["required_next_workflow"], "build-story")

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

            self.assertEqual(snapshot["resume"]["action"], "operate_or_evolve")
            self.assertEqual(snapshot["help_oracle"]["required_next_workflow"], "guidance-engine")
            self.assertEqual(resume["help_oracle"]["required_next_workflow"], "guidance-engine")
            self.assertIn("Ready projects must route", snapshot["help_oracle"]["reason"])
            self.assertIn("Next required workflow: guidance-engine", next_text)
            self.assertNotIn("publish current batch", next_text)

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

            self.assertEqual(snapshot["resume"]["action"], "continue_current_workflow")
            self.assertEqual(snapshot["help_oracle"]["required_next_workflow"], "runtime-builder")
            self.assertIn("Continue the active workflow", snapshot["help_oracle"]["reason"])
            self.assertIn("Implement Help Oracle invariant", next_text)
            self.assertIn("Next required workflow: runtime-builder", next_text)

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
                json.dumps({"name": "forge-method-core", "version": "1.28.0", "skills": "./skills/"}),
                encoding="utf-8",
            )
            skill_path.write_text("---\nname: forge-method\n---\n", encoding="utf-8")
            env = {"HOME": str(home), "USERPROFILE": str(home)}

            payload = json.loads(run_cmd("doctor", "--root", str(home), "--json", env=env).stdout)
            text = run_cmd("doctor", "--root", str(home), env=env).stdout

            plugin = payload["plugin_installation"]
            self.assertTrue(plugin["available"])
            self.assertEqual(plugin["status"], "ready")
            self.assertEqual(plugin["installed_version"], "1.28.0")
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
        workflow_list = run_cmd("workflow", "list").stdout
        guide = json.loads(
            run_cmd(
                "guide",
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
        self.assertIn("workflow-validate", workflow_list)
        for workflow_id in [
            "game-story-creation",
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
            "module-validate",
            "doc-index",
            "spec-distillation",
            "product-requirements",
            "ux-plan",
            "quick-dev",
            "story-creation",
            "track-decision",
            "project-context",
            "session-prep",
            "code-review",
            "retrospective",
            "research-closeout",
        ]:
            self.assertIn(workflow_id, workflow_list)
        for template_path in [
            ROOT / "skills" / "forge-method" / "templates" / "game-lifecycle-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-context-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "engine-setup-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "engine-architecture-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "quick-prototype-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "playtest-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "performance-plan-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "game-qa-review-artifact.md",
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
            ROOT / "skills" / "forge-method" / "templates" / "builder-utility-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "builder-factory-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "module-builder-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "module-validation-report.md",
            ROOT / "skills" / "forge-method" / "templates" / "document-utility-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "product-requirements-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "ux-design-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "quick-dev-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "story-creation-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "track-decision-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "project-context-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "session-prep-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "code-review-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "retrospective-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "readiness-matrix-artifact.md",
            ROOT / "skills" / "forge-method" / "templates" / "research-closeout-artifact.md",
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
        human_facing_required = {
            "product-requirements",
            "ux-plan",
            "quick-dev",
            "story-creation",
            "architecture",
            "create-epics",
            "plan-sprint",
            "readiness-check",
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
            "security-plan",
            "module-ideation",
            "agent-builder",
            "workflow-builder",
            "module-builder",
            "module-validate",
            "config-customization",
            "track-decision",
            "project-context",
            "session-prep",
            "code-review",
            "retrospective",
            "research-closeout",
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
        }
        by_id = {item["id"]: item for item in catalog["workflows"]}
        for workflow_id in human_facing_required:
            self.assertIn("facilitation_pack", by_id[workflow_id], workflow_id)
        self.assertEqual(by_id["product-requirements"].get("template"), "product-requirements-artifact")
        self.assertEqual(by_id["ux-plan"].get("template"), "ux-design-artifact")
        self.assertEqual(by_id["quick-dev"].get("template"), "quick-dev-artifact")
        self.assertEqual(by_id["story-creation"].get("template"), "story-creation-artifact")
        self.assertEqual(by_id["module-ideation"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["agent-builder"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["workflow-builder"].get("template"), "builder-factory-artifact")
        self.assertEqual(by_id["module-builder"].get("template"), "module-builder-artifact")
        self.assertEqual(by_id["module-validate"].get("template"), "module-validation-report")
        self.assertEqual(by_id["config-customization"].get("template"), "config-customization-artifact")
        self.assertEqual(by_id["track-decision"].get("template"), "track-decision-artifact")
        self.assertEqual(by_id["project-context"].get("template"), "project-context-artifact")
        self.assertEqual(by_id["session-prep"].get("template"), "session-prep-artifact")
        self.assertEqual(by_id["code-review"].get("template"), "code-review-artifact")
        self.assertEqual(by_id["retrospective"].get("template"), "retrospective-artifact")
        self.assertEqual(by_id["readiness-check"].get("template"), "readiness-matrix-artifact")
        self.assertEqual(by_id["research-closeout"].get("template"), "research-closeout-artifact")
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
        self.assertEqual(by_id["game-context"].get("template"), "game-context-artifact")
        self.assertEqual(by_id["gdd"].get("template"), "gdd")
        self.assertEqual(by_id["narrative-design"].get("template"), "narrative-bible")
        self.assertEqual(by_id["mechanics-design"].get("template"), "mechanics-matrix")
        self.assertEqual(by_id["engine-setup"].get("template"), "engine-setup-artifact")
        self.assertEqual(by_id["engine-architecture"].get("template"), "engine-architecture-artifact")
        self.assertEqual(by_id["quick-prototype"].get("template"), "quick-prototype-artifact")
        self.assertEqual(by_id["playtest-plan"].get("template"), "playtest-plan-artifact")
        self.assertEqual(by_id["performance-plan"].get("template"), "performance-plan-artifact")
        self.assertEqual(by_id["game-qa-review"].get("template"), "game-qa-review-artifact")
        self.assertIn("validate", by_id["product-requirements"].get("modes", []))
        self.assertIn("validate", by_id["ux-plan"].get("modes", []))
        self.assertIn("spec-lite", by_id["quick-dev"].get("modes", []))
        self.assertIn("validate", by_id["story-creation"].get("modes", []))
        self.assertIn("ideate", by_id["module-ideation"].get("modes", []))
        self.assertIn("create", by_id["agent-builder"].get("modes", []))
        self.assertIn("create", by_id["workflow-builder"].get("modes", []))
        self.assertIn("package", by_id["module-builder"].get("modes", []))
        self.assertIn("validate", by_id["module-validate"].get("modes", []))
        self.assertIn("index", by_id["config-customization"].get("modes", []))
        self.assertIn("decide", by_id["track-decision"].get("modes", []))
        self.assertIn("document", by_id["project-context"].get("modes", []))
        self.assertIn("prep", by_id["session-prep"].get("modes", []))
        self.assertIn("review", by_id["code-review"].get("modes", []))
        self.assertIn("create", by_id["retrospective"].get("modes", []))
        self.assertIn("matrix", by_id["readiness-check"].get("modes", []))
        self.assertIn("closeout", by_id["research-closeout"].get("modes", []))
        self.assertIn("validate", by_id["test-strategy"].get("modes", []))
        self.assertIn("teach", by_id["teach-testing"].get("modes", []))
        self.assertIn("decide", by_id["test-engagement-model"].get("modes", []))
        self.assertIn("fixtures", by_id["test-framework"].get("modes", []))
        self.assertIn("validate", by_id["ci-quality-pipeline"].get("modes", []))
        self.assertIn("validate", by_id["atdd-plan"].get("modes", []))
        self.assertIn("validate", by_id["test-automation"].get("modes", []))
        self.assertIn("review", by_id["test-review"].get("modes", []))
        self.assertIn("waiver", by_id["nfr-evidence-audit"].get("modes", []))
        self.assertIn("phase-2", by_id["traceability-gate"].get("modes", []))
        self.assertIn("document", by_id["game-context"].get("modes", []))
        self.assertIn("create", by_id["gdd"].get("modes", []))
        self.assertIn("create", by_id["narrative-design"].get("modes", []))
        self.assertIn("balance", by_id["mechanics-design"].get("modes", []))
        self.assertIn("setup", by_id["engine-setup"].get("modes", []))
        self.assertIn("create", by_id["engine-architecture"].get("modes", []))
        self.assertIn("prove", by_id["quick-prototype"].get("modes", []))
        self.assertIn("run", by_id["playtest-plan"].get("modes", []))
        self.assertIn("measure", by_id["performance-plan"].get("modes", []))
        self.assertIn("review", by_id["game-qa-review"].get("modes", []))
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
        self.assertEqual(version.strip(), "1.28.0")

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

            self.assertEqual(plan["runtime_version"], "1.28.0")
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

            lens_ids = {item["id"] for item in index_payload["persona_lenses"]}
            technique_ids = {item["id"] for item in index_payload["elicitation_techniques"]}
            self.assertTrue({"product-manager", "architect", "ux-designer", "qa-strategist", "game-designer", "builder", "tech-writer"} <= lens_ids)
            self.assertIn("risk-inversion", technique_ids)
            self.assertNotIn("persona", index_payload["agents"][0])

            self.assertIn("Persona lens: UX Designer Lens", council)
            self.assertIn("[Facilitator]", council)
            self.assertIn("[Spec Architect]", council)
            self.assertIn("[Quality Reviewer]", council)
            self.assertTrue((root / snapshot["state"]["last_council_artifact"]).exists())

    def test_lifecycle_closure_guidance_and_compact_contracts(self) -> None:
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
            run_cmd("init", "--project", "Lifecycle Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")

            for question, workflow, template, phase in lifecycle_cases:
                with self.subTest(workflow=workflow):
                    guide = json.loads(
                        run_cmd("guide", "--root", str(root), "--question", question, "--json").stdout
                    )

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
            prepare_guidance_fixture(root, "build_story_ready")
            review = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "review this code diff and create actionable findings before readiness",
                    "--json",
                ).stdout
            )

            self.assertEqual(review["intent_classification"], "lifecycle-flow")
            self.assertEqual(review["recommended_workflow"], "code-review")
            self.assertEqual(review["recommended_phase"], "4-build-verify")
            self.assertEqual(review["workflow_metadata"].get("template"), "code-review-artifact")
            self.assertEqual(review["facilitation_pack"], "skill:facilitation/lifecycle-closure.md")
            self.assertTrue(review["state_update_required"])

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            prepare_guidance_fixture(root, "evolve_runtime")
            session = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "prep next session with read order blockers first command and next workflow",
                    "--json",
                ).stdout
            )
            p14 = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "continue P1.4 Product Context Review Retrospective Closure from the systematic parity plan",
                    "--json",
                ).stdout
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
        game_cases = [
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
                "game QA review this playable slice for playability feedback stability performance evidence and repair route",
                "game-qa-review",
                "game-qa-review-artifact",
                "4-build-verify",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            run_cmd("init", "--project", "Game Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")

            for question, workflow, template, phase in game_cases:
                with self.subTest(workflow=workflow):
                    guide = json.loads(
                        run_cmd("guide", "--root", str(root), "--question", question, "--json").stdout
                    )

                    self.assertEqual(guide["intent_classification"], "game-flow")
                    self.assertEqual(guide["recommended_workflow"], workflow)
                    self.assertEqual(guide["recommended_phase"], phase)
                    self.assertEqual(guide["facilitation_pack"], "skill:facilitation/game-lifecycle.md")
                    self.assertEqual(guide["workflow_metadata"].get("template"), template)
                    self.assertTrue(guide["state_update_required"])
                    self.assertIn("transition-workflow", [item["name"] for item in guide["commands"]])

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
                    "playtest-plan",
                    "performance-plan",
                    "game-qa-review",
                }
                <= workflow_ids
            )

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            prepare_guidance_fixture(root, "evolve_runtime")
            p15 = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "continue P1.5 Game Studio Depth from the systematic parity plan",
                    "--json",
                ).stdout
            )

            self.assertEqual(p15["intent_classification"], "builder-flow")
            self.assertEqual(p15["recommended_workflow"], "runtime-builder")
            self.assertEqual(p15["facilitation_pack"], "skill:facilitation/runtime-builder.md")
            self.assertFalse(p15["state_update_required"])

        for ref_name in [
            "workflow-game-context.md",
            "workflow-engine-setup.md",
            "workflow-gdd.md",
            "workflow-engine-architecture.md",
            "workflow-quick-prototype.md",
            "workflow-playtest-plan.md",
            "workflow-performance-plan.md",
            "workflow-game-qa-review.md",
        ]:
            ref_text = (ROOT / "skills" / "forge-method" / "references" / ref_name).read_text(encoding="utf-8")
            self.assertIn("trigger:", ref_text, ref_name)
            self.assertIn("handoff:", ref_text, ref_name)
            self.assertLess(len(ref_text), 1700, ref_name)

    def test_tea_depth_guidance_and_compact_contracts(self) -> None:
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
            run_cmd("init", "--project", "Quality Project", "--root", str(root))
            run_cmd("transition", "--root", str(root), "--phase", "1-discovery")

            for question, workflow, template, phase in tea_cases:
                with self.subTest(workflow=workflow):
                    guide = json.loads(
                        run_cmd("guide", "--root", str(root), "--question", question, "--json").stdout
                    )

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
            prepare_guidance_fixture(root, "evolve_runtime")
            p16 = json.loads(
                run_cmd(
                    "guide",
                    "--root",
                    str(root),
                    "--question",
                    "continue P1.6 Test Architecture Enterprise Depth from the systematic parity plan",
                    "--json",
                ).stdout
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
            self.assertIn("Persisted decision artifact:", council)
            self.assertEqual(snapshot["state"]["track"], "game-studio")
            self.assertEqual(snapshot["state"]["module"], "game-studio")
            self.assertTrue((root / snapshot["state"]["last_council_artifact"]).exists())
            self.assertTrue((root / workflow_path).exists())
            self.assertIn("Builder validation passed.", builder_validation)
            self.assertEqual(config["sources"], [])
            self.assertIn("Config validation passed.", config_validation)
            self.assertIn("Gate passed.", gate)

    def test_mechanical_work_order_goal_and_commit_policy_contracts(self) -> None:
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
            guide = json.loads(run_cmd("guide", "--root", str(root), "--json").stdout)
            next_text = run_cmd("next", "--root", str(root)).stdout
            config_validation = run_cmd("config", "validate", "--root", str(root)).stdout

            work_order = resume["mechanical_work_order"]
            self.assertFalse(resume["grill_gate_required"])
            self.assertEqual(resume["action"], "start_next_story")
            self.assertTrue(work_order["autonomous"])
            self.assertTrue(work_order["goal_recommended"])
            self.assertEqual(work_order["commit_policy"], "epic")
            self.assertIn("required check fails", work_order["self_repair_when"])
            self.assertIn("missing external credential or access", work_order["stop_only_when"])
            self.assertTrue(resume["codex_goal_handoff"]["recommended"])
            self.assertIn("/goal", resume["codex_goal_handoff"]["command"])
            self.assertEqual(guide["mechanical_work_order"]["next_mechanical_step"], work_order["next_mechanical_step"])
            self.assertIn("Goal recommended", next_text)
            self.assertNotIn("ok?", next_text.lower())
            self.assertNotIn("continue?", next_text.lower())
            self.assertNotIn("quer continuar", next_text.lower())
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
            self.assertEqual(product_workflow["template"], "quick-dev-artifact")
            self.assertEqual(custom_capability["workflow"], "config-customization")
            self.assertEqual(facilitator["title"], "Project Facilitator")
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
            input_file = root / ".forge-method" / "inputs" / "initial-facilitation.yaml"
            artifact = root / ".forge-method" / "artifacts" / "project-brief.md"
            load_plan = root / ".forge-method" / "context" / "load-plan.json"
            project_list = run_cmd("project", "list", "--root", str(parent)).stdout
            gate = run_cmd("gate", "--root", str(root), "--require-evals").stdout
            resume = json.loads(run_cmd("resume", "--root", str(root), "--json").stdout)

            self.assertIn("Project created: Night Watch", create)
            self.assertTrue(state.exists())
            self.assertFalse(story.exists())
            self.assertTrue(input_file.exists())
            self.assertTrue(artifact.exists())
            self.assertTrue(load_plan.exists())
            self.assertIn('phase: "1-discovery"', state.read_text(encoding="utf-8"))
            state_text = state.read_text(encoding="utf-8")
            input_text = input_file.read_text(encoding="utf-8")
            self.assertIn('status: "waiting-human-input"', state_text)
            self.assertIn('human_input_required: "true"', state_text)
            self.assertIn('active_workflow: "discover-intent"', state_text)
            self.assertIn("answer human input initial-facilitation", state_text)
            self.assertIn("Antes de criar stories ou desenvolver", input_text)
            self.assertEqual(resume["action"], "answer_required_input")
            self.assertFalse(resume["autonomous"])
            self.assertIn("software-builder", artifact.read_text(encoding="utf-8"))
            self.assertIn("night-watch", project_list)
            self.assertIn("Gate passed.", gate)
            self.assertIn("Evals: 1/1 passed", gate)

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
            self.assertIn("fantasia do jogador", input_text)
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

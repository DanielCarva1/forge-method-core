import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
UPDATER = ROOT / "skills" / "forge-method" / "scripts" / "forge_method_updater.py"


def load_updater():
    spec = importlib.util.spec_from_file_location("forge_method_updater", UPDATER)
    if spec is None or spec.loader is None:
        raise AssertionError("Could not load forge_method_updater")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def make_plugin_root(raw: str, *, git_marketplace: bool = True, version: str = "1.24.0") -> Path:
    root = Path(raw)
    skill = root / "skills" / "forge-method"
    (skill / "scripts").mkdir(parents=True)
    (skill / "SKILL.md").write_text("old skill\n", encoding="utf-8")
    write_json(root / ".codex-plugin" / "plugin.json", {"name": "forge-method-core", "version": version})
    if git_marketplace:
        write_json(
            root / ".codex-marketplace-install.json",
            {
                "source_type": "git",
                "source": "https://github.com/DanielCarva1/forge-method-core.git",
                "ref_name": "main",
                "revision": "oldrev",
            },
        )
    return root


def run_updater(skill_dir: Path, *args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    return subprocess.run(
        [sys.executable, str(UPDATER), "--skill-dir", str(skill_dir), "--", *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=process_env,
    )


def run_manual_updater(skill_dir: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    return subprocess.run(
        [sys.executable, str(UPDATER), "--skill-dir", str(skill_dir), "--manual"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=process_env,
    )


class UpdaterTests(unittest.TestCase):
    def test_semver_and_entrypoint_detection(self) -> None:
        updater = load_updater()

        self.assertTrue(updater.is_newer_version("1.25.0", "1.24.9"))
        self.assertFalse(updater.is_newer_version("1.24.0", "1.24.0"))
        self.assertTrue(updater.should_run_for_args([]))
        self.assertTrue(updater.should_run_for_args(["preflight", "--json"]))
        self.assertTrue(updater.should_run_for_args(["reload", "--root", "."]))
        self.assertFalse(updater.should_run_for_args(["--help"]))
        self.assertFalse(updater.should_run_for_args(["story", "list"]))

    def test_policy_defaults_and_overrides(self) -> None:
        updater = load_updater()
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True)
            old_policy = os.environ.get("FORGE_METHOD_UPDATE_POLICY")
            try:
                os.environ.pop("FORGE_METHOD_UPDATE_POLICY", None)
                self.assertEqual("auto", updater.effective_policy(root))
                os.environ["FORGE_METHOD_UPDATE_POLICY"] = "notify"
                self.assertEqual("notify", updater.effective_policy(root))
                os.environ["FORGE_METHOD_UPDATE_POLICY"] = "off"
                self.assertEqual("off", updater.effective_policy(root))
                os.environ["FORGE_METHOD_UPDATE_POLICY"] = "nonsense"
                self.assertEqual("off", updater.effective_policy(root))
            finally:
                if old_policy is None:
                    os.environ.pop("FORGE_METHOD_UPDATE_POLICY", None)
                else:
                    os.environ["FORGE_METHOD_UPDATE_POLICY"] = old_policy

    def test_legacy_install_prints_migration_hint_once_without_stdout(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=False)
            state = Path(raw) / "state" / "update-state.json"
            env = {"FORGE_METHOD_UPDATE_STATE": str(state)}

            first = run_updater(root / "skills" / "forge-method", "preflight", "--json", env=env)
            second = run_updater(root / "skills" / "forge-method", "preflight", "--json", env=env)

            self.assertEqual(first.returncode, 0)
            self.assertEqual(first.stdout, "")
            self.assertIn("automatic updates require the Git marketplace install", first.stderr)
            self.assertEqual(second.stdout, "")
            self.assertEqual(second.stderr, "")

    def test_auto_update_prints_patch_notes_once_on_stderr(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True)
            fake_codex = Path(raw) / "fake_codex.py"
            fake_codex.write_text(
                "\n".join(
                    [
                        "import json, os",
                        "from pathlib import Path",
                        "root = Path(os.environ['FAKE_FORGE_ROOT'])",
                        "(root / '.codex-plugin').mkdir(exist_ok=True)",
                        "(root / '.codex-plugin' / 'plugin.json').write_text(json.dumps({'name': 'forge-method-core', 'version': '1.25.0'}), encoding='utf-8')",
                        "(root / 'skills' / 'forge-method' / 'SKILL.md').write_text('new skill\\n', encoding='utf-8')",
                        "notes = root / 'release-notes'",
                        "notes.mkdir(exist_ok=True)",
                        "(notes / 'latest.json').write_text(json.dumps({'version': '1.25.0', 'summary': 'Self-update is now automatic.', 'highlights': ['Updates before start', 'Patch notes feed'], 'full_notes_url': 'https://example.test/notes'}), encoding='utf-8')",
                        "meta_path = root / '.codex-marketplace-install.json'",
                        "meta = json.loads(meta_path.read_text(encoding='utf-8'))",
                        "meta['revision'] = 'newrev'",
                        "meta_path.write_text(json.dumps(meta), encoding='utf-8')",
                        "print('upgraded')",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            state = Path(raw) / "state" / "update-state.json"
            env = {
                "FAKE_FORGE_ROOT": str(root),
                "FORGE_METHOD_CODEX": f"{sys.executable} {fake_codex}",
                "FORGE_METHOD_UPDATE_STATE": str(state),
            }

            first = run_updater(root / "skills" / "forge-method", "preflight", "--root", ".", "--json", env=env)
            second = run_updater(root / "skills" / "forge-method", "preflight", "--root", ".", "--json", env=env)

            self.assertEqual(first.returncode, 0)
            self.assertEqual(first.stdout, "")
            self.assertIn("Forge Method updated: 1.24.0 -> 1.25.0", first.stderr)
            self.assertIn("Self-update is now automatic.", first.stderr)
            self.assertIn("- Updates before start", first.stderr)
            self.assertIn("Skill instructions changed", first.stderr)
            self.assertEqual(second.returncode, 0)
            self.assertEqual(second.stdout, "")
            self.assertEqual(second.stderr, "")

    def test_manual_update_prints_patch_notes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True)
            fake_codex = Path(raw) / "fake_codex.py"
            fake_codex.write_text(
                "\n".join(
                    [
                        "import json, os",
                        "from pathlib import Path",
                        "root = Path(os.environ['FAKE_FORGE_ROOT'])",
                        "(root / '.codex-plugin' / 'plugin.json').write_text(json.dumps({'name': 'forge-method-core', 'version': '1.33.0'}), encoding='utf-8')",
                        "(root / 'skills' / 'forge-update').mkdir(parents=True, exist_ok=True)",
                        "(root / 'skills' / 'forge-update' / 'SKILL.md').write_text('new update skill\\n', encoding='utf-8')",
                        "notes = root / 'release-notes'",
                        "notes.mkdir(exist_ok=True)",
                        "(notes / 'latest.json').write_text(json.dumps({'version': '1.33.0', 'summary': 'Manual updates now have a button.', 'highlights': ['forge-update skill', 'human patch notes'], 'full_notes_url': 'https://example.test/notes'}), encoding='utf-8')",
                        "meta_path = root / '.codex-marketplace-install.json'",
                        "meta = json.loads(meta_path.read_text(encoding='utf-8'))",
                        "meta['revision'] = 'newrev'",
                        "meta_path.write_text(json.dumps(meta), encoding='utf-8')",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            env = {
                "FAKE_FORGE_ROOT": str(root),
                "FORGE_METHOD_CODEX": f"{sys.executable} {fake_codex}",
                "FORGE_METHOD_UPDATE_STATE": str(Path(raw) / "state" / "update-state.json"),
            }

            result = run_manual_updater(root / "skills" / "forge-method", env=env)

            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("Forge Method updated: 1.24.0 -> 1.33.0", result.stderr)
            self.assertIn("Manual updates now have a button.", result.stderr)
            self.assertIn("- forge-update skill", result.stderr)
            self.assertIn("Skill instructions changed", result.stderr)

    def test_manual_update_already_current_is_quiet(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True, version="1.33.0")
            fake_codex = Path(raw) / "fake_codex.py"
            fake_codex.write_text(
                "\n".join(
                    [
                        "import os",
                        "from pathlib import Path",
                        "Path(os.environ['FAKE_FORGE_ROOT']).exists()",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            env = {
                "FAKE_FORGE_ROOT": str(root),
                "FORGE_METHOD_CODEX": f"{sys.executable} {fake_codex}",
                "FORGE_METHOD_UPDATE_STATE": str(Path(raw) / "state" / "update-state.json"),
            }

            result = run_manual_updater(root / "skills" / "forge-method", env=env)

            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("Forge Method is already up to date: 1.33.0", result.stderr)

    def test_manual_update_non_git_marketplace_explains_install_shape(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=False, version="1.32.0")

            result = run_manual_updater(root / "skills" / "forge-method")

            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("Forge Method current version: 1.32.0", result.stderr)
            self.assertIn("Manual update requires the Git marketplace install.", result.stderr)
            self.assertIn("codex plugin marketplace add DanielCarva1/forge-method-core --ref main", result.stderr)

    def test_manual_update_failure_keeps_install_usable(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True, version="1.33.0")

            result = run_manual_updater(
                root / "skills" / "forge-method",
                env={"FORGE_METHOD_CODEX": str(Path(raw) / "missing-codex"), "FORGE_METHOD_UPDATE_STATE": str(Path(raw) / "state.json")},
            )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("Forge Method update failed to start; your current install was left usable.", result.stderr)

    def test_skip_update_suppresses_all_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = make_plugin_root(raw, git_marketplace=True)
            result = run_updater(
                root / "skills" / "forge-method",
                "preflight",
                env={"FORGE_METHOD_SKIP_UPDATE": "1"},
            )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(result.stderr, "")


if __name__ == "__main__":
    unittest.main()

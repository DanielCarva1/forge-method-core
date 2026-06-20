import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts" / "test-runner.py"


def write_sample_tests(root: Path) -> None:
    tests = root / "tests"
    tests.mkdir()
    (tests / "__init__.py").write_text("", encoding="utf-8")
    (tests / "test_sample.py").write_text(
        "\n".join(
            [
                "import time",
                "import unittest",
                "",
                "class SampleTests(unittest.TestCase):",
                "    def test_pass(self):",
                "        self.assertTrue(True)",
                "",
                "    def test_fail(self):",
                "        print('failure diagnostic')",
                "        self.fail('boom')",
                "",
                "    def test_slow(self):",
                "        time.sleep(0.05)",
                "        self.assertTrue(True)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def run_runner(root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(RUNNER), *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


class TestRunnerTests(unittest.TestCase):
    def test_report_and_junit_for_filtered_run(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_sample_tests(root)
            report = root / "report.json"
            junit = root / "junit.xml"

            result = run_runner(root, "--match", "test_pass", "--report", str(report), "--junit", str(junit))

            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(payload["summary"]["total"], 1)
            self.assertEqual(payload["summary"]["passed"], 1)
            self.assertEqual(payload["tests"][0]["id"], "tests.test_sample.SampleTests.test_pass")
            self.assertTrue(junit.exists())
            self.assertIn("Responsive unit test run passed", result.stdout)

    def test_debug_retains_failure_output_and_reruns_from_report(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_sample_tests(root)
            report = root / "failure-report.json"
            output_dir = root / "debug-output"

            first = run_runner(
                root,
                "--match",
                "test_fail",
                "--debug",
                "--report",
                str(report),
                "--output-dir",
                str(output_dir),
            )

            self.assertEqual(first.returncode, 1)
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(payload["summary"]["failed"], 1)
            output_path = Path(payload["tests"][0]["output_path"])
            self.assertTrue(output_path.exists())
            self.assertIn("failure diagnostic", output_path.read_text(encoding="utf-8"))
            self.assertIn("Debug next steps", first.stdout)

            second = run_runner(root, "--rerun-failures", str(report), "--report", str(root / "rerun.json"))

            self.assertEqual(second.returncode, 1)
            self.assertIn("tests.test_sample.SampleTests.test_fail", second.stdout)

    def test_debug_retains_slow_test_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_sample_tests(root)
            report = root / "slow-report.json"
            output_dir = root / "debug-output"

            result = run_runner(
                root,
                "--match",
                "test_slow",
                "--debug",
                "--slow-threshold",
                "0.01",
                "--report",
                str(report),
                "--output-dir",
                str(output_dir),
            )

            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(payload["summary"]["slow"], 1)
            self.assertTrue(Path(payload["tests"][0]["output_path"]).exists())
            self.assertIn("Optimization next steps", result.stdout)


if __name__ == "__main__":
    unittest.main()

import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def find_bash() -> str | None:
    if os.name == "nt":
        for candidate in (
            Path("C:/Program Files/Git/usr/bin/bash.exe"),
            Path("C:/Program Files/Git/bin/bash.exe"),
        ):
            if candidate.exists():
                return str(candidate)
        return None
    return shutil.which("bash")


class ShellScriptTests(unittest.TestCase):
    def run_bash(self, *args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        bash = find_bash()
        if not bash:
            self.skipTest("bash not available")
        process_env = os.environ.copy()
        if env:
            process_env.update(env)
        return subprocess.run(
            [bash, *args],
            cwd=ROOT,
            env=process_env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=20,
        )

    def test_verify_fast_accepts_empty_test_and_match_arrays_with_nounset(self) -> None:
        result = self.run_bash("scripts/verify-fast.sh", "--no-report", env={"PYTHON": "/usr/bin/true"})

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Fast verification checks passed.", result.stdout)

    def test_verify_fast_missing_option_value_is_friendly(self) -> None:
        result = self.run_bash("scripts/verify-fast.sh", "--test")

        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("Missing value for --test", result.stderr)
        self.assertNotIn("unbound variable", result.stderr)

    def test_verify_all_missing_option_value_is_friendly(self) -> None:
        result = self.run_bash("scripts/verify-all.sh", "--workers")

        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("Missing value for --workers", result.stderr)
        self.assertNotIn("unbound variable", result.stderr)


if __name__ == "__main__":
    raise SystemExit(unittest.main())

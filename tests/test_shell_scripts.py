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
    def test_verify_fast_accepts_empty_test_and_match_arrays_with_nounset(self) -> None:
        bash = find_bash()
        if not bash:
            self.skipTest("bash not available")

        env = os.environ.copy()
        env["PYTHON"] = "/usr/bin/true"
        result = subprocess.run(
            [bash, "scripts/verify-fast.sh", "--no-report"],
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=20,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Fast verification checks passed.", result.stdout)


if __name__ == "__main__":
    raise SystemExit(unittest.main())

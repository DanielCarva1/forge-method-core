#!/usr/bin/env python3
"""Regression check for Start Forge's user-facing activation contract."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skill" / "start-forge" / "SKILL.md"


class GuidedActivationContractTests(unittest.TestCase):
    def test_skill_requires_evidence_backed_project_orientation(self) -> None:
        text = SKILL.read_text(encoding="utf-8")
        match = re.search(
            r"<!-- guided-activation-contract:start -->(.*?)"
            r"<!-- guided-activation-contract:end -->",
            text,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(match, "guided activation contract is missing")
        contract = match.group(1)
        normalized = " ".join(contract.split()).casefold()

        required_terms = (
            "greenfield",
            "brownfield_unmanaged",
            "brownfield_managed",
            "What this project is",
            "Where it is now",
            "What happened recently",
            "What is already planned",
            "What is missing or uncertain",
            "The next best step",
            "Why this step is recommended",
        )
        for term in required_terms:
            with self.subTest(term=term):
                self.assertIn(term.casefold(), normalized)

        self.assertIn("inspect the repository", normalized)
        self.assertIn("before asking the human", normalized)
        self.assertIn("do not ask the human to reconstruct", normalized)


if __name__ == "__main__":
    unittest.main()

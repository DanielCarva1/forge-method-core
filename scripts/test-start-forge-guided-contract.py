#!/usr/bin/env python3
"""Regression checks for Start Forge's user-facing activation journeys."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skill" / "start-forge" / "SKILL.md"
GETTING_STARTED = ROOT / "docs" / "getting-started.md"
AGENT_INTEGRATION = ROOT / "docs" / "agent-integration.md"
SOLO_SPEC = ROOT / "contracts" / "spec" / "solo-dogfood-readiness-v0.yaml"
PRODUCT_CONSTITUTION = (
    ROOT / "contracts" / "policies" / "agent-native-product-constitution.yaml"
)
ASSURANCE_ARCHITECTURE = (
    ROOT / "contracts" / "spec" / "agent-native-assurance-architecture.yaml"
)
RUNTIME_BUNDLE = (
    ROOT
    / "contracts"
    / "workflow-governance"
    / "runtime-universal-assurance-candidate-v0.yaml"
)
START_E2E = ROOT / "crates" / "forge-core-cli" / "tests" / "start_cli_e2e.rs"


def marked_section(text: str, name: str) -> str:
    match = re.search(
        rf"<!-- {re.escape(name)}:start -->(.*?)"
        rf"<!-- {re.escape(name)}:end -->",
        text,
        flags=re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"{name} section is missing")
    return match.group(1)


def normalized(text: str) -> str:
    return " ".join(text.split()).casefold()

def yaml_policy_block(text: str, policy_id: str) -> str:
    marker = f"  - id: {policy_id}"
    start = text.find(marker)
    if start < 0:
        raise AssertionError(f"policy is missing: {policy_id}")
    end = text.find("\n  - id: ", start + len(marker))
    return text[start:] if end < 0 else text[start:end]



class GuidedActivationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.skill = SKILL.read_text(encoding="utf-8")
        cls.contract = marked_section(cls.skill, "guided-activation-contract")
        cls.contract_normalized = normalized(cls.contract)

    def assert_contract_contains(self, *terms: str) -> None:
        for term in terms:
            with self.subTest(term=term):
                self.assertIn(term.casefold(), self.contract_normalized)

    def test_orientation_is_evidence_backed_and_complete(self) -> None:
        self.assert_contract_contains(
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
            "inspect the repository",
            "before asking the human",
            "Do not ask the human to reconstruct",
        )

    def test_language_and_technical_detail_are_balanced(self) -> None:
        self.assert_contract_contains(
            "language already used by the human",
            "keep all explanatory prose consistently in that language",
            "Do not alternate languages",
            "Technical detail is welcome",
            "must never be the whole explanation",
            "practical meaning",
            "Literal commands, paths, source identifiers, and product names",
        )

    def test_orientation_does_not_replace_action_or_create_fake_questions(self) -> None:
        self.assert_contract_contains(
            "Orientation is a checkpoint, not a stopping point",
            "perform and verify it in the same turn",
            "instead of merely announcing",
            "ask exactly one concise question",
            "If no human input is needed, say so plainly and continue",
        )

    def test_resume_refresh_is_bounded_by_real_state_changes(self) -> None:
        self.assert_contract_contains(
            "reuse the same v6 response",
            "Never run two resume commands consecutively",
            "a successful operation that can change workflow evaluation must intervene",
            "Do not refresh after repository inspection, validation, tests, status, report, or help",
        )
        integration = normalized(AGENT_INTEGRATION.read_text(encoding="utf-8"))
        for expected in (
            "never issues two consecutive resume calls",
            "reuses the complete current response",
            "operation capable of changing the workflow evaluation",
        ):
            with self.subTest(integration=expected):
                self.assertIn(expected, integration)

    def test_solo_work_uses_concrete_executable_steps_before_human_escalation(self) -> None:
        self.assert_contract_contains(
            "compatible with the active objective",
            "begin that work in the same turn",
            "do not treat an abstract capability label as the task itself",
            "exhaust concrete Solo Cooperative packets and reversible local work",
            "must not be replaced by an unrelated validation command",
        )

    def test_all_primary_activation_journeys_have_closed_behavior(self) -> None:
        matrix = marked_section(self.contract, "guided-activation-journeys")
        rows: dict[str, tuple[str, str, str]] = {}
        for line in matrix.splitlines():
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if len(cells) != 4 or not cells[0].startswith("`"):
                continue
            journey = cells[0].strip("`")
            self.assertNotIn(journey, rows, f"duplicate journey row: {journey}")
            rows[journey] = (cells[1], cells[2], cells[3])

        expected = {
            "greenfield",
            "brownfield_unmanaged",
            "brownfield_managed",
            "state_loss_or_integrity_failure",
            "runtime_or_bridge_unavailable",
            "human_decision_required",
            "autonomous_action_available",
        }
        self.assertEqual(set(rows), expected)
        for journey, cells in rows.items():
            with self.subTest(journey=journey):
                self.assertTrue(all(cells), f"incomplete journey row: {journey}")

        self.assertIn("Ask one concise outcome question", rows["greenfield"][2])
        self.assertIn("highest-ranked feasible safe action", rows["brownfield_managed"][2])
        self.assertIn("nothing will be recreated", rows["state_loss_or_integrity_failure"][1])
        self.assertIn("Do not initialize or switch roots", rows["runtime_or_bridge_unavailable"][2])
        self.assertIn("Ask exactly one concise question", rows["human_decision_required"][2])
        self.assertIn("Execute in the same turn", rows["autonomous_action_available"][2])

    def test_human_and_integrator_guides_preserve_the_same_experience(self) -> None:
        getting_started = normalized(GETTING_STARTED.read_text(encoding="utf-8"))
        agent_integration = normalized(AGENT_INTEGRATION.read_text(encoding="utf-8"))
        for document in (getting_started, agent_integration):
            self.assertIn("language already used", document)
            self.assertIn("technical", document)
            self.assertIn("practical meaning", document)
            self.assertIn("same turn", document)
        self.assertIn("exactly one concise question", getting_started)

    def test_consequential_uncertainty_drives_autonomous_research(self) -> None:
        research = normalized(
            marked_section(self.skill, "uncertainty-driven-research")
        )
        for expected in (
            "do not wait for the human to tell you to research",
            "decide whether the uncertainty is consequential",
            "research autonomously",
            "multiple credible and independent sources",
            "competing hypotheses",
            "contrary evidence",
            "explain the result and its product impact",
            "continue with the next safe action",
            "ask the human only",
            "forge-core research source add",
            "forge-core research source list",
            "forge-core research cite",
            "forge-core research check",
            "forge-core research graph",
            "do not register every search result or trivial fact",
            "registration proves provenance and resolvability, not that a source is true",
            "never decide whether research is needed or how broad it should be",
        ):
            with self.subTest(expected=expected):
                self.assertIn(expected, research)
        integration = normalized(AGENT_INTEGRATION.read_text(encoding="utf-8"))
        for expected in (
            "does not wait for the human to request research",
            "compares competing hypotheses, contrary evidence",
            "continues with the next safe action",
            "research source add",
            "registration proves provenance and resolvability, not truth",
            "do not register every search result or trivial fact",
        ):
            with self.subTest(integration=expected):
                self.assertIn(expected, integration)
        getting_started = normalized(GETTING_STARTED.read_text(encoding="utf-8"))
        for expected in (
            "does not need to tell the agent to research",
            "compares competing explanations and contrary evidence",
            "keeps working",
            "do not register every search result or small fact",
            "not that the information is automatically true",
        ):
            with self.subTest(getting_started=expected):
                self.assertIn(expected, getting_started)
        constitution = normalized(
            PRODUCT_CONSTITUTION.read_text(encoding="utf-8")
        )
        architecture = normalized(
            ASSURANCE_ARCHITECTURE.read_text(encoding="utf-8")
        )
        runtime_policy = normalized(
            yaml_policy_block(
                RUNTIME_BUNDLE.read_text(encoding="utf-8"),
                "policy.workflow.investigation",
            )
        )
        self.assertIn("research and competence acquisition", constitution)
        self.assertIn("consequential uncertainty must be researched", constitution)
        self.assertIn(
            "research multiple credible and independent sources", architecture
        )
        self.assertIn("competing hypotheses", runtime_policy)
        self.assertIn("contrary evidence", runtime_policy)
        self.assertIn("remaining uncertainty", runtime_policy)


    def test_product_readiness_spec_requires_primary_guided_journeys(self) -> None:
        specification = normalized(SOLO_SPEC.read_text(encoding="utf-8"))
        required_scenarios = (
            "greenfield orientation",
            "existing unmanaged project",
            "existing managed project",
            "state-loss or integrity failure",
            "runtime or host-bridge failure",
            "human-facing prose stays in the human's language",
            "material ambiguity produces one concise decision request",
            (
                "consequential uncertainty triggers autonomous research without "
                "waiting for a human instruction"
            ),
            "agent-autonomous reversible action proceeds without human confirmation",
        )
        for scenario in required_scenarios:
            with self.subTest(scenario=scenario):
                self.assertIn(scenario, specification)

    def test_fresh_start_e2e_uses_current_solo_objective_status(self) -> None:
        source = START_E2E.read_text(encoding="utf-8")
        match = re.search(
            r"fn fresh_start_handoff_initializes_and_resumes_solo_profile\(\).*?\n}\n",
            source,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(match, "fresh Start Forge journey test is missing")
        journey = match.group(0)
        self.assertIn('"missing_objective"', journey)
        self.assertNotIn('"missing_human_intent"', journey)


if __name__ == "__main__":
    unittest.main()

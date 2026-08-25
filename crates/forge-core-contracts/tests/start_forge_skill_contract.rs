use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SKILL: &str = include_str!("../../../skill/start-forge/SKILL.md");
const GETTING_STARTED: &str = include_str!("../../../docs/getting-started.md");
const AGENT_INTEGRATION: &str = include_str!("../../../docs/agent-integration.md");
const SOLO_SPEC: &str = include_str!("../../../contracts/spec/solo-dogfood-readiness-v0.yaml");
const PRODUCT_CONSTITUTION: &str =
    include_str!("../../../contracts/policies/agent-native-product-constitution.yaml");
const ASSURANCE_ARCHITECTURE: &str =
    include_str!("../../../contracts/spec/agent-native-assurance-architecture.yaml");
const RUNTIME_BUNDLE: &str = include_str!(
    "../../../contracts/workflow-governance/runtime-universal-assurance-candidate-v0.yaml"
);
const GUIDANCE_ENGINE: &str = include_str!("../../../contracts/workflows/guidance-engine.yaml");
const PRODUCT_JOURNEY_SPEC: &str =
    include_str!("../../../contracts/spec/product-journey-guidance-v0.yaml");
const START_E2E: &str = include_str!("../../forge-core-cli/tests/start_cli_e2e.rs");

fn repo_root() -> PathBuf {
    option_env!("CARGO_MANIFEST_DIR").map_or_else(
        || std::env::current_dir().expect("current repository root"),
        |manifest_dir| {
            Path::new(manifest_dir)
                .ancestors()
                .nth(2)
                .expect("repository root")
                .to_path_buf()
        },
    )
}

fn marked_section<'a>(text: &'a str, name: &str) -> &'a str {
    let start = format!("<!-- {name}:start -->");
    let end = format!("<!-- {name}:end -->");
    let (_, after_start) = text
        .split_once(&start)
        .unwrap_or_else(|| panic!("{name} start marker is missing"));
    let (section, _) = after_start
        .split_once(&end)
        .unwrap_or_else(|| panic!("{name} end marker is missing"));
    section
}

fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn assert_contains(text: &str, terms: &[&str]) {
    let text = normalized(text);
    for term in terms {
        assert!(
            text.contains(&term.to_lowercase()),
            "contract is missing: {term}"
        );
    }
}

fn policy_block<'a>(text: &'a str, policy_id: &str) -> &'a str {
    let marker = format!("  - id: {policy_id}");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("policy is missing: {policy_id}"));
    let remainder = &text[start + marker.len()..];
    let end = remainder
        .find("\n  - id: ")
        .map_or(text.len(), |offset| start + marker.len() + offset);
    &text[start..end]
}

fn guided_contract() -> &'static str {
    marked_section(SKILL, "guided-activation-contract")
}

#[test]
fn rust_contract_is_the_single_owner_of_start_forge_guidance_checks() {
    assert!(
        !repo_root()
            .join("scripts/test-start-forge-guided-contract.py")
            .exists(),
        "the Python contract must be removed after its checks move to Rust"
    );
    assert!(PRODUCT_JOURNEY_SPEC.contains("start-forge Rust contract test"));
    assert!(!PRODUCT_JOURNEY_SPEC.contains("start-forge Python contract test"));
}

#[test]
fn windows_default_source_install_precedes_wsl_fallback() {
    let (_, workflow_and_after) = SKILL.split_once("## Workflow").expect("workflow section");
    let (workflow, _) = workflow_and_after
        .split_once("## Safety checks")
        .expect("safety checks section");
    let workflow = normalized(workflow);
    let ordered = [
        "forge-core --version 2>/dev/null",
        "~/.cargo/bin/forge-core --version 2>/dev/null",
        r"%localappdata%\programs\forge-core\bin\forge-core.exe",
        "on windows only, when no native binary is found",
    ];
    let positions = ordered.map(|term| {
        workflow
            .find(term)
            .unwrap_or_else(|| panic!("workflow is missing: {term}"))
    });
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert_contains(
        &workflow,
        &[
            "Require a regular file",
            "a successful `--version` invocation",
            "Do not scan the filesystem",
            "compare arbitrary installed copies",
            "install or build a binary",
            "or edit `PATH` during activation",
            "An explicit non-default `--install-root`",
            "responsibility to expose through PATH",
        ],
    );
}

#[test]
fn orientation_is_evidence_backed_complete_and_actionable() {
    assert_contains(
        guided_contract(),
        &[
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
            "Orientation is a checkpoint, not a stopping point",
            "perform and verify it in the same turn",
            "instead of merely announcing",
            "ask exactly one concise question",
            "If no human input is needed, say so plainly and continue",
        ],
    );
}

#[test]
fn language_and_technical_detail_are_balanced() {
    assert_contains(
        guided_contract(),
        &[
            "language already used by the human",
            "keep all explanatory prose consistently in that language",
            "Do not alternate languages",
            "Technical detail is welcome",
            "must never be the whole explanation",
            "practical meaning",
            "Literal commands, paths, source identifiers, and product names",
        ],
    );
}

#[test]
fn resume_refresh_is_bounded_by_real_state_changes() {
    assert_contains(
        guided_contract(),
        &[
            "reuse the same v9 response",
            "Never run two resume commands consecutively",
            "a successful operation that can change workflow evaluation must intervene",
            "Do not refresh after repository inspection, validation, tests, status, report, or help",
        ],
    );
    assert_contains(
        AGENT_INTEGRATION,
        &[
            "never issues two consecutive resume calls",
            "reuses the complete current response",
            "operation capable of changing the workflow evaluation",
        ],
    );
}

#[test]
fn v9_uses_progressive_product_journey_and_current_work() {
    assert_contains(
        guided_contract(),
        &[
            "workflow_resume_summary_v9",
            "data.journey_guidance",
            "contact_density",
            "Expansion Signal",
            "unclear intent",
            "broad impact",
            "architectural choice",
            "validation failure",
            "catalog.status_argv",
            "catalog.detail_argv.argv",
            "Do not load every detail",
            "do not force research or a large document",
            "quick_cycle",
            "predecessor_detail_argv",
            "data.current_work",
            "absent",
            "stale",
            "read-only advice",
        ],
    );
}

#[test]
fn catalog_orchestration_runs_once_on_material_events() {
    assert_contains(
        marked_section(SKILL, "event-driven-catalog-orchestration"),
        &[
            "once after the first successful resume in a chat",
            "new or replaced Work Focus",
            "Phase changes",
            "human materially changes direction, expresses doubt, or gives corrective feedback",
            "validation exposes an earlier misunderstanding",
            "do not consult again for ordinary messages",
            "choose zero or one plausible practice",
            "execute `data.journey_guidance.catalog.status_argv`",
            "open at most one `data.journey_guidance.catalog.detail_argv.argv`",
            "does not write Forge state",
            "never creates a gate, approval, or mandatory ceremony",
        ],
    );
    assert_contains(
        AGENT_INTEGRATION,
        &[
            "event-driven catalog handoff",
            "once after the first successful resume in a chat",
            "do not repeat the catalog query for ordinary messages",
            "zero or one plausible practice",
        ],
    );
    assert_contains(
        GUIDANCE_ENGINE,
        &[
            "user changes direction, expresses doubt, gives feedback",
            "choose zero or one eligible practice",
            "do not mutate durable state merely because a practice was recommended",
        ],
    );
}

#[test]
fn solo_work_uses_concrete_steps_and_one_clear_priority() {
    assert_contains(
        guided_contract(),
        &[
            "compatible with the active objective",
            "begin that work in the same turn",
            "do not treat an abstract capability label as the task itself",
            "exhaust concrete Solo Cooperative packets and reversible local work",
            "must not be replaced by an unrelated validation command",
            "product-work continuity",
            "a terminal `next_step` is a handoff",
            "focus-bound",
            "workflow-wide",
            "actions.recommended",
            "compatible with the accepted product step",
            "candidate_next_actions",
            "not a competing recommendation",
            "one primary next step",
        ],
    );
}

#[test]
fn evolve_reentry_recovers_prior_context_progressively() {
    assert_contains(
        guided_contract(),
        &[
            "material_supersession",
            "previous_objective_digest",
            "stale Work Focus",
            "current proposal",
            "repository evidence",
            "at most one `workflow report`",
            "Do not run another `workflow resume`",
            "what remains valid",
            "what changed",
            "what is still uncertain",
            "first Discovery step",
        ],
    );
    assert_contains(
        AGENT_INTEGRATION,
        &[
            "material_supersession",
            "progressive context recovery",
            "at most one `workflow report`",
            "stale Work Focus is predecessor context, not active work",
            "do not run another `workflow resume`",
        ],
    );
}

#[test]
fn current_work_and_collaboration_use_existing_safe_interfaces() {
    assert_contains(
        guided_contract(),
        &[
            "workflow current-work prepare",
            "on-demand helper, not a pre-flight step",
            "temporary input outside the project snapshot",
            "Preparation is read-only",
            "current-work accept",
            "current-work update",
            "focus.collaboration",
            "next_ready_lane",
            "A dependent lane becomes ready only after every predecessor has a completed promotion receipt.",
            "parallelize only independent lanes",
            "../.forge-worktrees/<agent>/<task>",
            "isolation propose",
            "before active work begins",
            "Never run `git worktree add` first",
        ],
    );
}

#[test]
fn all_primary_activation_journeys_have_closed_behavior() {
    let matrix = marked_section(guided_contract(), "guided-activation-journeys");
    let mut rows = BTreeMap::<String, Vec<String>>::new();
    for line in matrix.lines() {
        let cells = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.len() != 4 || !cells[0].starts_with('`') {
            continue;
        }
        let journey = cells[0].trim_matches('`').to_owned();
        let previous = rows.insert(
            journey.clone(),
            cells[1..].iter().map(|cell| (*cell).to_owned()).collect(),
        );
        assert!(previous.is_none(), "duplicate journey row: {journey}");
    }

    let expected = BTreeSet::from([
        "autonomous_action_available".to_owned(),
        "brownfield_managed".to_owned(),
        "brownfield_unmanaged".to_owned(),
        "greenfield".to_owned(),
        "human_decision_required".to_owned(),
        "runtime_or_bridge_unavailable".to_owned(),
        "state_loss_or_integrity_failure".to_owned(),
    ]);
    assert_eq!(rows.keys().cloned().collect::<BTreeSet<_>>(), expected);
    for (journey, cells) in &rows {
        assert!(
            cells.iter().all(|cell| !cell.is_empty()),
            "incomplete journey row: {journey}"
        );
    }
    assert!(rows["greenfield"][2].contains("Ask one concise outcome question"));
    assert!(rows["brownfield_managed"][2].contains("highest-ranked feasible safe action"));
    assert!(rows["state_loss_or_integrity_failure"][1].contains("nothing will be recreated"));
    assert!(rows["runtime_or_bridge_unavailable"][2].contains("Do not initialize or switch roots"));
    assert!(rows["human_decision_required"][2].contains("Ask exactly one concise question"));
    assert!(rows["autonomous_action_available"][2].contains("Execute in the same turn"));
}

#[test]
fn human_and_integrator_guides_preserve_the_same_experience() {
    for document in [GETTING_STARTED, AGENT_INTEGRATION] {
        assert_contains(
            document,
            &[
                "language already used",
                "technical",
                "practical meaning",
                "same turn",
            ],
        );
    }
    assert_contains(GETTING_STARTED, &["exactly one concise question"]);
}

#[test]
fn consequential_uncertainty_drives_autonomous_research() {
    assert_contains(
        marked_section(SKILL, "uncertainty-driven-research"),
        &[
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
        ],
    );
    assert_contains(
        AGENT_INTEGRATION,
        &[
            "does not wait for the human to request research",
            "compares competing hypotheses, contrary evidence",
            "continues with the next safe action",
            "research source add",
            "registration proves provenance and resolvability, not truth",
            "do not register every search result or trivial fact",
        ],
    );
    assert_contains(
        GETTING_STARTED,
        &[
            "does not need to tell the agent to research",
            "compares competing explanations and contrary evidence",
            "keeps working",
            "do not register every search result or small fact",
            "not that the information is automatically true",
        ],
    );
    assert_contains(
        PRODUCT_CONSTITUTION,
        &[
            "research and competence acquisition",
            "consequential uncertainty must be researched",
        ],
    );
    assert_contains(
        ASSURANCE_ARCHITECTURE,
        &["research multiple credible and independent sources"],
    );
    assert_contains(
        policy_block(RUNTIME_BUNDLE, "policy.workflow.investigation"),
        &[
            "competing hypotheses",
            "contrary evidence",
            "remaining uncertainty",
        ],
    );
}

#[test]
fn product_readiness_spec_requires_primary_guided_journeys() {
    assert_contains(
        SOLO_SPEC,
        &[
            "greenfield orientation",
            "existing unmanaged project",
            "existing managed project",
            "state-loss or integrity failure",
            "runtime or host-bridge failure",
            "human-facing prose stays in the human's language",
            "material ambiguity produces one concise decision request",
            "consequential uncertainty triggers autonomous research without waiting for a human instruction",
            "agent-autonomous reversible action proceeds without human confirmation",
        ],
    );
}

#[test]
fn fresh_start_e2e_uses_current_solo_objective_status() {
    let start = START_E2E
        .find("fn fresh_start_handoff_initializes_and_resumes_solo_profile()")
        .expect("fresh Start Forge journey test is missing");
    let remainder = &START_E2E[start..];
    let journey = remainder
        .split("\n#[test]")
        .next()
        .expect("fresh Start Forge journey body");
    assert!(journey.contains("\"missing_objective\""));
    assert!(!journey.contains("\"missing_human_intent\""));
}

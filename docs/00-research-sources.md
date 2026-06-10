# Research Sources

This prototype was shaped by the following systems and techniques.

## Codex-Native Runtime

- OpenAI Codex Skills: https://developers.openai.com/codex/skills
  - Takeaway: package workflows as skills with progressive disclosure, references, scripts, assets, and optional agents config.
- OpenAI Codex Customization: https://developers.openai.com/codex/concepts/customization
  - Takeaway: combine AGENTS.md, memories, skills, MCP, and subagents; keep persistent repo guidance small.

## BMAD Product Family

- BMAD docs: https://docs.bmad-method.org/
  - Takeaway: BMAD is strong because it combines planning, agent roles, workflows, and implementation loops.
- BMAD workflow map: https://docs.bmad-method.org/reference/workflow-map/
  - Takeaway: progressive context across phases is valuable, but should be represented as compact state.
- BMAD GitHub repo: https://github.com/bmad-code-org/bmad-method
  - Takeaway: Builder, Creative Intelligence Suite, Game Dev Studio, and Test Architect should become runtime modules, not one giant core.

## Spec-Driven Development

- GitHub Spec Kit: https://github.github.com/spec-kit/
  - Takeaway: Spec -> Plan -> Tasks -> Implement is a good baseline spine.
- Spec-driven development concepts: https://github.github.com/spec-kit/concepts/sdd.html
  - Takeaway: specs should stay central and executable, not become disposable planning docs.
- Kiro Specs: https://kiro.dev/docs/specs/
  - Takeaway: requirements, design, tasks, and task accountability map cleanly to method phases.

## Coding Agent Lessons

- Aider repo map: https://aider.chat/docs/repomap.html
  - Takeaway: context should be selected with a repo map, not dumped into the model.
- Aider lint/test loop: https://aider.chat/docs/usage/lint-test.html
  - Takeaway: tests and linters should feed automatic repair loops.
- SWE-agent paper: https://arxiv.org/abs/2405.15793
  - Takeaway: agent-computer interface design matters; workflows need tools and state, not just prompts.

## Validation Loops

- OpenAI Codex repair loops: https://developers.openai.com/cookbook/examples/codex/build_iterative_repair_loops_with_codex
  - Takeaway: Review -> Repair -> Validate should be a first-class workflow.
- OpenAI agent improvement loop: https://developers.openai.com/cookbook/examples/agents_sdk/agent_improvement_loop
  - Takeaway: traces, evals, feedback, and handoffs can make the runtime improve itself over time.


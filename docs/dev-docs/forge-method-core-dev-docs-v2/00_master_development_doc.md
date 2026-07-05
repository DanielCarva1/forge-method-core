# Forge Method Core v2 - audited development document

Date: 2026-06-28  
Scientific window for papers: 2025-10-28 to 2026-06-28

## 1. Objective

This document converts recent scientific review, market/community signals, official protocol sources, and the reading of the current codebase into a development plan for the Forge Method Core.

The main decision is simple: Forge should become a deterministic coordination layer for work with agents. The product must expose agent power without giving up preview, verification, trace, rollback, governed memory, protocol security, and conflict control.

## 2. Audit rules

Scientific papers enter as the primary basis only if they fall within the eight-month window: 2025-10-28 to 2026-06-28. Current official sources enter as ecosystem state, not as empirical proof. Rust docs enter as stable engineering. Community signals and cases enter as demand, not as proof of efficacy.

Strength levels used in the ledger:

- A: primary evidence for a product or architecture decision.
- B: complementary evidence or a current official source.
- C: useful context, but not sufficient for a central decision.

## 3. Current reading of the field

### 3.1 Homogeneous multi-agent should not be the default

OneFlow, BenchAgent, and the uncertainty study in MAS converge on one conclusion: several agents with the same base model, changing only the prompt, role, or position, do not automatically beat a well-controlled single-agent. Forge must treat single-agent as a mandatory anchor and require evidence to activate MAS. This becomes a product feature in `forge eval compare`.

Decision: every multi-agent feature must state which advantage it is seeking: model heterogeneity, different tools, different permissions, different principals, multimodality, context isolation, or real parallelism.

### 3.2 Graph orchestration has become the most defensible form

The workflow optimization survey separates template, realized graph, and execution trace. GraphBit and VMAO reinforce explicit DAG, deterministic execution, verifier, and replanning. This fits Forge directly because the current `OperationContract` already models authority, execution, gates, and effects, but needs to become a node within a larger graph.

Decision: create `WorkflowGraph` as a first-class entity and keep `OperationContract` as a node type or operation contract. Prompt does not decide structural routing on its own.

### 3.3 Trace and eval need to be a product

AgentBeats shows that agent evaluation is moving toward standardized interfaces, judge agents, and reproducibility via protocols. Forge already has evidence logs and typed reasons, but needs to formalize a canonical `TraceEvent` and an eval harness.

Decision: no run without trace. No new architecture without comparison against a baseline.

### 3.4 Memory needs policy, not just retrieval

MemFlow, MemRouter, and Experience Compression Spectrum show that good memory depends on write policy, read routing, compression level, budget, and support validation. Memory must not create authority automatically.

Decision: create `MemoryPolicy`, `MemoryRecord`, `MemoryPromotion`, and `MemoryAuthorityBoundary` before any rich persistent memory.

### 3.5 Protocol needs identity and capability binding

MCP and A2A are important for the ecosystem. But the security threat modeling, MCP security, AIP, and AgentRFC papers show risks of missing attestation, trust propagation, unsafe composition, delegation without identity, and conformance gaps.

Decision: Forge's MCP/A2A adapters are projections of the kernel, not sources of truth. Every mutable call must be bound to PrincipalId, capability, scope, OperationContract, and trace.

### 3.6 Community demand is in control, integration, and governance

The Stack Overflow paper on agents shows pain points in runtime integration, dependency management, orchestration complexity, and evaluation reliability. Current agentic platform documentation shows demand for custom agents, hooks, skills, sandboxes, memory, budgets, MCP, rollback, and audit logs. Papers on common users show that security and privacy transparency needs to be actionable and on-demand.

Decision: Forge must sell `build safely with agents`, not `more agents talking`.

## 4. Evidence from the current codebase

| ID | Location | Finding |
|---|---|---|
| C01 | `README.md:52-70` | Forge v2 already defines .forge-method as a coordination layer for humans and agents, with registry, lane claims, append-only handoffs, and optimistic concurrency. |
| C02 | `Cargo.toml:3-12` | Rust workspace split into forge-contract-validator, forge-core-contracts, forge-core-schema, forge-core-cli, forge-core-store, forge-core-validate, and forge-core-kernel. |
| C03 | `crates/forge-core-contracts/src/operation.rs:13-44` | OperationContract models autonomy, authority, coordination, execution, stop policy, commands, effects, gates, and diagnostics. |
| C04 | `crates/forge-core-store/src/lib.rs:21-34,127-177,299-324` | Store concentrates indexing, YAML collection, JSONL append, transactional effect, WAL, lock, and metadata. |
| C05 | `crates/forge-core-cli/src/main.rs:47-93` | Current CLI uses manual parsing via env::args, match command, index loops, and process::exit. |
| C06 | `crates/forge-core-kernel/src/lib.rs:43-98,244-365` | RuntimePlan already computes status and typed reasons with validation, gates, human input, review, and mutation policy. |

## 5. Priority feature map

| ID | Priority | Feature | Users | Evidence |
|---|---|---|---|---|
| F01 | P0 | forge preview | all | P21,P22,P28,O03,C06 |
| F02 | P0 | forge ready | common user, QA, dev, company | P22,P23,P30,C06 |
| F03 | P0 | Canonical TraceEvent and forge explain | all | P04,P07,P17,P24,P26,C06 |
| F04 | P0 | WorkflowGraph v0 | power user, company | P04,P05,P06,C03,C06 |
| F05 | P1 | Eval Compare single-agent baseline | power user, research, company | P01,P02,P03,P07 |
| F06 | P1 | Memory Policy | all | P09,P10,P11,P28,O03 |
| F07 | P1 | Multi-principal governance | teams, companies, open source | P08,P24,P25,P26,C01 |
| F08 | P1 | Secure MCP adapter | power user, companies | O01,P17,P18,P19,P20,O03 |
| F09 | P2 | Secure A2A adapter | power user, companies | O02,P08,P17,P19,P20 |
| F10 | P2 | Local Control Plane | power user, QA, teams | P06,P07,P21,P28,O03,C01 |
| F11 | P1 | Risk Audit Gate for AI code | QA, dev, companies | P22,P23,P30 |
| F12 | P2 | Guided Start and Product UX | common user, founder, beginner dev | P28,P29,O03 |
| F13 | P2 | Budget and Cost Accounting | power user, companies | P01,P02,P16,O03 |
| F14 | P3 | Knowledge Orchestration mode | research, product, analysts | P13,P14,P15 |
| F15 | P0 | Rust ergonomics and codegen track | maintainers and code agents | O04,O05,O06,O07,P31,C04,C05 |

## 6. Expected final product

### For the common user

Forge should feel like a safe mode to build with AI. The person does not need to know what MCP, A2A, WAL, or WorkflowGraph is. The experience needs to be:

1. Get started guided.
2. Understand the plan.
3. See a preview before mutation.
4. Run verification.
5. Receive a short explanation.
6. Undo when something goes wrong.

Translated features: `forge start --guided`, `forge preview`, `forge ready`, `forge explain`, `forge undo`.

### For the vibe coder, indie maker, and founder

The biggest risk is publishing something that appears to work, but has bad auth, exposed data, a fake test, an unsafe dependency, or a silent failure. Forge must be the seatbelt.

Translated features: security checklist, secrets gate, deploy gate, data risk gate, risk audit gate, ready gate, and rollback.

### For the professional dev/QA

The pain is not generating code. The pain is reviewing, validating, tracking, testing, and explaining what the agent did. Forge must become a control layer for code agents.

Translated features: trace, eval, readiness report, AI risk audit, policy-as-code, CI integration, failure taxonomy, and evidence ledger.

### For the AI power user

This user wants to control graph, nodes, tools, budgets, memory, protocols, replay, and evals. They accept declarativeness and files.

Translated features: `forge graph`, `forge eval`, `forge memory`, `forge protocol mcp`, `forge protocol a2a`, local control plane.

### For teams/companies

The value is governance of AI-generated work. The team needs to know who did it, with which permission, which evidence, which risk, and which rollback.

Translated features: PrincipalId, IntentContract, ConflictContract, GovernancePolicy, audit ledger, allowed capabilities, budgets, and approval gates.

## 7. Architecture decisions

1. Rust stays in the deterministic kernel.
2. Live semantics stay declarative until stabilized.
3. `OperationContract` becomes a node or payload within `WorkflowGraph`.
4. Every run generates a `TraceEvent`.
5. Every multi-agent feature needs a single-agent baseline.
6. MCP and A2A enter as secure adapters, not as authority.
7. Memory needs policy and source evidence.
8. Multi-principal governance becomes Forge's differentiator.
9. CLI uses manual argv in `main.rs` (no `clap`, no derive macros). Each new subcommand adds an arm to the `match` in `main.rs` and a `run_<command>(&[String])` fn. See `04_rust_refactor_guide.md`.
10. Errors and diagnostics must be hand-written typed enums (no `thiserror`, no `anyhow`), deriving `Debug, Clone, PartialEq, Eq`.

## 8. Summary source ledger

| ID | Date | Strength | Title | Area |
|---|---|---|---|---|
| P01 | 2026-01-18 | A | Rethinking the Value of Multi-Agent Workflow: A Strong Single Agent Baseline | multi-agent baseline |
| P02 | 2026-06-04 | A | Do More Agents Help? Controlled and Protocol-Aligned Evaluation of LLM Agent Workflows | multi-agent evaluation |
| P03 | 2026-02-04 | A | On the Uncertainty of Large Language Model-Based Multi-Agent Systems | MAS uncertainty |
| P04 | 2026-03-23 | A | From Static Templates to Dynamic Runtime Graphs: A Survey of Workflow Optimization for LLM Agents | workflow graphs |
| P05 | 2026-03-08 | A | GraphBit: A Graph-based Agentic Framework for Non-Linear Agent Orchestration | deterministic graph orchestration |
| P06 | 2026-03-12 | A | Verified Multi-Agent Orchestration: A Plan-Execute-Verify-Replan Framework for Complex Query Resolution | verify replan |
| P07 | 2026-06-11 | A | AgentBeats: Agentifying Agent Assessment for Openness, Standardization, and Reproducibility | agent evaluation |
| P08 | 2026-04-10 | A | MPAC: A Multi-Principal Agent Coordination Protocol for Interoperable Multi-Agent Collaboration | multi-principal governance |
| P09 | 2026-05-05 | A | MemFlow: Intent-Driven Memory Orchestration for Small Language Model Agents | memory orchestration |
| P10 | 2026-05-01 | A | MemRouter: Memory-as-Embedding Routing for Long-Term Conversational Agents | memory admission |
| P11 | 2026-04-17 | A | Experience Compression Spectrum: Unifying Memory, Skills, and Rules in LLM Agents | experience compression |
| P12 | 2026-02-02 | A | Kimi K2.5: Visual Agentic Intelligence | oriental swarm and multimodal agents |
| P13 | 2026-06-01 | A | K-BrowseComp: A Web Browsing Agent Benchmark Grounded in Korean Contexts | localized agent benchmarks |
| P14 | 2026-06-11 | A | EvoBrowseComp: Benchmarking Search Agents on Evolving Knowledge | fresh benchmarks |
| P15 | 2026-06-11 | A | Agents-K1: Towards Agent-native Knowledge Orchestration | knowledge orchestration |
| P16 | 2026-05-07 | B | Efficient Serving for Dynamic Agent Workflows with Prediction-based KV-Cache Management | serving and cost |
| P17 | 2026-02-11 | A | Security Threat Modeling for Emerging AI-Agent Protocols: A Comparative Analysis of MCP, A2A, Agora, and ANP | protocol security |
| P18 | 2026-01-24 | A | Breaking the Protocol: Security Analysis of the Model Context Protocol Specification and Prompt Injection Vulnerabilities in Tool-Integrated LLM Agents | MCP security |
| P19 | 2026-03-25 | A | AIP: Agent Identity Protocol for Verifiable Delegation Across MCP and A2A | identity and delegation |
| P20 | 2026-03-25 | B | AgentRFC: Security Design Principles and Conformance Testing for Agent Protocols | protocol conformance |
| P21 | 2025-10-29 | A | What Challenges Do Developers Face in AI Agent Systems? An Empirical Study on Stack Overflow | developer demand |
| P22 | 2026-04-19 | A | AIRA: AI-Induced Risk Audit: A Structured Inspection Framework for AI-Generated Code | AI generated code risk |
| P23 | 2026-06-07 | B | Governance Controls for AI-Generated Test Artifacts in Autonomous Software Testing | QA and governance |
| P24 | 2026-01-26 | A | Agentic Much? Adoption of Coding Agents on GitHub | coding agent adoption |
| P25 | 2026-02-09 | A | AIDev: Studying AI Coding Agents on GitHub | coding agent dataset |
| P26 | 2026-01-24 | B | Fingerprinting AI Coding Agents on GitHub | authorship and governance |
| P27 | 2026-02-16 | A | Configuring Agentic AI Coding Tools: An Exploratory Study | agent configuration demand |
| P28 | 2026-04-19 | A | What Security and Privacy Transparency Users Need from Consumer-Facing Generative AI | consumer trust |
| P29 | 2026-01-26 | B | Generative AI in Saudi Arabia: A National Survey of Adoption, Risks, and Public Perceptions | non-western consumer adoption |
| P30 | 2026-04-13 | B | Taking a Pulse on How Generative AI is Reshaping the Software Engineering Research Landscape | software engineering governance |
| P31 | 2026-05-22 | B | MISRust: Mapping MISRA-C++ Coding Guidelines to the Rust Programming Language | Rust safety guidance |
| O01 | current-2026 | B | Model Context Protocol docs: What is MCP? | official protocol |
| O02 | current-2026 | B | A2A Protocol docs | official protocol |
| O03 | current-2026 | B | GitHub Copilot cloud agent docs | market signal |
| O04 | current-2026 | B | clap derive docs | Rust CLI |
| O05 | current-2026 | B | tracing crate docs | Rust observability |
| O06 | current-2026 | B | thiserror crate docs | Rust errors |
| O07 | current-2026 | B | Rust API Guidelines | Rust API |

The complete ledger is in `data/evidence_ledger.csv`.

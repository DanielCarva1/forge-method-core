# Repository agent instructions

## Agent skills

### Issue tracker

Work items live in GitHub Issues for `DanielCarva1/forge-method-core`. The user
discusses and approves the work; the agent writes and publishes the issue only
after that approval. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the repository's existing GitHub labels. A fully specified, approved issue
that an agent can implement receives `ready-for-agent`. See
`docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. Read the root `CONTEXT.md` and relevant
ADRs before naming or changing domain concepts. See `docs/agents/domain.md`.

## GitHub publication and authorship

- Do not publish an issue, pull request, release, or other planning artifact
  until the user has accepted its content or explicitly requested publication.
- The agent prepares issue text from the accepted product discussion; do not
  require the user to write the ticket.
- Never add Codex, OpenAI, another AI system, or an AI-generated identity as an
  author or co-author in commits, pull requests, issues, releases, or repository
  metadata.
- Do not add `Co-authored-by` or similar attribution trailers for an AI system.
- Preserve the human maintainer as the project author. Do not rewrite existing
  authorship or contribution history.

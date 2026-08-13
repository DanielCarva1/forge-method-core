# Issue tracker: GitHub

Issues and product specifications for this repository live in GitHub Issues at
`DanielCarva1/forge-method-core`. They do not become files in the source tree.
Use the `gh` CLI from the repository clone.

## Ownership and approval

- The user discusses product intent, tradeoffs, and acceptance with the agent.
- The agent drafts the complete issue from that accepted discussion.
- Before publication, show the proposed ticket breakdown or ticket content and
  receive the user's acceptance.
- Do not require the user to write, format, label, or publish the issue.
- After acceptance, the agent may create the issue and apply the agreed labels.
- Creating an issue is publication, not a private draft.

## Commands

- Create: `gh issue create --repo DanielCarva1/forge-method-core --title "..." --body-file <file>`
- Read: `gh issue view <number> --repo DanielCarva1/forge-method-core --comments`
- List: `gh issue list --repo DanielCarva1/forge-method-core --state open`
- Edit labels: `gh issue edit <number> --repo DanielCarva1/forge-method-core --add-label "..."`
- Close: `gh issue close <number> --repo DanielCarva1/forge-method-core --comment "..."`

Use a temporary file outside the project snapshot for a multiline issue body,
then remove it after successful publication. Never place secrets in an issue.

## Dependencies

Publish tracer-bullet issues in dependency order. Record genuine blockers in
the `Blocked by` section using real issue numbers. Do not invent a dependency
merely to force serial work.

## Authorship

Do not add an AI system as author or co-author in issue text, comments, commits,
pull requests, releases, or metadata.

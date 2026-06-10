# Project Agent Rules

This project uses Forge Method.

Runtime state is stored in:

```txt
.forge-method/state.yaml
.forge-method/sprint.yaml
.forge-method/evidence/
```

Rules:

- Read runtime state before choosing a workflow.
- Do not infer phase from chat history.
- Keep implementation scoped to the active story.
- Run required checks before marking work done.
- Write evidence before updating a story to done.
- Ask for human input only when the workflow requires it.


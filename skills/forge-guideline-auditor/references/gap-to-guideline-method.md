# Gap To Guideline Method

Use this when a product or method gap must become a durable guideline.

## Input

- source gap or matrix row
- current project docs
- existing Forge state
- affected layer
- acceptance evidence expected by the human

## Algorithm

1. Name the gap in one sentence.
2. Name the risk if an agent implements now.
3. Classify layer:
   - human experience
   - agent substrate
   - machine contract
   - product governance
   - release governance
4. Decide whether a guideline is needed:
   - repeated decision: yes
   - cross-file/cross-agent behavior: yes
   - safety/release/permission behavior: yes
   - one-off implementation detail: no
5. Name the guideline.
6. Define acceptance evidence before any work order.
7. List the first work-order candidate.

## Output Shape

```md
## Gap

## Risk

## Needed Guideline

## Layer

## Acceptance Evidence

## Work Order Candidate

## Implementation Block
```

## Block Rule

If acceptance evidence cannot be stated in observable terms, implementation remains blocked.

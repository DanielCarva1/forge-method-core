# Post-release guidance audit

- kind: validation
- created_at: 2026-06-12T03:51:48+00:00
- story: post-release-guidance-audit-p1
- checks: workflow validate: passed; audit: passed; gate --require-evals: passed; stale phrase scan: no matches; smoke-plugin-clone-install.ps1 -Ref v1.27.0 -ExpectedVersion 1.27.0: passed

## Summary

Audited the 1.27.0 guidance batch for misleading docs and stale agent context. Fixed published-ref smoke examples to use v1.27.0 instead of main, marked the old BMAD facilitation-depth verdict as superseded, confirmed official workflow/audit/gate validators pass, and verified clone/install smoke from the v1.27.0 tag.

# v1.31.1 public install hotfix validated

- created_at: 2026-06-17T20:50:38+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>

## Summary

Implemented and validated public install routing guard: installed Forge packages no longer expose core project state to normal users; core state requires maintainer marker/env plus explicit allow-runtime-state.

## Decisions

- Ship as patch release 1.31.1 because this affects public install/start behavior.

## Checks

- unit full suite passed
- runtime/package smokes passed
- installed package simulation no longer leaks continue_current_project

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

Commit v1.31.1 hotfix, create GitHub tag/release, and validate clone/install by v1.31.1 and main.

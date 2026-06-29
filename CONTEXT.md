# Forge Method Context Glossary

## Consumer Project Repo

The application, game, library, or product repository being developed with Forge Method. It owns product source code and may carry a small Forge Project Link, but it does not own Forge runtime state.

## Forge Runtime Sidecar

A sibling directory or repository that owns the Forge Method runtime state for one Consumer Project Repo. It contains the real `.forge-method/` tree, including state, artifacts, evidence, ledger, stories, and claims.

## Forge Project Link

The small `.forge-method.yaml` file stored at a Consumer Project Repo root. It points to the Forge Runtime Sidecar and its `.forge-method/` state root.

## Bootstrap Core Exception

The temporary exception that allows `D:\Forge-method-core` to keep local `.forge-method/` state while the Forge core is still being developed by Forge itself. This exception is explicit and must not be copied to consumer projects.

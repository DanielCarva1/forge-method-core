# Validate published version 1.30.0

- kind: validation
- created_at: 2026-06-17T17:59:07+00:00
- checks: web raw main VERSION=1.30.0; web release latest=v1.30.0; gh release view v1.30.0; git fetch origin main:main --tags; smoke-plugin-clone-install.ps1 -Ref main -ExpectedVersion 1.30.0; smoke-plugin-clone-install.ps1 -Ref v1.30.0 -ExpectedVersion 1.30.0

## Summary

Confirmed GitHub raw main VERSION and plugin manifest are 1.30.0; GitHub latest release redirects to v1.30.0; gh release v1.30.0 is published and non-prerelease; origin/main and local main now point to acd892c with VERSION 1.30.0; clean clone/install smoke passed for both main and v1.30.0 with ExpectedVersion 1.30.0.

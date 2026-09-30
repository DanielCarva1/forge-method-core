# Forge Desktop 0.1.80 alpha

## Connect without a Forge account

- A visible **Conectar ao Codex** button uses the existing Codex account when
  available; it no longer sits inside the history selector.
- When an explicit connection or Send attempt needs authentication, Forge
  starts the official Codex device-login flow directly, without an extra
  preliminary sign-in click or a separate Forge registration.
- Completing login continues the requested conversation connection. It never
  sends the preserved draft automatically. Canceling leaves the draft intact.
- Changing projects discards the old pending connection continuation.

## Verification and limits

Focused isolated native tests covered simulated first-login completion,
cancellation and an existing account, plus connection with the real existing
account on the development host, without sending a model turn. Provider-side
first-login completion by a new human on a clean device remains NOT_RUN.

Core stays 0.13.4. Mobile access is deferred at the maintainer's request. No
server, copied account credentials, automatic login to an unverified account,
or mandatory cloud service was added. Existing unsigned/manual alpha update
and result-preview limits remain. This closes the connection interaction,
not the overall visual-polish work.

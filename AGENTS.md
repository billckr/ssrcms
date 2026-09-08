# SynapCMS Agent Guide

Use this file as the starting point for work in this repository. Consult the
more detailed documents under `docs/` when a task touches those areas.

## Local Application Workflow

- Manage the development application with `./app.sh`.
- Use `./app.sh status`, `./app.sh start`, `./app.sh stop`, and
  `./app.sh restart` for normal process management.
- Use `./app.sh logs` when startup or request handling needs investigation.
- `./app.sh restart` does not compile Rust. Use `./app.sh rebuild` when Rust
  source changes must be included in the running application.
- Theme template and CSS changes do not require a Rust rebuild, but manually
  edited theme templates may require `./app.sh restart` because Tera caches
  templates in memory.
- The local application listens on port 3000 and is normally reached through
  Caddy at `https://localhost`.

## Theme Resolution

- Bundled themes live under `themes/global/<theme>/`.
- A site may have an overriding copy under
  `sites/<site-id>/themes/<theme>/`. The site-specific copy takes precedence
  for that site.
- Before changing a live theme, determine the path actually used by the
  request. Application logs report messages such as `loaded theme ... from
  <path>` and are the most reliable check.
- If a feature is intended for both the bundled theme and a site using an
  override, apply the change to both copies while preserving unrelated,
  intentional site customizations. Do not blindly replace an entire
  site-specific theme with the global version.
- After changing a theme, verify both the rendered page and its served
  stylesheet. Browser caching may require a hard refresh.

## Verification

- Run tests appropriate to the changed area. For core Rust changes, a useful
  baseline is `cargo test -p synaptic-core --lib`.
- Run `git diff --check` after editing files.
- When changing live behavior, restart or rebuild as appropriate and verify
  the actual localhost route rather than relying only on source inspection.
- A logged-out request cannot verify controls that render only for authenticated
  users; call out that limitation or perform a logged-in check when possible.

## Repository Safety

- Preserve existing uncommitted changes and keep unrelated edits out of the
  current task.
- Do not modify database content, site settings, or documentation stored in the
  database unless the user explicitly requests it.
- Prefer focused edits over broad synchronization, especially under `sites/`,
  where files may contain deliberate per-site customizations.

## Project Documentation

- Development commands and architecture: `docs/development-guide.md`
- Theme behavior and authoring: `docs/theme-authoring-guide.md`
- Deployment operations: `docs/deployment-guide.md`
- Authentication review and implementation notes:
  `AUTH_SECURITY_REVIEW.md` and `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`

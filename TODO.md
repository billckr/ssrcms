# TODO

Running backlog of outstanding tasks. Not part of Claude's memory system — memory only holds a
pointer here, plus the "why" behind deferred/architectural items (see `MEMORY.md`'s "Deferred
implementation notes" for those). Add items here as they come up; check them off (or delete) once
done.

## Open

- [ ] create option when using install-vps.sh to also populate the documention table with the current docs. May need to creation new migration as part of the process.
- [ ] allow site admins to either upload via ui or web their owns logo for their account simlar to how the super admin has for the main app.
- [ ] work on synap install flow.
- [ ] review dark mode and make small tweaks to button, icon and text colors. Documetnation bold also needs changing.
- [ ] adjust badged to better fit others size on /admin/sites
- [ ] add a dark mode maintenaince page. Add light/dark move option to https://synapcms.dev/admin/sites/8e8b22bc-ecd8-4f39-bcf5-f29876d7312b/settings in Maintenance Mode section. So either the dark mode or light mode maintance page can be selected.
- [ ] revisit the need or funtion login behind the http://pong.com/admin/menus menu location. The theme has to be aware of this and it may need adjustment based on new menu options.
- [ ] consiider adding clerk-rs as account creation option. It's not offical but appears to be well maintained.
- [ ] add Google login via OAuth2 (oauth2 crate) as an account creation/sign-in option.
- [ ] need to think about how we're going to relay via docs about all the possible data points that can be displayed for a site for users who want to build or customize their own themes. example Posts, pages, etc etc.
- [ ] consider adding an owner column on the sites page to clearly show the top level domain this site belongs to
- [ ] consider a byline for the SynapCMS name. Something like "words have life" or something deep and thoughtful
- [ ] consider adding a link back to the main site on the subscribe form.
- [ ] media manager/picker WASM island only stays "warm" within a single admin page — navigating to a new page and reopening pays the full bootstrap + fetch delay again. Real-world gain from the keep-warm fix is smaller than a quick same-page test suggests. Revisit with sessionStorage-cached initial data (cheap, partial fix) or as part of a future whole-admin WASM SPA conversion (real fix, bigger project) if this keeps being noticeable.
- [ ] Media manager dark-mode color scheme (sidebar/toolbar/content/footer backgrounds) still isn't right after several passes — revisit from scratch with a clearer reference/screenshot before making more changes.
- [ ] in theme editor, when you change a color, trying to revert back to original doesn't work.

- [ ] the "Scheduled" date/time picker popup on the post editor (native `<input type="datetime-local">` calendar) can render off the right edge of the browser window when the field sits near the right side of the page — needs to stay clamped inside the viewport.

- [ ] revisit whether anyone who can edit a post (not just users with can_manage_forms) should be able to see the embedded form's submission count in the post editor sidebar and the "view form metrics" link — currently no permission gate on that, only the destination /admin/form-analytics page itself enforces can_manage_forms.

- [ ] decide how (or whether) to surface the install-wide (.env MAILGUN_*) email fallback account on a site's Email Settings tab — right now it's active but invisible in the UI when a form has no provider selected.

- [ ] form_submissions.data has no GIN or expression index, so filtering by a specific field's value (not listing/paginating, but "find every submission where email = x") would currently mean scanning within whatever the site+form index already narrowed down to. That's fine today because nothing does that yet — and if it's ever needed, it's a single CREATE INDEX on the specific field, not a redesign.

- [ ] revisit whether Media Manager's alt-text field should be required (or at least nudge/warn) on upload — currently optional, editors can leave it blank. Don't force it outright without more thought (WP doesn't require it either); consider a softer nudge instead. Surfaced while auditing PageSpeed accessibility findings (2026-08-21).

- [ ] follow-ups deferred from the 2026-09-07 auth security hardening (`AUTH_SECURITY_REVIEW.md` / `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`), none required before the fixes already shipped. Most of this list has since shipped (see Done below: verified email change for subscribers, cross-site join, common-password denylist, escalating login delay, sign-out-other-devices, `routes.rs` integration suite) — what's actually still open:
  - verified self-service email change exists for subscribers (`f22e9fb`) but was intentionally left read-only for staff/admin (`/admin/profile`) — `send_for_site` needs a concrete `site_id` and a super_admin's `AdminUser.site_id` can be `None`; would need a design decision for that case before extending it to staff
  - move the in-process login/registration/recovery rate limiter to PostgreSQL or a shared cache before running more than one app instance
  - WebAuthn/passkeys
  - recent-authentication ("step-up auth") requirement for especially sensitive actions — nothing beyond the existing password/email-change confirmation
  - broader security-notification emails (new-device login, session revoked, etc.) — currently only the email-change old-address notice exists
  - CAPTCHA — deliberately deferred until real abuse patterns warrant it
  - decide whether production should default Axum's bind host to `127.0.0.1` when Caddy runs on the same box (currently defaults to `0.0.0.0`, relies on firewall/deployment docs) — see "Deferred follow-up: Axum bind policy" in `AUTH_EXPLOIT_REVIEW_RESULTS.md`

## Done

- [x] TOTP authenticator-app MFA for staff logins (`/admin/login`), plus hashed single-use recovery codes. Shipped 2026-09-11 — see `TOTP_MFA_IMPLEMENTATION_RESULTS.md`. Self-service opt-in via `/admin/profile` (QR + manual-secret enrollment, disable, recovery-code regeneration, all gated behind re-confirming the current password); a super_admin can force-disable another staff member's MFA (`/admin/users/{id}/disable-mfa`) to recover a lockout. Notification emails on enable/disable/regenerate. Staff only — subscribers, org-wide enforcement, and WebAuthn/passkeys remain future follow-ups.

- [x] bug: any `.data-table`'s Actions column (Tags, Categories, Menus, Users, site AI Providers, etc.) could visually detach from its row and cut across mid-row once that row grew taller than the action icons (e.g. wrapped multi-line text in another column) — first spotted on the AI Translation providers table, then confirmed sitewide. Root cause: `.data-table .actions { display: flex; ... }` overrode the `<td>`'s own display away from `table-cell`, so it stopped stretching to the row's full height like every other cell. Fixed 2026-09-11: dropped the `display:flex`/`gap` (every `.actions` cell only ever has one child — the `.icon-pill-actionbuttons` div, which already does its own flex layout) in favor of `vertical-align: middle`, which centers correctly on a real table cell. Confirmed via computed-style + bounding-rect checks that the cell now reports `display: table-cell` and fills the row.

- [x] bug: `cli/src/commands/install.rs::generate_password()` generated a 10-character password (not re-checked against `validate_password()`) after the 2026-09-07 auth security pass raised the minimum to 12 chars — non-interactive installs without `ADMIN_PASSWORD` could mint a super_admin below the app's own policy. Fixed 2026-09-07: generator widened to 16 chars to match `core::models::user::generate_password()`, generated output is now validated before use, and a regression test covers it. See `GENERATE_PASSWORD_BUG.md`.

- [x] change the text field colors on post page — they were white/bright for a dark theme; rolled out site-wide via --field-bg/--field-text
- [x] theme images take about 0.25 or higher secs to fully load. admin/themes — fixed 2026-08-18 (`7975541`): the no-store cache-buster middleware was overwriting the theme-screenshot handler's own Cache-Control on every /admin/* response, so thumbnails were never actually cached and re-fetched every load.

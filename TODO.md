# TODO

Running backlog of outstanding tasks. Not part of Claude's memory system — memory only holds a
pointer here, plus the "why" behind deferred/architectural items (see `MEMORY.md`'s "Deferred
implementation notes" for those). Add items here as they come up; check them off (or delete) once
done.

## Open

- [ ] create option when using install-vps.sh to also populate the documention table with the current docs. May need to creation new migration as part of the process.
- [ ] theme images take about 0.25 or higher secs to fully load. admin/themes  explore possible options to load load them in mem or startup for quick load or optimize in some other way
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

- [ ] revisit whether anyone who can edit a post (not just users with can_manage_forms) should be able to see the embedded form's submission count in the post editor sidebar and the "view form metrics" link — currently no permission gate on that, only the destination /admin/form-analytics page itself enforces can_manage_forms.

- [ ] decide how (or whether) to surface the install-wide (.env MAILGUN_*) email fallback account on a site's Email Settings tab — right now it's active but invisible in the UI when a form has no provider selected.

## Done

- [x] change the text field colors on post page — they were white/bright for a dark theme; rolled out site-wide via --field-bg/--field-text

# Synaptic Signals — Admin UI Conventions

## Buttons, everywhere

Every submit/action button in any form or modal — full-page or dialog — is an icon button, never a text button:
- Icon comes from the existing Feather set at `admin/static/icons/*.svg` (already installed, ~290 icons). Never invent a new icon file, use emoji, or fall back to text-only.
- Always wrapped in a pill: `.icon-pill` (page-level toolbar) or `.icon-pill-actionbuttons` (form/modal footer) containing one or more `.icon-btn` buttons.
- Never `.btn` / `.btn-primary` text buttons for submit/cancel/save actions.

## Modals

Structure:
```html
<dialog id="thing-dialog" class="modal-card">
  <form method="POST" id="thing-form">
    <h3 class="modal-card-header">Thing Title</h3>
    <div class="modal-card-body">
      <div class="form-group">...</div>
      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
        <div class="icon-pill-actionbuttons">
          <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="...">
            <img src="/admin/static/icons/x.svg" alt="">
          </button>
          <button type="submit" class="icon-btn" id="thing-save" title="Save" aria-label="Save" disabled>
            <img src="/admin/static/icons/save.svg" alt="">
          </button>
        </div>
      </div>
    </div>
  </form>
</dialog>
```

- `.modal-card` / `.modal-card-header` / `.modal-card-body` only — theme-aware via CSS vars. No inline/hardcoded colors.
- Footer buttons: `.icon-pill-actionbuttons` > `.icon-btn` (Cancel = `x.svg`, Save = `save.svg` or a semantically matching icon, e.g. `copy.svg` for "duplicate"). Never plain `.btn`/`.btn-primary` text buttons.

### Progress / long-running-task modals

A second modal variant, for a background task the JS itself opens and closes
(not a native `<dialog>`, since there's no form submission driving it) —
see the WP import modal, `admin/src/pages/sites.rs`'s Import Content tab, for
a full worked example:

```html
<div id="thing-modal" style="display:none;position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:200;align-items:center;justify-content:center">
  <div class="modal-card" style="max-width:440px;width:90%">
    <h3 class="modal-card-header">Doing the thing</h3>
    <div class="modal-card-body">
      <!-- phase label + progress bar (a background div with a width-animated fill using var(--primary)) -->
      <!-- short status line + any actionable results, shown only once finished -->
      <div style="text-align:right">
        <div class="icon-pill" id="thing-modal-actions" style="margin-top:0;display:none">
          <button class="icon-btn" title="Close" aria-label="Close" onclick="...">
            <img src="/admin/static/icons/x.svg" alt="">
          </button>
        </div>
      </div>
    </div>
  </div>
</div>
```

- `display:none` → JS sets `'flex'` on the *outer* overlay div (that one's a
  block-level backdrop, so `flex` is correct there for centering).
- For the action-buttons `.icon-pill` itself, toggle `'none'` ↔
  `'inline-flex'`, never `'flex'` — `.icon-pill`'s CSS default is
  `inline-flex` (shrink-to-fit); setting `display:'flex'` on it turns it into
  a block-level flex container that stretches to the modal's full width,
  which looks like a giant pill bar spanning the dialog. Right-align it by
  wrapping in a plain `text-align:right` div, not `justify-content` on the
  pill itself (which has no effect while it's shrink-to-fit inline-flex).
- The triggering form submits via `XMLHttpRequest` (not `fetch`) specifically
  so `xhr.upload.onprogress` can drive a real upload-progress percentage —
  `fetch` has no upload-progress event, and for anything uploading a
  meaningfully-sized file, waiting for the response before showing the modal
  at all reads as "nothing is happening."
- Backing endpoint pattern: the POST kicks off a `tokio::spawn`ed background
  task and returns immediately (JSON, not a redirect); a separate `GET
  .../status` endpoint, polled every ~1s while the modal is open, reports
  progress from state the background task updates as it runs (see
  `WpImportProgress`/`WpImportPhase` in `app_state.rs` for the shape — a
  `site_id`-keyed `HashMap` behind `std::sync::RwLock`, the same pattern
  `active_theme`/`site_cache` already use for other hot-reloadable state).
- Closing the modal should never do a full `location.reload()` just to
  "refresh" — that causes a visible flash of unstyled content while the page
  re-fetches CSS. Reset the form / navigate only if there's somewhere the
  user actually needs to land next (WP import's Close button sends them to
  `/admin`, since the settings page itself has nothing left to show).
- Any one-time secret produced by the task (e.g. newly-created account
  passwords) must never round-trip through the polled JSON status —
  `#[serde(skip)]` it server-side and serve it only through a dedicated
  single-use download endpoint that drains it from memory on first read.
  Show only counts in the modal itself.

## Edit-form validation

- Form pre-filled with an existing item's data → Save starts `disabled`; JS tracks a baseline of the loaded values and re-enables only when the live form differs from it. Reverting a field to its original value re-disables Save.
- A pre-filled *default* suggestion (e.g. "Name (copy)") still counts as the baseline — must be edited away from before Save enables.
- Create form with no existing item → Save just requires non-empty required fields; no baseline tracking.

Baseline-tracking pattern:
```js
var thingBaseline = { name: '', desc: '' };
function updateThingSaveState() {
  var nameInput = document.getElementById('thing-name-input');
  var descInput = document.getElementById('thing-desc-input');
  var saveBtn = document.getElementById('thing-save');
  var empty = nameInput.value.trim().length === 0;
  var unchanged = nameInput.value === thingBaseline.name && descInput.value === thingBaseline.desc;
  saveBtn.disabled = empty || unchanged;
}
nameInput.addEventListener('input', updateThingSaveState);
descInput.addEventListener('input', updateThingSaveState);
// when opening with existing data:
thingBaseline = { name: currentName, desc: currentDesc || '' };
nameInput.value = currentName;
descInput.value = currentDesc || '';
updateThingSaveState();
```

# Synaptic Signals — Admin UI Conventions

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

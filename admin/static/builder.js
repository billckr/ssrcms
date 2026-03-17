/**
 * Visual Page Builder — SortableJS + Quill client logic.
 * Loaded by the builder editor page. Initial state is provided
 * via window.__builderInit set inline in the page.
 */
(function () {
  "use strict";

  const init = window.__builderInit || {};
  const SAVE_URL    = init.saveUrl    || "/admin/appearance/builder/save";
  const COMP_ID     = init.compId     || null;
  const THEME_NAME  = init.themeName  || "";
  const PREVIEW_URL = "/admin/appearance/builder/preview";

  // Layout → zone list
  const LAYOUT_ZONES = {
    "single-column": ["header", "main", "footer"],
    "left-sidebar":  ["header", "sidebar", "main", "footer"],
    "right-sidebar": ["header", "main", "sidebar", "footer"],
  };

  // Block type definitions
  const BLOCK_DEFS = {
    "text-block":   { label: "Text Block",   icon: "📝", defaultConfig: { content: "", bg: "", padding: "medium" } },
    "posts-grid":   { label: "Posts Grid",   icon: "📰", defaultConfig: { limit: 6, columns: 2, category: "", bg: "" } },
    "nav-menu":     { label: "Nav Menu",     icon: "🧭", defaultConfig: { menu_location: "primary", orientation: "horizontal", bg: "" } },
    "contact-form": { label: "Contact Form", icon: "✉️", defaultConfig: { title: "Contact Us", submit_label: "Send Message", bg: "" } },
  };

  // ── State ──────────────────────────────────────────────────────────────────
  const savedComp = init.composition || {};
  const state = {
    layout: init.layout || "single-column",
    zones: (savedComp.zones) ? savedComp.zones : {},
    selected: null,  // { zone, idx }
  };

  // Sync layout picker to state on page load
  const layoutPicker = document.getElementById("layout-picker");
  if (layoutPicker) layoutPicker.value = state.layout;

  // ── Layout picker ──────────────────────────────────────────────────────────
  window.builderLayoutChanged = function (layout) {
    state.layout = layout;
    state.selected = null;
    renderCanvas();
    renderConfig();
    schedulePreview();
  };

  // ── Canvas ─────────────────────────────────────────────────────────────────
  function renderCanvas() {
    const canvas = document.getElementById("canvas");
    if (!canvas) return;
    const zones = LAYOUT_ZONES[state.layout] || ["header", "main", "footer"];
    canvas.innerHTML = "";

    zones.forEach(function (zone) {
      const blocks = state.zones[zone] || [];
      const zoneEl = document.createElement("div");
      zoneEl.className = "canvas-zone";

      const label = document.createElement("div");
      label.className = "canvas-zone-label";
      label.innerHTML =
        '<span style="width:8px;height:8px;border-radius:50%;background:#a5b4fc;display:inline-block;margin-right:.4rem"></span>' +
        escHtml(zone);
      zoneEl.appendChild(label);

      const dropEl = document.createElement("div");
      dropEl.className = "canvas-zone-drop";
      dropEl.dataset.zone = zone;

      if (blocks.length === 0) {
        dropEl.innerHTML = '<div class="drop-hint">Drop blocks here</div>';
      }
      blocks.forEach(function (block, idx) {
        dropEl.appendChild(makeChip(block, zone, idx));
      });

      zoneEl.appendChild(dropEl);
      canvas.appendChild(zoneEl);

      // SortableJS on drop zone
      if (window.Sortable) {
        Sortable.create(dropEl, {
          group: "blocks",
          animation: 150,
          ghostClass: "sortable-ghost",
          onEnd: function (evt) {
            const fromZone = evt.from.dataset.zone;
            const toZone   = evt.to.dataset.zone;
            const oldIdx   = evt.oldIndex;
            const newIdx   = evt.newIndex;
            if (!state.zones[fromZone]) return;
            const block = state.zones[fromZone].splice(oldIdx, 1)[0];
            if (!state.zones[toZone]) state.zones[toZone] = [];
            state.zones[toZone].splice(newIdx, 0, block);
            if (state.selected &&
                state.selected.zone === fromZone &&
                state.selected.idx  === oldIdx) {
              state.selected = { zone: toZone, idx: newIdx };
            }
            renderCanvas();
            renderConfig();
            schedulePreview();
          },
        });
      }
    });

    // SortableJS on palette (clone mode — initialised once)
    const paletteBlocks = document.getElementById("palette-blocks");
    if (window.Sortable && paletteBlocks && !paletteBlocks._sortableInit) {
      paletteBlocks._sortableInit = true;
      Sortable.create(paletteBlocks, {
        group: { name: "blocks", pull: "clone", put: false },
        sort: false,
        animation: 150,
        onEnd: function (evt) {
          if (evt.to === paletteBlocks) return;
          const zone = evt.to.dataset.zone;
          if (!zone) return;
          const blockType = evt.item.dataset.blockType;
          if (!blockType) return;
          evt.item.remove(); // remove cloned DOM; state drives rendering
          const def = BLOCK_DEFS[blockType];
          const newBlock = {
            block_type: blockType,
            config: Object.assign({}, def ? def.defaultConfig : {}),
          };
          if (!state.zones[zone]) state.zones[zone] = [];
          state.zones[zone].splice(evt.newIndex, 0, newBlock);
          state.selected = { zone: zone, idx: evt.newIndex };
          renderCanvas();
          renderConfig();
          schedulePreview();
        },
      });
    }
  }

  function makeChip(block, zone, idx) {
    const def = BLOCK_DEFS[block.block_type] || { label: block.block_type, icon: "□" };
    const isSelected = state.selected &&
      state.selected.zone === zone &&
      state.selected.idx  === idx;

    const chip = document.createElement("div");
    chip.className = "canvas-block-chip" + (isSelected ? " selected" : "");
    chip.dataset.zone = zone;
    chip.dataset.idx  = idx;
    chip.innerHTML =
      '<span class="chip-label">' + def.icon + " " + escHtml(def.label) + "</span>" +
      '<span class="chip-actions">' +
        '<button class="chip-btn" title="Remove" ' +
          'onclick="removeBlock(\'' + escHtml(zone) + "'," + idx + ')">✕</button>' +
      "</span>";

    chip.addEventListener("click", function (e) {
      if (e.target.closest(".chip-btn")) return;
      state.selected = { zone: zone, idx: idx };
      renderCanvas();
      renderConfig();
    });
    return chip;
  }

  window.removeBlock = function (zone, idx) {
    if (!state.zones[zone]) return;
    state.zones[zone].splice(idx, 1);
    if (state.selected && state.selected.zone === zone) state.selected = null;
    renderCanvas();
    renderConfig();
    schedulePreview();
  };

  // ── Config panel ───────────────────────────────────────────────────────────
  function renderConfig() {
    const emptyEl  = document.getElementById("config-empty");
    const fieldsEl = document.getElementById("config-fields");
    if (!emptyEl || !fieldsEl) return;

    if (!state.selected) {
      emptyEl.style.display  = "";
      fieldsEl.style.display = "none";
      fieldsEl.innerHTML = "";
      return;
    }

    const { zone, idx } = state.selected;
    const block = (state.zones[zone] || [])[idx];
    if (!block) {
      emptyEl.style.display  = "";
      fieldsEl.style.display = "none";
      return;
    }

    emptyEl.style.display  = "none";
    fieldsEl.style.display = "";
    fieldsEl.innerHTML = buildConfigHtml(block);

    // Attach Quill for text-block content field
    if (block.block_type === "text-block") {
      initQuill(block);
    }
  }

  function configFieldsFor(blockType) {
    switch (blockType) {
      case "text-block":
        return [
          { key: "content",  label: "Content",                    type: "quill" },
          { key: "bg",       label: "Background color",           type: "color" },
          { key: "padding",  label: "Padding",                    type: "select", options: ["small", "medium", "large"] },
        ];
      case "posts-grid":
        return [
          { key: "limit",    label: "Post limit (1–20)",          type: "number", min: 1, max: 20 },
          { key: "columns",  label: "Columns",                    type: "select", options: ["1", "2", "3"] },
          { key: "category", label: "Category slug (optional)",   type: "text" },
          { key: "bg",       label: "Background color",           type: "color" },
        ];
      case "nav-menu":
        return [
          { key: "menu_location", label: "Menu location",        type: "text" },
          { key: "orientation",   label: "Orientation",          type: "select", options: ["horizontal", "vertical"] },
          { key: "bg",            label: "Background color",     type: "color" },
        ];
      case "contact-form":
        return [
          { key: "title",        label: "Form title",            type: "text" },
          { key: "submit_label", label: "Submit button label",   type: "text" },
          { key: "bg",           label: "Background color",      type: "color" },
        ];
      default:
        return [];
    }
  }

  function buildConfigHtml(block) {
    const def = BLOCK_DEFS[block.block_type];
    const header = '<h3>' + escHtml(def ? def.label : block.block_type) + '</h3>';
    const fields = configFieldsFor(block.block_type);
    return header + fields.map(function (f) {
      return buildFieldHtml(f, block.config[f.key]);
    }).join("");
  }

  function buildFieldHtml(f, value) {
    const val = (value !== undefined && value !== null) ? value : "";

    if (f.type === "quill") {
      return '<div class="config-field"><label>' + escHtml(f.label) + '</label>' +
        '<div id="quill-editor" style="height:180px;border:1px solid #ddd;border-radius:4px;background:#fff"></div></div>';
    }

    if (f.type === "select") {
      const opts = f.options.map(function (o) {
        return '<option value="' + escHtml(o) + '"' + (String(val) === o ? " selected" : "") + ">" + escHtml(o) + "</option>";
      }).join("");
      return '<div class="config-field"><label>' + escHtml(f.label) +
        '</label><select data-key="' + escHtml(f.key) + '" onchange="updateConfig(\'' +
        escHtml(f.key) + '\',this.value)">' + opts + "</select></div>";
    }

    if (f.type === "color") {
      const colorVal = val || "#ffffff";
      return '<div class="config-field"><label>' + escHtml(f.label) +
        '</label><div style="display:flex;gap:.4rem;align-items:center">' +
        '<input type="color" data-key="' + escHtml(f.key) + '" value="' + escHtml(colorVal) +
        '" onchange="updateConfig(\'' + escHtml(f.key) + '\',this.value)" style="width:40px;height:32px;padding:1px;border-radius:4px">' +
        '<input type="text" data-key="' + escHtml(f.key) + '-text" value="' + escHtml(colorVal) +
        '" placeholder="#ffffff" oninput="updateColorText(this,\'' + escHtml(f.key) + '\')" style="flex:1"></div></div>';
    }

    if (f.type === "number") {
      return '<div class="config-field"><label>' + escHtml(f.label) +
        '</label><input type="number" data-key="' + escHtml(f.key) + '" value="' + escHtml(String(val)) + '"' +
        (f.min !== undefined ? ' min="' + f.min + '"' : "") +
        (f.max !== undefined ? ' max="' + f.max + '"' : "") +
        ' onchange="updateConfig(\'' + escHtml(f.key) + '\',Number(this.value))"></div>';
    }

    return '<div class="config-field"><label>' + escHtml(f.label) +
      '</label><input type="text" data-key="' + escHtml(f.key) + '" value="' + escHtml(String(val)) +
      '" oninput="updateConfig(\'' + escHtml(f.key) + '\',this.value)"></div>';
  }

  window.updateConfig = function (key, value) {
    if (!state.selected) return;
    const { zone, idx } = state.selected;
    const block = (state.zones[zone] || [])[idx];
    if (!block) return;
    block.config[key] = value;
    schedulePreview();
  };

  window.updateColorText = function (input, key) {
    const picker = document.querySelector('[data-key="' + key + '"]');
    if (picker) picker.value = input.value;
    updateConfig(key, input.value);
  };

  // ── Quill ──────────────────────────────────────────────────────────────────
  function initQuill(block) {
    if (!window.Quill) return;
    setTimeout(function () {
      const editorEl = document.getElementById("quill-editor");
      if (!editorEl || editorEl._quillInit) return;
      editorEl._quillInit = true;
      const q = new Quill(editorEl, {
        theme: "snow",
        modules: {
          toolbar: [
            ["bold", "italic", "underline", "strike"],
            [{ list: "ordered" }, { list: "bullet" }],
            ["link"],
            [{ header: [1, 2, 3, false] }],
            ["clean"],
          ],
        },
      });
      if (block.config.content) {
        q.root.innerHTML = block.config.content;
      }
      q.on("text-change", function () {
        if (!state.selected) return;
        const { zone, idx } = state.selected;
        const b = (state.zones[zone] || [])[idx];
        if (b) {
          b.config.content = q.root.innerHTML;
          schedulePreview();
        }
      });
    }, 50);
  }

  // ── Preview ────────────────────────────────────────────────────────────────
  let previewTimer = null;
  function schedulePreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(fetchPreview, 500);
  }

  function fetchPreview() {
    const frame = document.getElementById("builder-preview-frame");
    if (!frame) return;
    const payload = {
      layout: state.layout,
      composition: { zones: state.zones },
      theme_name: THEME_NAME,
    };
    fetch(PREVIEW_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    })
    .then(function (res) { return res.text(); })
    .then(function (html) {
      frame.srcdoc = html;
      frame.style.display = "block";
    })
    .catch(function (e) {
      console.warn("preview fetch failed:", e);
    });
  }

  // ── Save ───────────────────────────────────────────────────────────────────
  window.builderSave = async function () {
    const nameInput = document.getElementById("comp-name");
    const name = nameInput ? nameInput.value.trim() : "";
    if (!name) { showFlash("Please enter a composition name.", "error"); return; }

    const payload = {
      id: COMP_ID,
      name: name,
      layout: state.layout,
      composition: { zones: state.zones },
    };

    try {
      const res = await fetch(SAVE_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      const data = await res.json();
      if (data.ok) {
        if (data.id && !COMP_ID) {
          window.location.href = "/admin/appearance/builder/" + data.id + "/edit?saved=1";
        } else {
          showFlash("Composition saved.", "success");
        }
      } else {
        showFlash("Save failed: " + (data.error || "unknown error"), "error");
      }
    } catch (e) {
      showFlash("Network error: " + e.message, "error");
    }
  };

  // ── Flash ──────────────────────────────────────────────────────────────────
  function showFlash(msg, type) {
    let el = document.getElementById("builder-flash");
    if (!el) {
      el = document.createElement("div");
      el.id = "builder-flash";
      el.style.cssText = "position:fixed;top:16px;right:16px;padding:.6rem 1rem;" +
        "border-radius:6px;font-size:.85rem;z-index:9999;max-width:340px;" +
        "box-shadow:0 2px 8px rgba(0,0,0,.18)";
      document.body.appendChild(el);
    }
    el.textContent = msg;
    el.style.background = type === "error" ? "#fef2f2" : "#f0fdf4";
    el.style.border      = type === "error" ? "1px solid #fca5a5" : "1px solid #86efac";
    el.style.color       = type === "error" ? "#b91c1c" : "#166534";
    el.style.display = "block";
    clearTimeout(el._hideTimer);
    el._hideTimer = setTimeout(function () { el.style.display = "none"; }, 4000);
  }

  function escHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  // ── Init ───────────────────────────────────────────────────────────────────
  // Sync the layout picker and re-render canvas with the saved layout.
  // Calling builderLayoutChanged ensures the select, canvas, and zones all agree.
  builderLayoutChanged(state.layout);

  // Show saved flash when redirected after new composition save
  if (new URLSearchParams(window.location.search).get("saved") === "1") {
    showFlash("Composition saved.", "success");
    history.replaceState(null, "", window.location.pathname);
  }
})();

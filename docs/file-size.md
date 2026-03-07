# File Size Notes

## Source File Line Counts (as of 2026-03-07)

Total: **21,801 lines across 95 files** (excluding `target/`)

### Core — Handlers (Admin)
| File | Lines |
|------|------:|
| `core/src/handlers/admin/appearance.rs` | 1,749 |
| `core/src/handlers/admin/users.rs` | 1,086 |
| `core/src/handlers/admin/posts.rs` | 909 |
| `core/src/handlers/admin/plugins.rs` | 661 |
| `core/src/handlers/admin/sites.rs` | 636 |
| `core/src/handlers/admin/forms.rs` | 256 |
| `core/src/handlers/admin/taxonomy.rs` | 165 |
| `core/src/handlers/admin/profile.rs` | 145 |
| `core/src/handlers/admin/media.rs` | 136 |
| `core/src/handlers/admin/upload.rs` | 123 |
| `core/src/handlers/admin/mod.rs` | 85 |
| `core/src/handlers/admin/settings.rs` | 81 |
| `core/src/handlers/admin/dashboard.rs` | 73 |
| `core/src/handlers/admin/comments.rs` | 41 |

### Core — Handlers (Public)
| File | Lines |
|------|------:|
| `core/src/handlers/home.rs` | 295 |
| `core/src/handlers/post_unlock.rs` | 261 |
| `core/src/handlers/archive.rs` | 242 |
| `core/src/handlers/auth.rs` | 219 |
| `core/src/handlers/subscribe.rs` | 207 |
| `core/src/handlers/plugin_route.rs` | 160 |
| `core/src/handlers/post.rs` | 155 |
| `core/src/handlers/account.rs` | 149 |
| `core/src/handlers/page.rs` | 134 |
| `core/src/handlers/search.rs` | 111 |
| `core/src/handlers/theme_static.rs` | 94 |
| `core/src/handlers/form.rs` | 82 |
| `core/src/handlers/comment.rs` | 82 |
| `core/src/handlers/metrics.rs` | 36 |
| `core/src/handlers/mod.rs` | 15 |

### Core — Models
| File | Lines |
|------|------:|
| `core/src/models/post.rs` | 804 |
| `core/src/models/user.rs` | 562 |
| `core/src/models/site.rs` | 262 |
| `core/src/models/taxonomy.rs` | 197 |
| `core/src/models/site_user.rs` | 183 |
| `core/src/models/form_submission.rs` | 181 |
| `core/src/models/media.rs` | 166 |
| `core/src/models/comment.rs` | 145 |
| `core/src/models/mod.rs` | 9 |

### Core — Templates
| File | Lines |
|------|------:|
| `core/src/templates/loader.rs` | 404 |
| `core/src/templates/functions.rs` | 323 |
| `core/src/templates/filters.rs` | 313 |
| `core/src/templates/context.rs` | 166 |
| `core/src/templates/mod.rs` | 8 |

### Core — Other
| File | Lines |
|------|------:|
| `core/src/main.rs` | 441 |
| `core/src/app_state.rs` | 298 |
| `core/src/middleware/admin_auth.rs` | 284 |
| `core/src/config.rs` | 283 |
| `core/src/router.rs` | 189 |
| `core/src/search/index.rs` | 241 |
| `core/src/middleware/site.rs` | 116 |
| `core/src/errors.rs` | 116 |
| `core/src/search/indexer.rs` | 74 |
| `core/src/scheduler.rs` | 42 |
| `core/src/db.rs` | 24 |
| `core/src/lib.rs` | 16 |
| `core/src/utils/slugify.rs` | 88 |
| `core/src/plugins/loader.rs` | 133 |
| `core/src/plugins/hook_registry.rs` | 70 |
| `core/src/plugins/manifest.rs` | 59 |
| `core/src/search/mod.rs` | 4 |
| `core/src/middleware/mod.rs` | 3 |
| `core/src/plugins/mod.rs` | 7 |
| `core/src/utils/mod.rs` | 1 |

### Core — Tests
| File | Lines |
|------|------:|
| `core/tests/model_crud.rs` | 496 |
| `core/tests/theme_e2e.rs` | 146 |
| `core/tests/routes.rs` | 64 |

### Admin (Leptos UI)
| File | Lines |
|------|------:|
| `admin/src/pages/users.rs` | 926 |
| `admin/src/pages/posts.rs` | 713 |
| `admin/src/pages/appearance.rs` | 574 |
| `admin/src/lib.rs` | 399 |
| `admin/src/pages/sites.rs` | 291 |
| `admin/src/pages/forms.rs` | 221 |
| `admin/src/pages/account.rs` | 213 |
| `admin/src/pages/settings.rs` | 189 |
| `admin/src/pages/plugins.rs` | 182 |
| `admin/src/pages/media.rs` | 129 |
| `admin/src/pages/subscribe.rs` | 95 |
| `admin/src/pages/profile.rs` | 86 |
| `admin/src/pages/taxonomy.rs` | 72 |
| `admin/src/pages/login.rs` | 58 |
| `admin/src/pages/mod.rs` | 14 |
| `admin/src/components/mod.rs` | 2 |

### CLI
| File | Lines |
|------|------:|
| `cli/src/commands/install.rs` | 650 |
| `cli/src/commands/site.rs` | 396 |
| `cli/src/commands/dev.rs` | 248 |
| `cli/src/commands/theme.rs` | 203 |
| `cli/src/commands/user.rs` | 190 |
| `cli/src/commands/caddy.rs` | 150 |
| `cli/src/commands/plugin.rs` | 59 |
| `cli/src/commands/migrate.rs` | 25 |
| `cli/src/commands/mod.rs` | 22 |
| `cli/src/main.rs` | 73 |

---

## appearance.rs — Is 1,749 Lines a Problem?

**Short answer: No.** The file is legitimately dense, not bloated. It owns the entire theming system
for both admins and users — upload, extraction, file editing, backup/restore, activation, deletion,
screenshot serving, theme creation, and publishing — plus all the filesystem path resolution logic
that underpins those operations.

### Function inventory (28 functions)

| Category | Functions | Approx. Lines |
|----------|-----------|--------------|
| Route handlers: list, activate, delete, screenshot | 4 | ~330 |
| Theme upload & ZIP extraction | 3 | ~200 |
| Theme create (form + submit) | 2 | ~100 |
| Theme file editor: get, new, edit, save, restore, delete | 6 | ~500 |
| Theme publish/export | 1 | ~100 |
| Filesystem helpers: dir resolution, file walking, path utils | 12 | ~350 |
| List renderer + theme scanner | 2 | ~130 |

The file editor alone (6 handlers + 12 helpers) accounts for nearly half the file. The size is a
consequence of scope, not poor organization.

---

## Possible Future Refactor

If the file becomes hard to navigate or the editor and manager need to diverge in ownership, there
is a clean split into a `handlers/admin/appearance/` module:

```
core/src/handlers/admin/appearance/
    mod.rs          — list handler, render_appearance_list, scan_theme_dir (~350 lines)
    file_editor.rs  — edit/save/restore/delete/new file handlers + all filesystem helpers (~700 lines)
    theme_manager.rs — upload, extract, create, activate, delete, screenshot, publish (~700 lines)
```

**Do not do this preemptively.** The current single-file layout is fine and avoids cross-module
import churn. Revisit only if:
- A second developer needs to own one area independently, or
- The file grows past ~2,500 lines, or
- Tests or bugs keep touching the same helper functions from different concerns.

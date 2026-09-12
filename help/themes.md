---
title: Themes
group: feature
---
# Themes

Themes control the public site's templates, layout, colors, and styling. The Themes page manages
the current site's copies as well as shared theme libraries. Theme changes affect visitor-facing
pages, so preview the important routes after editing or activation.

- [Theme libraries](#theme-libraries)
- [Getting and activating themes](#activating-themes)
- [Creating or uploading a theme](#installing-themes)
- [Customizing a theme](#customizing-themes)
- [Editing theme files](#editing-theme-files)
- [Removing and publishing themes](#removing-themes)

<a id="theme-libraries"></a>
## Theme libraries

Use the filter at the top of the page:

- **My Themes** contains editable copies belonging to the current site. The active card is
  highlighted. Cards show the theme preview, name, version, description, author, and actions.
- **Global Themes** contains shared themes available to sites. Get Theme copies one into My Themes;
  it does not activate it. **My Themes** on a card means a copy already exists.
- **Private Themes** is visible only to a Super Admin who is not visiting another site's admin.
  It contains the Super Admin's private working library and adds direct Edit, Publish to Global,
  and Remove controls.

A Private badge on a site copy records where it originated. A number badge on a global theme tells
a Super Admin how many sites currently use it.

<a id="activating-themes"></a>
## Getting and activating themes

From Global or Private Themes, select **Get Theme** to copy the theme into the current site's My
Themes library. Existing copies are not overwritten by this action. Switch to My Themes, then:

- **Activate** makes that theme live for the current site after confirmation. The switch and
  template reload are immediate; no application restart is normally needed.
- **Customize Theme** opens the theme's visual customizer or file editor.
- **Remove** deletes an inactive site copy and its local edits.

Activating a different theme does not delete the old one, so keep a known-good theme available as
a fallback. After activation, check the homepage, a post, a page, category/tag archives, search,
forms or polls, and a missing-page URL.

<a id="installing-themes"></a>
## Creating or uploading a theme

Select **Create Theme** to make a complete starter copy of the default theme. Enter:

- **Theme name** — the folder identifier; use letters, numbers, hyphens, or underscores, with no
  slashes, backslashes, or leading dot.
- **Description** — an optional summary of up to 30 characters.
- **Author** — the person or organization maintaining it.
- **Visibility** — Super Admins choose Private or Public. Private stays in their personal library;
  Public enters the global library. Site Admin creations belong to the current site.

Creating opens the new theme in the editor but does not activate it.

Select **Upload & Install Theme (.zip)** to choose a ZIP archive. It may contain the theme at the
archive root or inside one top-level folder. A valid `theme.toml` and all required templates must
be present. The installation-wide upload-size limit applies. Uploads by a Super Admin go to Global
Themes; Site Admin uploads go to the current site's My Themes library.

Warning: uploading a valid theme with the same theme name replaces that installed copy in full.
Back up local work before uploading an update.

<a id="customizing-themes"></a>
## Customizing a theme

Themes that declare visual customizer options open on grouped cards. The exact controls vary by
theme and can include:

- color pickers;
- on/off layout choices;
- single-choice options;
- text values;
- Media Library image choices; and
- drag-to-reorder lists.

Save Changes applies the values in that card. Restore Original appears after an override exists
and returns the affected colors or options to the theme's defaults, overwriting their current
changes. Theme Details shows manifest information, and Files opens the lower-level editor.

Customizer settings are separate by group, so save each changed card. Open the public site after
changes; the option labels describe intent, but the theme decides exactly where each value appears.

<a id="editing-theme-files"></a>
## Editing theme files

The Files selector lists editable templates and assets. A star marks a file with an original
backup, and the page shows when it was edited. Select New File to enter a relative name and choose
`.html`, `.css`, `.js`, or `.xml`.

When a file is open:

- edit it in the line-numbered text area and select **Save File**;
- use the Colors panel on compatible CSS files to change detected `:root` hex variables, then Save
  File to apply them;
- select **Restore Original** to replace current edits with the saved original backup; or
- select **Delete File** for an optional file. Required templates cannot be deleted.

Restoring and deleting are destructive. Copy important changes elsewhere first. Template syntax
errors can break public rendering: `{% extends %}` must be the very first line of a child template,
and Tera or HTML comments placed outside template blocks can also cause parsing errors.

A shared Global Theme can appear read-only to a Site Admin. Get a site copy and edit it from My
Themes. Super Admins can directly edit Private originals and other copies their permissions allow.

<a id="removing-themes"></a>
## Removing and publishing themes

An active site theme cannot be removed; activate another theme first. Removing from My Themes
deletes only the current site's copy and local edits. A fresh shared copy can be obtained later,
but the removed edits are not recoverable.

Only Super Admins can permanently delete Global or Private originals. A Global theme cannot be
deleted while any site uses it. Deleting a Private original leaves site copies made from it
untouched.

From Private Themes, **Publish to Global** copies the current private version into the shared
library. If a global theme with the same name already exists, confirmation overwrites that global
copy. Private and existing site copies remain separate, so later edits do not synchronize
automatically.

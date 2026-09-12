---
title: Pages
group: feature
---
# Pages

Pages are standalone content such as About, Contact, or Terms. They use the same rich-text editor
as posts but do not belong to the dated post feed and do not use categories, tags, or post sources.
The Pages area is available to Editors and administrators, not the Author role.

- [Finding and managing pages](#finding-pages)
- [Writing a page](#writing-pages)
- [Page options](#page-options)
- [Templates and parent pages](#page-structure)
- [Media and translations](#page-media)
- [Saving and deleting](#saving-pages)

<a id="finding-pages"></a>
## Finding and managing pages

Use the **All**, **Published**, **Draft**, and **Trashed** tabs to filter by status. Open search with
the magnifying-glass button; results update as you type while staying inside the active status tab.
Select the **Title**, **Status**, **Author**, **Domain**, or displayed date headings to sort, and
select the active heading again to reverse the order. Pagination preserves the current filters.

Select a title or the Edit button to open a page. View is available for Published and Scheduled
content. Scheduled and Pending Review pages remain available from All even though they do not have
separate page tabs. The row trash button permanently deletes after confirmation—it does not move
the page to Trashed.
Select multiple checkboxes, or the heading checkbox for the visible page, to reveal the permanent
**Delete Selected** action. Select New Page to start a page.

<a id="writing-pages"></a>
## Writing a page

- **Title** is required and limited to 255 characters.
- **Slug** becomes the public path, such as `about`. It is generated from the title until manually
  edited and is normalized to lowercase letters, numbers, and hyphens.
- **Excerpt** is required, limited to 500 characters, and used as the page's meta description.
- **Content** provides headings, emphasis, quotes, code, lists, links, images, audio, formatting
  cleanup, and reusable form or poll insertion.

Pages normally open at `/<slug>`. A Parent Page can make the path hierarchical. Changing a saved
slug or parent changes the public URL, so update menus and external links as needed.

<a id="page-options"></a>
## Page options

New pages display the Status selector. On an existing page, select Change Status first.

- **Draft** keeps the page private.
- **Pending Review** marks it for editorial review when that option is available.
- **Published** makes it public.
- **Scheduled** publishes at the selected UTC date and time.
- **Trashed** keeps an existing page stored but removes it from normal public use.

The editor also displays original and last-updated dates. Editors and administrators can allow or
disable comments and can password-protect a page. Disabling comments retains existing comments
but stops displaying them and accepting new ones. Saved passwords are never shown; enter a new one
only to replace it, or clear Password Protect to remove protection.

<a id="page-structure"></a>
## Templates and parent pages

- **Template** selects an optional page template supplied by the active theme. Default uses
  `page.html`. The selector appears only when the theme provides additional eligible templates.
- **Parent Page** nests this page under another published page, producing a path such as
  `/company/team`. Choose None for a top-level page. The selector appears when an eligible parent
  exists and prevents a page from selecting itself.

Template appearance and available choices are theme-specific. Check the public page after either
setting changes.

<a id="page-media"></a>
## Media and translations

Choose or remove a **Featured Image** through the Media Library. Use the Content toolbar to insert
inline images or audio and saved forms or polls. Embedded items appear as placeholders in the
editor and become working components on the public page. **Inline Media** summarizes body images
and audio; remove them from the Content editor itself.

When AI Translation is enabled, a saved page has the same Translations controls as a post. Choose
an enabled language and verified provider, use the globe button for page prose, and use the layers
button when only embedded forms or polls are missing or stale. Save source edits first. Existing
translations are marked Source changed after prose edits, and deleting a translation leaves the
source page untouched. Provider and language setup is explained under [Site Settings](/help#doc-sites).

<a id="saving-pages"></a>
## Saving and deleting

Save enables after a real change and the browser warns before leaving unsaved work. The eye button
opens a live page or staff preview when one is available. A stale edit is rejected if someone else
saved a newer version first, preventing an unnoticed overwrite.

Delete Page and list-page delete actions are permanent. To remove a page from public use while
retaining it, change its status to Trashed instead. Also review menus, child pages, and links before
changing or deleting an established page.

---
title: Posts
group: feature
---
# Posts

Posts are dated articles that appear in the site's post feed and can be organized with categories
and tags. Use the Posts page to find, sort, create, edit, preview, publish, or permanently delete
them.

- [Finding and managing posts](#finding-posts)
- [Writing a post](#writing-posts)
- [Post options](#post-options)
- [Media, forms, and polls](#post-media)
- [Sources](#post-sources)
- [Translations](#post-translations)
- [Saving, previewing, and deleting](#saving-posts)

<a id="finding-posts"></a>
## Finding and managing posts

The status tabs narrow the list to **All**, **Published**, **Draft**, **Pending Review**,
**Scheduled**, or **Trashed**. The Pending Review badge shows how many posts are waiting. Authors
do not see the Trashed tab, and see Scheduled only when they have scheduled content.

Use the magnifying-glass button to search the current status view. Search matches post titles and
author display names, and updates the table as you type. Select a column heading—**Title**,
**Status**, **Author**, **Domain**, or a displayed date—to sort; select it again to reverse the
order. Filters, search, and sorting remain in effect while moving through pagination.

Each row can offer:

- **View** for Published or Scheduled content. It opens the public page or authorized preview in a
  new tab.
- **Edit**, also available by selecting the title.
- **Delete**, which permanently removes the post after confirmation. It does not move the item to
  Trashed.

Select row checkboxes—or the heading checkbox for every visible row—to reveal **Delete Selected**.
Bulk deletion is also permanent. Authors do not receive the list's Delete controls. Restricted
Authors can edit only their own Draft or Pending Review posts; Authors granted self-publishing
permission can also manage their own published and scheduled work.

Select New Post to open a blank editor.

<a id="writing-posts"></a>
## Writing a post

- **Title** is required and limited to 255 characters.
- **Slug** is the post's URL name. It is generated from the title until you edit it yourself, then
  normalized to lowercase letters, numbers, and hyphens. Changing a published slug changes its
  public URL.
- **Excerpt** is required, limited to 500 characters, and used as the post's meta description.
- **Content** is the main rich-text editor. Use its toolbar for headings, emphasis, block quotes,
  code blocks, numbered or bulleted lists, links, images, and formatting cleanup.

The Save button enables after a real change, and a Save Changes indicator appears. If you attempt
to leave with unsaved work, the browser warns you. Publishing is blocked when the content is
effectively empty.

<a id="post-options"></a>
## Post options

New posts display the **Status** selector directly. Existing posts show the current status; select
Change Status to reveal the selector.

- **Draft** keeps the post private and editable.
- **Pending Review** marks it for an editor. Authors without self-publishing permission can save
  only Draft or Submit for Review.
- **Published** makes it public when saved.
- **Scheduled** reveals a UTC date and time. The post becomes public when that time arrives.
- **Trashed** keeps an existing post in the database but removes it from normal public listings.

The editor also shows the original and last-updated dates. If the post contains a form, it shows
submission totals and may offer a shortcut to that form's results.

Editors and administrators can control:

- **Allow/Disable Comments** — turning comments off keeps existing comments but hides them and
  blocks new ones. The badge shows the existing count.
- **Password Protect** — requires visitors to enter the chosen password before viewing the post.
  Existing passwords are never displayed; use Change Password to replace one, or clear Password
  Protect to remove it.

Use the collapsible **Categories** and **Tags** sections to select existing terms. Create missing
terms from their own admin pages first. The **Author** card identifies the current author for
editors and administrators; change account or site access from Users rather than here.

<a id="post-media"></a>
## Media, forms, and polls

Use **Featured Image** to choose an item from the Media Library or remove the current selection.
The active theme decides where the featured image appears.

The Content toolbar's image and audio controls insert Media Library items at the cursor. The form
and poll controls insert reusable items created in Designer. These appear as placeholders in the
editor and are expanded into working visitor controls on the live page. The collapsible **Inline
Media** panel lists images and audio currently embedded in the body; edit or remove the item in the
content itself.

<a id="post-sources"></a>
## Sources

Sources are optional `http://` or `https://` reference links for a post. Select Add Source URL,
enter a complete address, and drag rows by their handles to reorder them. Remove a row with its
trash button. On an existing post, **Save Sources** saves this list separately from the main post
form. **Show sources on the live page** is also saved immediately when changed. Whether and how a
public source list appears depends on the active theme.

<a id="post-translations"></a>
## Translations

For a saved post, the Translations section appears when AI Translation is enabled. It lists each
localized copy, whether the source has changed, generation time, and the Current/Missing/Source
changed state of embedded forms and polls.

Choose an enabled language and verified provider. Use the globe button to translate or refresh
the post prose. If the post already has a translation but only an embedded form or poll needs work,
use the layers button to translate those embedded items without paying to translate the prose
again. Save ordinary post edits before translating—the translator always uses the last saved
version. The trash button removes only that locale's translation, not the source post.

See [Site Settings](/help#doc-sites) for language and provider setup. AI-generated text should be
reviewed by someone who understands the target language.

<a id="saving-posts"></a>
## Saving, previewing, and deleting

Select Save after changing the content or options. The eye button opens the live post when
published, or a staff-only preview when available. If another editor saved a newer version first,
SynapCMS rejects the stale save instead of silently overwriting their work; reload and reconcile
your changes.

Delete Post in the editor and every list-page delete action permanently remove the post and its
related data after confirmation. Use the Trashed status when you want to take a post out of public
use without immediately deleting it.

---
title: Categories
group: feature
---
# Categories

Categories organize posts into broad subjects, such as News, Guides, or Events. A post can belong
to more than one category, and themes can expose public category archive pages. Categories can
also be used in the site's post permalink structure.

- [Viewing and sorting categories](#viewing-categories)
- [Adding a category](#adding-categories)
- [Using and deleting categories](#using-categories)

<a id="viewing-categories"></a>
## Viewing and sorting categories

The table shows each category's **Name**, **Slug**, and number of associated **Posts**. Select any
of those column headings to sort by it; select the active heading again to reverse the order.

The count includes associations with non-public content, so it may not match the number visible
on the public category archive.

<a id="adding-categories"></a>
## Adding a category

1. Enter the visitor-facing **Name** in the Add Category panel.
2. Review the generated **Slug**, or replace it with your own.
3. Select Add Category.

The slug is the URL-safe identifier used in paths such as `/category/company-news`. It must use
only lowercase letters, numbers, and hyphens. Spaces are changed to hyphens, and leaving it blank
generates a value from the name. Names and slugs must be unique within the site.

After creating the category, select it from a post's **Categories** section. There is no rename or
edit action on the Categories page currently; if a category must change, create the replacement
and update the affected posts.

<a id="using-categories"></a>
## Using and deleting categories

Select the trash button and confirm to delete a category. The posts remain, but their association
with that category is removed. This can affect category archive links and URLs on a site using the
`/%category%/%postname%/` permalink structure, so review the Posts count before deleting.

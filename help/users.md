---
title: Users
group: feature
---
# Users

Users contains staff accounts and public Subscriber accounts. What you can see or change depends
on your own role and the sites you manage.

- [Finding users](#finding-users)
- [Site Users and Subscribers](#user-tabs)
- [Creating and editing accounts](#editing-users)
- [Roles and site access](#site-access)
- [Suspension, 2FA, and deletion](#user-security)
- [Erasing subscriber data](#erase-data)

<a id="finding-users"></a>
## Finding users

Switch between **Site Users** and **Subscribers** with the tabs; each badge shows its total. The
magnifying glass searches display name, username, and email in the active tab and updates the table
as you type. Select **Name**, **Username**, **Email**, or **Role** where available to sort; select
again to reverse the order. Search and sorting are preserved across pagination.

Select an email address to copy it. Domain badges show site memberships; the highlighted badge is
the user's primary site, and a `+` indicates multiple roles on that site. Administrators with the
required access can select a domain badge to switch to that site.

Use row checkboxes or Select All to reveal **Delete Selected**. Accounts that cannot be deleted,
including your own account, protected accounts, and a site's only Site Admin, are not selectable.
Bulk deletion is permanent.

<a id="user-tabs"></a>
## Site Users and Subscribers

**Site Users** are staff with Site Admin, Editor, or Author access. Their rows show role, domains,
status, Edit, and—when permitted—Manage Site Access and Delete. A suspended badge means login is
blocked but their account and content remain stored. An Unassigned badge means a staff account
currently has no site role.

**Subscribers** are public accounts, normally created through `/subscribe`. They do not have staff
admin access. Their rows include Edit, Erase Personal Data, and Delete actions. An Erased badge
means the account has already been anonymized.

<a id="editing-users"></a>
## Creating and editing accounts

Select New User from the Site Users tab and complete:

- **Display Name** — the name presented in the admin and where themes show an author.
- **Username** — 5–15 lowercase letters, numbers, or hyphens; it cannot begin or end with a
  hyphen. It is generated from Display Name until manually edited.
- **Email** — must be a valid, unique address and is used for account communication and sign-in
  recovery.
- **Password** — required for a new account and must be 12–128 characters. A passphrase is fine.
- **Role** — Site Admin, Editor, Author, or Subscriber according to your permission level. For an
  Author, **Can publish own posts** allows direct publishing; when off, their posts go to an Editor
  for review.
- **Site Assignment** — when shown, choose None, an Existing site, or New and enter a valid domain.
  A single-site Site Admin may have new users assigned automatically instead.

On an existing account, change the display name, username, or email and Save. Leave Password blank
to keep the current one, or enter a compliant replacement. The Current Roles table is informational;
use Change Role/Manage Site Access to alter assignments. Super Admin roles are protected and not
editable here.

<a id="site-access"></a>
## Roles and site access

Select the key button from a Site User row or Change Role in the editor. **Current Roles** lists
each site assignment and whether an Author can publish. Remove deletes only that site assignment,
not the user, but SynapCMS blocks removal of a site's only Site Admin.

Under **Site Assignment**, choose a site and role, then Assign. A user may hold more than one role
on the same site; assigning a role they already hold refreshes that role's settings rather than
creating a duplicate. When selecting Author, decide whether they may publish their own posts.

When assigning Site Admin to a site that already has an owner, choose carefully:

- **Add as an additional Site Admin** keeps the existing administrator and ownership unchanged.
- **Remove from site** immediately removes the existing administrator's access and transfers
  ownership.
- **Demote to Author, transfer ownership** keeps their author access but transfers ownership.

Changing the current owner to another role also removes their ownership. The application prevents
a demotion or removal that would leave a site with no Site Admin; assign a replacement first.

Role summary: Site Admin manages the site, Editor manages and publishes content, Author manages
their own posts within the configured publishing permission, and Subscriber has public-account
access only. One person can hold roles on multiple sites and, where configured, multiple roles on
the same site.

<a id="user-security"></a>
## Suspension, 2FA, and deletion

- **Account Status** — suspend to block login immediately without deleting content; reactivate to
  restore login. You cannot suspend yourself or a protected account.
- **Two-Factor Authentication** — a Super Admin can disable another user's enabled 2FA as an
  account-recovery action. This lets the user sign in with only their password again; confirm their
  identity before doing it. Users normally manage their own 2FA under Profile. See
  [Two-Factor Authentication](/help#doc-two-factor-authentication).
- **Delete** — permanently removes an account and reassigns its posts and media to the
  administrator performing the deletion. Your own account and protected accounts cannot be
  deleted, and the sole Site Admin for any site must be replaced first.

Suspension is normally the safer reversible choice when access should stop but content or records
must remain.

<a id="erase-data"></a>
## Erasing subscriber data

Erase Personal Data is a GDPR-oriented anonymization workflow for Subscribers. The review page
explains the effects and finds form submissions and mail-log entries containing the subscriber's
email. Those matches start selected, but review each one before erasing: a form submission may be
a business record or may only quote another person's address.

Confirmation replaces the account's username, email, display name, bio, and avatar with anonymous
placeholders; deactivates the account; resets its password; anonymizes the identity and stored IP
on their comments while keeping comment text; and deletes saved posts and pending password-reset
tokens. Selected matching form submissions and mail-log records are also removed. This cannot be
undone and is different from ordinary account deletion.

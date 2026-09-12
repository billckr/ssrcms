---
title: Two-Factor Authentication
group: feature
---
# Two-Factor Authentication

Two-factor authentication (2FA) adds a second step to signing in. After your password, you'll
also enter a 6-digit code from an authenticator app on your phone. If someone ever gets hold of
your password, they still can't sign in without that second code.

This is available for staff accounts (Super Admin, Site Admin, Editor, Author) signing in at
`/admin/login`. Subscriber accounts don't use it.

- [Setting it up](#setting-it-up)
- [Recovery codes](#recovery-codes)
- [Signing in](#signing-in)
- [Managing 2FA](#managing-2fa)

<a id="setting-it-up"></a>
## Setting it up

Go to **Profile** in the sidebar. If 2FA isn't set up yet, you'll see this:

![Two-factor authentication not enabled](/admin/static/help-images/2fa-not-enabled.png)

Click the shield icon to start. You'll be asked to confirm your current password first:

![Confirm your password to begin setup](/admin/static/help-images/2fa-confirm-password.png)

Next, scan the QR code with an authenticator app — Google Authenticator, Authy, 1Password, and
Bitwarden all work. No app on hand yet? Any of those will do; install one from your phone's app
store first. Can't scan a QR code from your computer's screen? Enter the code shown underneath it
into the app manually instead.

![Scan the QR code or enter the secret manually](/admin/static/help-images/2fa-setup-qr.png)

Once the app is set up, it starts showing you a new 6-digit code every 30 seconds. Type the
current one into the **Code** field and confirm.

<a id="recovery-codes"></a>
## Recovery codes

As soon as setup is confirmed, you'll see ten recovery codes:

![Ten one-time recovery codes](/admin/static/help-images/2fa-recovery-codes.png)

**Save these somewhere safe before leaving this page — they're shown exactly once.** Each code
works one time only, as a stand-in for your authenticator app. They're for the day you lose your
phone, reset it, or otherwise lose access to the app: sign in with your password as usual, and
enter a recovery code instead of a 6-digit code when asked for your second factor.

Use the download button to save them straight to a file, or copy them into a password manager.
Either way, keep them somewhere other than the phone your authenticator app lives on — if that
phone is what you lose, codes stored only on it won't help.

<a id="signing-in"></a>
## Signing in

Once 2FA is on, signing in takes one extra step: enter your password as usual, then you'll land on
a second screen asking for a code. Enter the current code from your authenticator app, or one of
your recovery codes if you don't have the app handy. Wrong codes are rate-limited, so don't worry
about mistyping — just try again.

<a id="managing-2fa"></a>
## Managing 2FA

Once enabled, the Profile card shows your status and how many recovery codes you have left:

![Two-factor authentication enabled, with management options](/admin/static/help-images/2fa-enabled.png)

- **Regenerate recovery codes** — invalidates every existing code and issues ten new ones. Do this
  if you're running low, or if you're at all worried an old code leaked.
- **Disable two-factor authentication** — turns it off entirely and removes your recovery codes.
  You'll need your current password for either action.

Enabling, disabling, or regenerating codes each send a confirmation email to your account —
so if any of that ever happens without you doing it, you'll know right away.

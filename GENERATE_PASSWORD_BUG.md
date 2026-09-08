# Bug: CLI-generated admin password no longer meets the app's own password policy

Status: Resolved on 2026-09-07.

Resolution: the CLI generator now emits 16-character passwords, the non-interactive
call site validates generated output before storing it, and a regression test generates
100 passwords and verifies their length and compliance with the current policy.

Date found: 2026-09-07
Found while: reviewing `AUTH_SECURITY_REVIEW.md` / `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md`
against the actual code (not something either of those documents mentions correctly).

## Summary

Before resolution, `synap install --non-interactive` could generate a 10-character admin password that failed
the application's own 12-character minimum password rule, because the code path that
generates it is a separate, un-synced duplicate of the generator that was correctly fixed
elsewhere, and the generated value is never re-validated before use.

## Background: two separate `generate_password()` implementations

There is no shared password-generation function. Two independent copies exist:

1. `core/src/models/user.rs` (~line 62) — used by WP-import and the dev-tools seeding
   endpoint.
2. `cli/src/commands/install.rs` (~line 1329) — used only by `synap install
   --non-interactive` when `ADMIN_PASSWORD` is not supplied.

As part of the 2026-09-07 authentication security hardening, `validate_password()` was
changed everywhere (three separate copies: `core/src/models/user.rs`,
`cli/src/commands/user.rs`, `cli/src/commands/install.rs`) from:

- old: 8–12 characters, must include ≥1 uppercase letter, ≥1 digit, ≥1 symbol
- new: 12–128 Unicode characters, no composition requirements

Generator #1 (`core::models::user::generate_password`) was correctly widened from 8 to 16
characters to match. At discovery, generator #2 (`cli::commands::install::generate_password`) was **not**
— only a doc comment above it was edited, not the function body.

## The original unfixed function

`cli/src/commands/install.rs`, function `generate_password()` (~line 1329):

```rust
/// Generate a password that satisfies validate_password():
/// 12-128 Unicode scalar values; passphrases are supported.   <-- comment WAS updated
fn generate_password() -> String {
    use rand::seq::SliceRandom;
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let lower:   Vec<char> = ('a'..='z').collect();
    let upper:   Vec<char> = ('A'..='Z').collect();
    let digits:  Vec<char> = ('0'..='9').collect();
    let symbols: &[char]   = &['@', '#', '%', '&'];

    // Guarantee one of each required class within the 10-char budget.   <-- body NOT updated
    let mut chars: Vec<char> = Vec::with_capacity(10);
    chars.push(upper[rng.gen_range(0..upper.len())]);
    chars.push(digits[rng.gen_range(0..digits.len())]);
    chars.push(symbols[rng.gen_range(0..symbols.len())]);
    // Fill remaining 7 slots with lowercase.
    for _ in 0..7 {
        chars.push(lower[rng.gen_range(0..lower.len())]);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}
```

Output length: `1 (uppercase) + 1 (digit) + 1 (symbol) + 7 (lowercase) = 10` characters.
`validate_password()` now requires a minimum of **12**.

The `git diff` for this file confirms only the doc comment changed — the "10-char budget"
comment and the `Vec::with_capacity(10)` / `0..7` loop bound are untouched from before the
security pass. It looks like updating the comment was mistaken for actually fixing the
generator.

For comparison, the sibling function that *was* fixed correctly,
`core/src/models/user.rs` (~line 62):

```rust
/// Generate a 16-character password with mixed character classes.
pub fn generate_password() -> String {
    ...
    let mut chars: Vec<char> = Vec::with_capacity(16);
    chars.push((lower[rng.gen_range(0..lower.len())] as char).to_ascii_uppercase());
    chars.push(char::from_digit(rng.gen_range(0..10), 10).unwrap());
    chars.push(symbols[rng.gen_range(0..symbols.len())] as char);
    for _ in 0..13 {
        chars.push(lower[rng.gen_range(0..lower.len())] as char);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}
```

`1 + 1 + 1 + 13 = 16` characters — correctly widened from the old 8-char version (`3 fixed +
5 lowercase`).

## Why the mismatch was not caught

Call site in `cli/src/commands/install.rs`, inside the non-interactive admin-password
branch (~line 616–628):

```rust
let password = if ni {
    match args.admin_password.clone() {
        Some(pw) => {
            validate_password(&pw)
                .map_err(|e| anyhow::anyhow!("Provided ADMIN_PASSWORD is invalid: {e}"))?;
            pw
        }
        None => {
            let pw = generate_password();
            println!("GENERATED_ADMIN_PASSWORD={pw}");
            println!("IMPORTANT: Save this password — it will not be shown again.");
            pw
        }
    }
} else {
    ...
};
```

- If the operator supplies `ADMIN_PASSWORD`, it goes through `validate_password()` and would
  be rejected if too short.
- If the operator omits it, `generate_password()` is called and its output is used directly
  — **no call to `validate_password()` on this branch at all.**

So the one branch most likely to violate the policy is exactly the one branch that skips the
check.

## Practical impact before resolution

- `synap install --non-interactive` without `ADMIN_PASSWORD` set produces a working,
  Argon2-hashed 10-character super_admin password. Login itself is unaffected — Argon2
  verification doesn't re-check the app's password-policy rules, and existing/generated
  passwords are never retroactively invalidated for being "too short" (this is explicitly
  documented as intentional elsewhere: existing passwords keep working after the policy
  changed).
- The impact is a **correctness/consistency bug**, not an active exploit: the generated
  password is randomly generated, not predictable, and still reasonably strong for 10 mixed
  characters. The concern is that it contradicts the app's own stated minimum policy on a
  path that's supposed to represent "policy-compliant" credential generation, and any future
  logic that assumes "every account's password satisfies `validate_password()`" would be
  wrong for accounts created this way.

## Resolution applied

The CLI generator was widened to match the 16-character pattern used in
`core/src/models/user.rs` (`Vec::with_capacity(16)` with 13 lowercase fill
characters). Its output is also explicitly passed through `validate_password()` before use,
so a future policy change cannot silently reintroduce this gap. A regression test generates
100 passwords and verifies their length and policy compliance.

## How this was found

Neither `AUTH_SECURITY_REVIEW.md` nor `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md` mentions
this discrepancy — `AUTH_SECURITY_IMPLEMENTATION_RESULTS.md` actually states "Generated
local passwords are now 16 characters instead of eight," which is true for
`core::models::user::generate_password()` but not for this CLI-local copy. This was caught
by diffing `cli/src/commands/install.rs` against its pre-security-pass version while
updating the project's internal documentation (`documentation` DB table, `cli` doc slug) to
match the implementation-results doc, and noticing the function body didn't match the claim.

//! Two-factor authentication enrollment/recovery-code pages under
//! `/admin/profile/2fa/*`. Kept separate from `profile.rs` since the QR
//! setup screen and the one-time recovery-code display are full pages, not
//! the small modal dialogs the rest of that page uses.

/// GET /admin/profile/2fa/setup — QR + manual-entry secret + code-confirm
/// form for a pending (unconfirmed) enrollment.
pub fn render_setup(
    secret_b32: &str,
    qr_svg: Option<&str>,
    error: Option<&str>,
    ctx: &crate::PageContext,
) -> String {
    let error_html = match error {
        Some(msg) => format!(r#"<div class="flash error">{}</div>"#, crate::html_escape(msg)),
        None => String::new(),
    };
    let qr_html = match qr_svg {
        Some(svg) => format!(r#"<div class="mfa-qr">{svg}</div>"#),
        None => String::new(),
    };
    let content = format!(
        r#"<div class="profile-main">
  <div class="profile-bio-card">
    <h3 style="margin-top:0">Set up two-factor authentication</h3>
    {error_html}
    <p>1. Scan this QR code with an authenticator app (Google Authenticator, Authy, 1Password, Bitwarden, etc.):</p>
    {qr_html}
    <p>Or enter this code manually:</p>
    <p class="form-static-value" style="font-family:monospace;letter-spacing:.05em">{secret}</p>
    <p>2. Enter the 6-digit code the app shows for this account to confirm setup:</p>
    <form method="POST" action="/admin/profile/2fa/setup/confirm">
      <div class="form-group">
        <label for="code">Code</label>
        <input type="text" id="code" name="code" inputmode="numeric" autocomplete="one-time-code" required autofocus style="max-width:12rem">
      </div>
      <div class="icon-pill">
        <a href="/admin/profile" class="icon-btn" title="Cancel" aria-label="Cancel">
          <img src="/admin/static/icons/x.svg" alt="">
        </a>
        <button type="submit" class="icon-btn icon-btn-active-blue" title="Confirm" aria-label="Confirm">
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
    </form>
  </div>
</div>"#,
        error_html = error_html,
        qr_html = qr_html,
        secret = crate::html_escape(secret_b32),
    );
    crate::admin_page("Two-Factor Authentication", "/admin/profile", None, &content, ctx)
}

/// Shown exactly once, right after enrollment is confirmed or recovery
/// codes are regenerated — the only time these codes are ever visible in
/// plaintext.
pub fn render_recovery_codes(codes: &[String], heading: &str, ctx: &crate::PageContext) -> String {
    let codes_html = codes
        .iter()
        .map(|c| format!("<li>{}</li>", crate::html_escape(c)))
        .collect::<Vec<_>>()
        .join("\n");
    // Codes only ever contain `mfa_recovery_code`'s fixed charset
    // (uppercase letters/digits and a dash) — safe to splice directly into
    // a JS string-array literal with no further escaping.
    let codes_js = codes
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(",");
    let content = format!(
        r#"<div class="profile-main">
  <div class="profile-bio-card">
    <h3 style="margin-top:0">{heading}</h3>
    <p>Save these recovery codes somewhere safe. Each one can be used once, in place of a code from your authenticator app, if you lose access to it. They will not be shown again.</p>
    <ul style="font-family:monospace;line-height:1.8;list-style:none;padding-left:0">
      {codes_html}
    </ul>
    <div class="icon-pill">
      <button type="button" class="icon-btn" title="Download codes" aria-label="Download codes" onclick="downloadMfaRecoveryCodes()">
        <img src="/admin/static/icons/download.svg" alt="">
      </button>
      <a href="/admin/profile" class="icon-btn icon-btn-active-blue" title="Done" aria-label="Done">
        <img src="/admin/static/icons/save.svg" alt="">
      </a>
    </div>
  </div>
</div>
<script>
function downloadMfaRecoveryCodes() {{
  var codes = [{codes_js}];
  var text = "SynapCMS two-factor recovery codes\n"
    + "Each code can be used once, in place of your authenticator app.\n\n"
    + codes.join("\n") + "\n";
  var blob = new Blob([text], {{ type: "text/plain" }});
  var url = URL.createObjectURL(blob);
  var a = document.createElement("a");
  a.href = url;
  a.download = "synapcms-recovery-codes.txt";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}}
</script>"#,
        heading = crate::html_escape(heading),
        codes_html = codes_html,
        codes_js = codes_js,
    );
    crate::admin_page("Two-Factor Authentication", "/admin/profile", None, &content, ctx)
}

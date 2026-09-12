//! Admin user profile page — for the logged-in user to update their own info.

pub struct ProfileForm {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub bio: String,
    pub mfa_enabled: bool,
    /// Formatted enable date, when `mfa_enabled` is true.
    pub mfa_enabled_at: Option<String>,
    pub mfa_recovery_codes_remaining: i64,
}

/// Up to two uppercase initials, preferring the display name over the username.
fn initials(display_name: &str, username: &str) -> String {
    let source = if display_name.trim().is_empty() {
        username
    } else {
        display_name
    };
    let letters: String = source
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect();
    if letters.is_empty() {
        "?".to_string()
    } else {
        letters
    }
}

/// Escaped bio, or a muted placeholder line when the user hasn't written one.
fn display_or_placeholder(value: &str) -> String {
    if value.trim().is_empty() {
        r#"<span class="profile-summary-empty">&quot;The future has yet to be written...&quot;</span>"#.to_string()
    } else {
        // Trim before quoting — .profile-bio uses white-space: pre-wrap, so
        // a trailing newline from the textarea (very easy to leave in,
        // e.g. hitting Enter at the end) would otherwise push the closing
        // quote onto its own line.
        format!("&quot;{}&quot;", crate::html_escape(value.trim()))
    }
}

fn mfa_card_html(profile: &ProfileForm) -> String {
    if profile.mfa_enabled {
        format!(
            r#"<div class="profile-bio-card">
    <h3 style="margin-top:0">Two-factor authentication <a href="/help#doc-two-factor-authentication" target="_blank" title="Help: Two-Factor Authentication" aria-label="Help: Two-Factor Authentication" style="display:inline-flex;vertical-align:middle;opacity:.55"><img src="/admin/static/icons/help-circle.svg" alt="" style="width:14px;height:14px"></a></h3>
    <p>Enabled{enabled_at}. {remaining} of 10 recovery codes remaining.</p>
    <div class="icon-pill">
      <button type="button" class="icon-btn" title="Regenerate recovery codes" aria-label="Regenerate recovery codes"
              onclick="document.getElementById('mfa-regenerate-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
        <img src="/admin/static/icons/refresh-cw.svg" alt="">
      </button>
      <button type="button" class="icon-btn" title="Disable two-factor authentication" aria-label="Disable two-factor authentication"
              onclick="document.getElementById('mfa-disable-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
        <img src="/admin/static/icons/shield-off.svg" alt="">
      </button>
    </div>
  </div>"#,
            enabled_at = profile
                .mfa_enabled_at
                .as_deref()
                .map(|d| format!(" on {}", crate::html_escape(d)))
                .unwrap_or_default(),
            remaining = profile.mfa_recovery_codes_remaining,
        )
    } else {
        r#"<div class="profile-bio-card">
    <h3 style="margin-top:0">Two-factor authentication <a href="/help#doc-two-factor-authentication" target="_blank" title="Help: Two-Factor Authentication" aria-label="Help: Two-Factor Authentication" style="display:inline-flex;vertical-align:middle;opacity:.55"><img src="/admin/static/icons/help-circle.svg" alt="" style="width:14px;height:14px"></a></h3>
    <p>Not enabled. Add an authenticator app (Google Authenticator, Authy, 1Password, etc.) as a second sign-in step.</p>
    <div class="icon-pill">
      <button type="button" class="icon-btn" title="Set up two-factor authentication" aria-label="Set up two-factor authentication"
              onclick="document.getElementById('mfa-setup-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
        <img src="/admin/static/icons/shield.svg" alt="">
      </button>
    </div>
  </div>"#
        .to_string()
    }
}

/// Each of setup/disable/regenerate needs a fresh current-password
/// confirmation — same sensitivity bar as `change_password` — before its
/// respective POST. One small dialog shape shared by all three.
fn mfa_password_confirm_dialog(id: &str, title: &str, action: &str) -> String {
    format!(
        r#"<dialog id="{id}" class="modal-card">
  <form method="POST" action="{action}">
    <h3 class="modal-card-header">{title}</h3>
    <div class="modal-card-body">
      <div class="form-group">
        <label for="{id}-password">Current Password</label>
        <input type="password" id="{id}-password" name="current_password" required>
      </div>
      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('{id}').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn icon-btn-active-blue" title="Continue" aria-label="Continue">
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>
<script>
document.getElementById('{id}').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
</script>"#
    )
}

pub fn render_profile(
    profile: &ProfileForm,
    flash: Option<&str>,
    ctx: &crate::PageContext,
) -> String {
    let mfa_card = mfa_card_html(profile);
    let mfa_setup_dialog = mfa_password_confirm_dialog(
        "mfa-setup-dialog",
        "Set Up Two-Factor Authentication",
        "/admin/profile/2fa/setup/start",
    );
    let mfa_disable_dialog = mfa_password_confirm_dialog(
        "mfa-disable-dialog",
        "Disable Two-Factor Authentication",
        "/admin/profile/2fa/disable",
    );
    let mfa_regenerate_dialog = mfa_password_confirm_dialog(
        "mfa-regenerate-dialog",
        "Regenerate Recovery Codes",
        "/admin/profile/2fa/recovery-codes/regenerate",
    );
    let content = format!(
        r#"<div class="profile-layout">
  <div class="profile-main">
    {mfa_card}
  </div>

  <div class="profile-side">
    <div class="profile-avatar-card">
      <div class="profile-avatar" aria-hidden="true">{initials}</div>
      <div class="profile-avatar-name">{display_name_or_username}</div>
      <div class="profile-avatar-email">{email}</div>
      <div class="icon-pill profile-avatar-btn">
        <button type="button" class="icon-btn" disabled title="Change photo (coming soon)" aria-label="Change photo">
          <img src="/admin/static/icons/camera.svg" alt="">
        </button>
        <button type="button" class="icon-btn" title="Edit Profile" aria-label="Edit Profile"
                onclick="document.getElementById('edit-profile-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
          <img src="/admin/static/icons/fingerprint-light.svg" alt="">
        </button>
        <button type="button" class="icon-btn" title="Change password" aria-label="Change password"
                onclick="document.getElementById('change-password-dialog').showModal();document.querySelector('.admin-content').style.filter='blur(1.5px)'">
          <img src="/admin/static/icons/key.svg" alt="">
        </button>
        <button type="button" class="icon-btn" title="Sign out other devices" aria-label="Sign out other devices"
                onclick="if (confirm('Sign out of every other session? You will stay signed in here.')) document.getElementById('sign-out-other-devices-form').submit();">
          <img src="/admin/static/icons/log-out.svg" alt="">
        </button>
      </div>
      <p class="profile-avatar-hint">Custom avatars aren't supported yet — this is a placeholder.</p>
      <form id="sign-out-other-devices-form" method="POST" action="/admin/profile/sign-out-other-devices" hidden></form>
    </div>

    <div class="profile-bio-card">
      <p class="profile-bio">{bio_shown}</p>
    </div>
  </div>
</div>

<dialog id="edit-profile-dialog" class="modal-card">
  <form method="POST" action="/admin/profile/update">
    <h3 class="modal-card-header">Edit Profile</h3>
    <div class="modal-card-body">
      <div class="form-group">
        <label>Username</label>
        <p class="form-static-value">{username}</p>
        <small>Username cannot be changed.</small>
      </div>

      <div class="form-group">
        <label for="email">Email</label>
        <input type="email" id="email" name="email" value="{email}" readonly>
        <small>Contact an administrator to change your sign-in email.</small>
      </div>

      <div class="form-group">
        <label for="display_name">Display Name</label>
        <input type="text" id="display_name" name="display_name" value="{display_name}">
      </div>

      <div class="form-group">
        <label for="bio">Bio</label>
        <textarea id="bio" name="bio" rows="4">{bio}</textarea>
      </div>

      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('edit-profile-dialog').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn" title="Update Profile" aria-label="Update Profile" id="edit-profile-save-btn" disabled>
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>

<dialog id="change-password-dialog" class="modal-card">
  <form method="POST" action="/admin/profile/change-password" id="change-password-form" novalidate>
    <h3 class="modal-card-header">Change Password</h3>
    <div class="modal-card-body">
      <div class="form-group">
        <label for="current_password">Current Password</label>
        <input type="password" id="current_password" name="current_password" required>
      </div>

      <div class="form-group">
        <label for="new_password">New Password</label>
        <input type="password" id="new_password" name="new_password" required minlength="12" maxlength="128">
      </div>

      <div class="form-group">
        <label for="confirm_password">Confirm New Password</label>
        <input type="password" id="confirm_password" name="confirm_password" required minlength="12" maxlength="128">
      </div>

      <div class="form-note">
        <p><strong>Password requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="np-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>12–128 characters; passphrases are welcome</li>
          <li id="np-req-match"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>Passwords match</li>
        </ul>
      </div>

      <p id="change-password-error" class="profile-form-error" hidden></p>

      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('change-password-dialog').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn" title="Change Password" aria-label="Change Password" id="change-password-save-btn" disabled>
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>

{mfa_setup_dialog}
{mfa_disable_dialog}
{mfa_regenerate_dialog}

<script>
document.getElementById('edit-profile-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});
document.getElementById('change-password-dialog').addEventListener('close', function() {{
  document.querySelector('.admin-content').style.filter = '';
}});

(function() {{
  var emailInput = document.getElementById('email');
  var displayNameInput = document.getElementById('display_name');
  var bioInput = document.getElementById('bio');
  var saveBtn = document.getElementById('edit-profile-save-btn');

  var original = {{
    email: emailInput.value,
    display_name: displayNameInput.value,
    bio: bioInput.value,
  }};

  var syncSaveBtn = function() {{
    var changed = emailInput.value !== original.email
      || displayNameInput.value !== original.display_name
      || bioInput.value !== original.bio;
    var active = changed && emailInput.checkValidity();
    saveBtn.disabled = !active;
    saveBtn.classList.toggle('icon-btn-active-blue', active);
  }};

  [emailInput, displayNameInput, bioInput].forEach(function(el) {{
    el.addEventListener('input', syncSaveBtn);
  }});
}})();

(function() {{
  var currentPwInput = document.getElementById('current_password');
  var newPwInput = document.getElementById('new_password');
  var confirmPwInput = document.getElementById('confirm_password');
  var saveBtn = document.getElementById('change-password-save-btn');

  var npReqs = [
    {{ id: 'np-req-len', test: function(p) {{ return Array.from(p).length >= 12 && Array.from(p).length <= 128; }} }},
  ];

  var updateFeedback = function() {{
    var errorEl = document.getElementById('change-password-error');
    if (errorEl) errorEl.hidden = true;

    var pw = newPwInput ? newPwInput.value : '';
    npReqs.forEach(function(req) {{
      var li = document.getElementById(req.id);
      var dot = li ? li.querySelector('.pw-dot') : null;
      if (!li) return;
      if (!pw) {{
        li.style.color = ''; if (dot) dot.textContent = '·';
      }} else if (req.test(pw)) {{
        li.style.color = '#16a34a'; if (dot) dot.textContent = '✓';
      }} else {{
        li.style.color = '#dc2626'; if (dot) dot.textContent = '✗';
      }}
    }});

    var matchLi = document.getElementById('np-req-match');
    var matchDot = matchLi ? matchLi.querySelector('.pw-dot') : null;
    var confirmPw = confirmPwInput ? confirmPwInput.value : '';
    var matches = !!pw && pw === confirmPw;
    if (matchLi) {{
      if (!pw && !confirmPw) {{
        matchLi.style.color = ''; if (matchDot) matchDot.textContent = '·';
      }} else if (matches) {{
        matchLi.style.color = '#16a34a'; if (matchDot) matchDot.textContent = '✓';
      }} else {{
        matchLi.style.color = '#dc2626'; if (matchDot) matchDot.textContent = '✗';
      }}
    }}

    var meetsAllReqs = npReqs.every(function(req) {{ return req.test(pw); }});
    var currentPw = currentPwInput ? currentPwInput.value : '';
    var active = !!(currentPw && meetsAllReqs && matches);
    if (saveBtn) {{
      saveBtn.disabled = !active;
      saveBtn.classList.toggle('icon-btn-active-blue', active);
    }}
  }};

  if (currentPwInput) currentPwInput.addEventListener('input', updateFeedback);
  if (newPwInput) newPwInput.addEventListener('input', updateFeedback);
  if (confirmPwInput) confirmPwInput.addEventListener('input', updateFeedback);

  document.getElementById('change-password-form').addEventListener('submit', function(e) {{
    var newPw = newPwInput.value;
    var confirmPw = confirmPwInput.value;
    var errorEl = document.getElementById('change-password-error');
    var errors = [];

    if (Array.from(newPw).length < 12 || Array.from(newPw).length > 128) {{
      errors.push('Password must be 12-128 characters.');
    }}
    if (newPw !== confirmPw) {{
      errors.push('New passwords do not match.');
    }}

    if (errors.length > 0) {{
      e.preventDefault();
      errorEl.textContent = errors[0];
      errorEl.hidden = false;
    }} else {{
      errorEl.hidden = true;
    }}
  }});
}})();
</script>"#,
        mfa_card = mfa_card,
        mfa_setup_dialog = mfa_setup_dialog,
        mfa_disable_dialog = mfa_disable_dialog,
        mfa_regenerate_dialog = mfa_regenerate_dialog,
        username = crate::html_escape(&profile.username),
        email = crate::html_escape(&profile.email),
        display_name = crate::html_escape(&profile.display_name),
        bio = crate::html_escape(&profile.bio),
        bio_shown = display_or_placeholder(&profile.bio),
        initials = crate::html_escape(&initials(&profile.display_name, &profile.username)),
        display_name_or_username = crate::html_escape(if profile.display_name.trim().is_empty() {
            &profile.username
        } else {
            &profile.display_name
        }),
    );

    crate::admin_page("Profile Management", "/admin/profile", flash, &content, ctx)
}

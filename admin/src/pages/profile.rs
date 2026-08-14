//! Admin user profile page — for the logged-in user to update their own info.

pub struct ProfileForm {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub bio: String,
}

/// Up to two uppercase initials, preferring the display name over the username.
fn initials(display_name: &str, username: &str) -> String {
    let source = if display_name.trim().is_empty() { username } else { display_name };
    let letters: String = source
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect();
    if letters.is_empty() { "?".to_string() } else { letters }
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

pub fn render_profile(profile: &ProfileForm, flash: Option<&str>, ctx: &crate::PageContext) -> String {
    let content = format!(
        r#"<div class="profile-layout">
  <div class="profile-main">
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
      </div>
      <p class="profile-avatar-hint">Custom avatars aren't supported yet — this is a placeholder.</p>
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
        <input type="email" id="email" name="email" value="{email}" required>
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
        <input type="password" id="new_password" name="new_password" required minlength="8" maxlength="12">
      </div>

      <div class="form-group">
        <label for="confirm_password">Confirm New Password</label>
        <input type="password" id="confirm_password" name="confirm_password" required minlength="8" maxlength="12">
      </div>

      <div class="form-note">
        <p><strong>Password requirements:</strong></p>
        <ul style="list-style:none;padding-left:0;margin:0.25rem 0 0">
          <li id="np-req-len"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>8–12 characters</li>
          <li id="np-req-upper"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one uppercase letter</li>
          <li id="np-req-num"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one number</li>
          <li id="np-req-sym"><span class="pw-dot" style="display:inline-block;width:1.1rem;font-style:normal">&middot;</span>At least one symbol: ! @ # $ % &amp;</li>
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
    {{ id: 'np-req-len',   test: function(p) {{ return p.length >= 8 && p.length <= 12; }} }},
    {{ id: 'np-req-upper', test: function(p) {{ return /[A-Z]/.test(p); }} }},
    {{ id: 'np-req-num',   test: function(p) {{ return /[0-9]/.test(p); }} }},
    {{ id: 'np-req-sym',   test: function(p) {{ return /[!@#$%&]/.test(p); }} }},
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

    if (newPw.length < 8 || newPw.length > 12) {{
      errors.push('Password must be 8-12 characters.');
    }}
    if (!/[A-Z]/.test(newPw)) {{
      errors.push('Password must contain at least one uppercase letter.');
    }}
    if (!/[0-9]/.test(newPw)) {{
      errors.push('Password must contain at least one number.');
    }}
    if (!/[!@#$%&]/.test(newPw)) {{
      errors.push('Password must contain at least one symbol: ! @ # $ % &');
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
        username = crate::html_escape(&profile.username),
        email = crate::html_escape(&profile.email),
        display_name = crate::html_escape(&profile.display_name),
        bio = crate::html_escape(&profile.bio),
        bio_shown = display_or_placeholder(&profile.bio),
        initials = crate::html_escape(&initials(&profile.display_name, &profile.username)),
        display_name_or_username = crate::html_escape(
            if profile.display_name.trim().is_empty() { &profile.username } else { &profile.display_name }
        ),
    );

    crate::admin_page("Profile Management", "/admin/profile", flash, &content, ctx)
}

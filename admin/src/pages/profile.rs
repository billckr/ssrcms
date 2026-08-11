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

/// Escaped value for display, or a muted em-dash placeholder when empty.
fn display_or_placeholder(value: &str) -> String {
    if value.trim().is_empty() {
        r#"<span class="profile-summary-empty">&mdash;</span>"#.to_string()
    } else {
        crate::html_escape(value)
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
          <img src="/admin/static/icons/edit-2.svg" alt="">
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
        <button type="submit" class="icon-btn" title="Update Profile" aria-label="Update Profile">
          <img src="/admin/static/icons/save.svg" alt="">
        </button>
      </div>
      </div>
    </div>
  </form>
</dialog>

<dialog id="change-password-dialog" class="modal-card">
  <form method="POST" action="/admin/profile/change-password">
    <h3 class="modal-card-header">Change Password</h3>
    <div class="modal-card-body">
      <div class="form-group">
        <label for="current_password">Current Password</label>
        <input type="password" id="current_password" name="current_password" required>
      </div>

      <div class="form-group">
        <label for="new_password">New Password</label>
        <input type="password" id="new_password" name="new_password" required>
      </div>

      <div class="form-group">
        <label for="confirm_password">Confirm New Password</label>
        <input type="password" id="confirm_password" name="confirm_password" required>
      </div>

      <div class="form-note">
        <p><strong>Password requirements:</strong></p>
        <ul>
          <li>8–12 characters</li>
          <li>At least one uppercase letter</li>
          <li>At least one number</li>
          <li>At least one symbol: ! @ # $ % &amp;</li>
        </ul>
      </div>

      <div style="display:flex;justify-content:flex-end;margin-top:1rem">
      <div class="icon-pill">
        <button type="button" class="icon-btn" title="Cancel" aria-label="Cancel" onclick="document.getElementById('change-password-dialog').close()">
          <img src="/admin/static/icons/x.svg" alt="">
        </button>
        <button type="submit" class="icon-btn" title="Change Password" aria-label="Change Password">
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

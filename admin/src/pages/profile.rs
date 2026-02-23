//! Admin user profile page — for the logged-in user to update their own info.

pub struct ProfileForm {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub bio: String,
}

pub fn render_profile(profile: &ProfileForm, flash: Option<&str>, current_site: &str, is_global_admin: bool, user_email: &str) -> String {
    let content = format!(
        r#"<div class="profile-container">
  <h2>My Profile</h2>
  
  <form method="POST" action="/admin/profile/update" class="profile-form">
    <fieldset>
      <legend>Profile Information</legend>
      
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
    </fieldset>
    
    <button type="submit" class="btn btn-primary">Update Profile</button>
  </form>
</div>

<div class="profile-container">
  <h2>Change Password</h2>
  
  <form method="POST" action="/admin/profile/change-password" class="password-form">
    <fieldset>
      <legend>Password Management</legend>
      
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
          <li>At least 8 characters</li>
          <li>Mix of uppercase and lowercase letters</li>
          <li>At least one number</li>
        </ul>
      </div>
    </fieldset>
    
    <button type="submit" class="btn btn-primary">Change Password</button>
  </form>
</div>"#,
        username = crate::html_escape(&profile.username),
        email = crate::html_escape(&profile.email),
        display_name = crate::html_escape(&profile.display_name),
        bio = crate::html_escape(&profile.bio),
    );

    crate::admin_page("My Profile", "/admin/profile", flash, &content, current_site, is_global_admin, user_email)
}

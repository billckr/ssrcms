CREATE TABLE user_totp (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret_encrypted TEXT NOT NULL,
    enabled_at TIMESTAMPTZ,
    last_used_step BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE user_totp IS 'One row per staff user who has started or completed TOTP MFA enrollment. enabled_at IS NULL means the secret was generated (QR shown) but never confirmed with a correct code.';
COMMENT ON COLUMN user_totp.secret_encrypted IS 'Base32 TOTP secret, AES-256-GCM encrypted via core::crypto (keyed off SECRET_KEY) — same pattern as email_providers API keys.';
COMMENT ON COLUMN user_totp.last_used_step IS 'The RFC 6238 time-step last accepted at login, so the same code cannot be replayed within its own validity window.';

CREATE TABLE mfa_recovery_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mfa_recovery_codes_user_id ON mfa_recovery_codes(user_id);

COMMENT ON TABLE mfa_recovery_codes IS 'Single-use TOTP recovery codes, hashed (SHA-256) same as password_resets.token_hash — never stored in plaintext. 10 issued at a time, replaced wholesale on regeneration.';

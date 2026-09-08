ALTER TABLE public.users
    ADD COLUMN session_nonce uuid DEFAULT gen_random_uuid() NOT NULL;

COMMENT ON COLUMN public.users.session_nonce IS 'Regenerated to a fresh random value to invalidate every other session ("sign out other devices") without touching password_hash or email.';

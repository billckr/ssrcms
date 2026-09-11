CREATE TABLE form_translations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    form_id UUID NOT NULL REFERENCES forms(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    payload JSONB NOT NULL,
    source_updated_at TIMESTAMPTZ NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (form_id, locale)
);

CREATE INDEX idx_form_translations_form ON form_translations(form_id);

CREATE TABLE poll_translations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id UUID NOT NULL REFERENCES polls(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    payload JSONB NOT NULL,
    source_updated_at TIMESTAMPTZ NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (poll_id, locale)
);

CREATE INDEX idx_poll_translations_poll ON poll_translations(poll_id);

CREATE TABLE public.ai_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    site_id uuid NOT NULL REFERENCES public.sites(id) ON DELETE CASCADE,
    provider_type text NOT NULL,
    label text NOT NULL,
    config_encrypted text NOT NULL,
    verified boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ai_providers_provider_type_check CHECK (provider_type = ANY (ARRAY['anthropic'::text, 'openai_compatible'::text]))
);
CREATE INDEX ai_providers_site_id_idx ON public.ai_providers USING btree (site_id);

CREATE TABLE public.post_translations (
    id uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    post_id uuid NOT NULL REFERENCES public.posts(id) ON DELETE CASCADE,
    locale text NOT NULL,
    title text NOT NULL,
    excerpt text,
    content text NOT NULL,
    source_updated_at timestamp with time zone NOT NULL,
    generated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT post_translations_post_locale_unique UNIQUE (post_id, locale)
);
CREATE INDEX post_translations_post_id_idx ON public.post_translations USING btree (post_id);

COMMENT ON COLUMN public.post_translations.source_updated_at IS 'posts.updated_at at translation time — used to flag a translation as stale when the source post changes afterward.';

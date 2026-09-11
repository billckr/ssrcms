ALTER TABLE public.ai_providers DROP CONSTRAINT ai_providers_provider_type_check;
ALTER TABLE public.ai_providers ADD CONSTRAINT ai_providers_provider_type_check
    CHECK (provider_type = ANY (ARRAY['anthropic'::text, 'openai_compatible'::text, 'deepseek'::text]));

-- Migration: 0001_baseline
-- Consolidated baseline schema, replacing migrations 0001-0072 (squashed
-- 2026-09-08, pre-release). Generated from a `pg_dump --schema-only` of the
-- database those 72 migrations produced, verified table-for-table,
-- column-for-column, index-for-index, and constraint-for-constraint
-- identical to that source before this file replaced them.
--
-- Two objects present in the old schema are deliberately NOT recreated
-- here:
--   - `tower_sessions` (schema + table): the session store is created and
--     managed by `tower_sessions_sqlx_store`'s own `PostgresStore::migrate()`
--     call at startup (see core/src/main.rs), not by our migrations. A
--     legacy `public.tower_sessions` table from the original 0007 migration
--     (superseded by the crate's own schema-qualified table, 0 rows) is
--     also dropped here as dead weight.
--   - `documentation` (table + its `documentation_id_seq` sequence): this
--     content moved to files under `documentation/` — see the "Move admin
--     documentation from the database to files" commit.
--
-- New migrations resume at 0002 from here.

--
-- Name: app_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.app_settings (
    key text NOT NULL,
    value text DEFAULT ''::text NOT NULL
);

--
-- Name: audit_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_log (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    actor_user_id uuid,
    actor_email text NOT NULL,
    actor_role text NOT NULL,
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    target_label text NOT NULL,
    site_id uuid,
    details jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: builder_projects; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.builder_projects (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name character varying(35) NOT NULL,
    description character varying(100),
    is_active boolean DEFAULT false NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: comments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.comments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    post_id uuid NOT NULL,
    site_id uuid,
    author_id uuid NOT NULL,
    parent_id uuid,
    body text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone,
    ip_address text,
    CONSTRAINT comments_body_check CHECK (((char_length(body) >= 1) AND (char_length(body) <= 400)))
);

--
-- Name: email_changes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.email_changes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    new_email text NOT NULL,
    token_hash text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: email_providers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.email_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    provider_type text NOT NULL,
    label text NOT NULL,
    config_encrypted text NOT NULL,
    verified boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: form_blocks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.form_blocks (
    site_id uuid NOT NULL,
    form_name text NOT NULL,
    blocked_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: form_submissions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.form_submissions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    form_name text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    ip_address text,
    read_at timestamp with time zone,
    submitted_at timestamp with time zone DEFAULT now() NOT NULL,
    form_id uuid
);

--
-- Name: forms; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.forms (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name text NOT NULL,
    slug text NOT NULL,
    fields jsonb DEFAULT '[]'::jsonb NOT NULL,
    settings jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    email_provider_id uuid,
    total_submissions bigint DEFAULT 0 NOT NULL
);

--
-- Name: mail_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.mail_log (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    to_email text NOT NULL,
    subject text NOT NULL,
    success boolean NOT NULL,
    mailgun_message_id text,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    form_id uuid
);

--
-- Name: media; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.media (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    filename text NOT NULL,
    mime_type text NOT NULL,
    path text NOT NULL,
    alt_text text DEFAULT ''::text NOT NULL,
    width integer,
    height integer,
    file_size bigint DEFAULT 0 NOT NULL,
    uploaded_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    site_id uuid,
    title text DEFAULT ''::text NOT NULL,
    caption text DEFAULT ''::text NOT NULL,
    folder_id uuid
);

--
-- Name: media_folders; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.media_folders (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: nav_menu_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.nav_menu_items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    menu_id uuid NOT NULL,
    parent_id uuid,
    sort_order integer DEFAULT 0 NOT NULL,
    label text NOT NULL,
    url text,
    page_id uuid,
    target text DEFAULT '_self'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: nav_menus; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.nav_menus (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name text NOT NULL,
    location text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: page_compositions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.page_compositions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    composition jsonb DEFAULT '{}'::jsonb NOT NULL,
    is_homepage boolean DEFAULT false NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    project_id uuid,
    slug character varying(100),
    page_type character varying(20) DEFAULT 'page'::character varying NOT NULL,
    draft_composition jsonb DEFAULT '{}'::jsonb NOT NULL
);

--
-- Name: password_resets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.password_resets (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    token_hash text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: poll_votes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.poll_votes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    poll_id uuid NOT NULL,
    site_id uuid NOT NULL,
    option_key text NOT NULL,
    voter_token text NOT NULL,
    ip_address text,
    voted_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: polls; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.polls (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    name text NOT NULL,
    slug text NOT NULL,
    question text NOT NULL,
    options jsonb DEFAULT '[]'::jsonb NOT NULL,
    settings jsonb DEFAULT '{}'::jsonb NOT NULL,
    total_votes bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: post_meta; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.post_meta (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    post_id uuid NOT NULL,
    meta_key text NOT NULL,
    meta_value text DEFAULT ''::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: post_taxonomies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.post_taxonomies (
    post_id uuid NOT NULL,
    taxonomy_id uuid NOT NULL
);

--
-- Name: post_views; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.post_views (
    post_id uuid NOT NULL,
    ip_hash text NOT NULL,
    viewed_date date DEFAULT CURRENT_DATE NOT NULL
);

--
-- Name: posts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.posts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    title text NOT NULL,
    slug text NOT NULL,
    content text DEFAULT ''::text NOT NULL,
    content_format text DEFAULT 'html'::text NOT NULL,
    excerpt text,
    status text DEFAULT 'draft'::text NOT NULL,
    post_type text DEFAULT 'post'::text NOT NULL,
    author_id uuid NOT NULL,
    featured_image_id uuid,
    published_at timestamp with time zone,
    scheduled_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    site_id uuid,
    template text,
    post_password text,
    submitted_at timestamp with time zone,
    comments_enabled boolean DEFAULT false NOT NULL,
    parent_id uuid,
    sources jsonb DEFAULT '[]'::jsonb NOT NULL,
    sources_public boolean DEFAULT false NOT NULL,
    CONSTRAINT posts_content_format_check CHECK ((content_format = ANY (ARRAY['html'::text, 'markdown'::text]))),
    CONSTRAINT posts_post_type_check CHECK ((post_type = ANY (ARRAY['post'::text, 'page'::text]))),
    CONSTRAINT posts_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'pending'::text, 'published'::text, 'scheduled'::text, 'trashed'::text])))
);

--
-- Name: saved_posts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.saved_posts (
    user_id uuid NOT NULL,
    post_id uuid NOT NULL,
    site_id uuid,
    saved_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: site_join_requests; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.site_join_requests (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    site_id uuid NOT NULL,
    token_hash text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: site_plugins; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.site_plugins (
    site_id uuid NOT NULL,
    plugin_name text NOT NULL,
    active boolean DEFAULT false NOT NULL,
    installed_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: site_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.site_settings (
    key text NOT NULL,
    value text DEFAULT ''::text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    site_id uuid
);

--
-- Name: site_users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.site_users (
    site_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role text DEFAULT 'subscriber'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    invited_by uuid,
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    can_self_publish boolean DEFAULT false NOT NULL,
    CONSTRAINT site_users_role_check CHECK ((role = ANY (ARRAY['admin'::text, 'editor'::text, 'author'::text, 'subscriber'::text])))
);

--
-- Name: COLUMN site_users.invited_by; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.site_users.invited_by IS 'User (admin or super_admin) who added this person to the site. NULL for legacy/CLI-seeded rows.';

--
-- Name: sites; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sites (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    hostname text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    owner_user_id uuid,
    parent_site_id uuid
);

--
-- Name: COLUMN sites.owner_user_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sites.owner_user_id IS 'Immutable creator of this site. NULL = installed by CLI / super_admin. Never updated after insert.';

--
-- Name: COLUMN sites.parent_site_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sites.parent_site_id IS 'Site the creator was logged into when this site was created. NULL for top-level sites (created by super_admin, or the system default site). Non-NULL sites cannot manage their own branding and inherit it from this parent instead.';

--
-- Name: taxonomies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.taxonomies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    slug text NOT NULL,
    taxonomy text DEFAULT 'category'::text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    site_id uuid,
    CONSTRAINT taxonomies_taxonomy_check CHECK ((taxonomy = ANY (ARRAY['category'::text, 'tag'::text])))
);

--
-- Name: theme_options; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.theme_options (
    site_id uuid NOT NULL,
    theme_name text NOT NULL,
    option_key text NOT NULL,
    value text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    username text NOT NULL,
    email text NOT NULL,
    display_name text NOT NULL,
    password_hash text NOT NULL,
    bio text DEFAULT ''::text NOT NULL,
    avatar_media_id uuid,
    role text DEFAULT 'subscriber'::text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    is_protected boolean DEFAULT false NOT NULL,
    deleted_at timestamp with time zone,
    default_site_id uuid,
    is_seeded boolean DEFAULT false NOT NULL,
    dashboard_widget_layout jsonb,
    personal_data_erased_at timestamp with time zone,
    welcome_panel_dismissed_at timestamp with time zone,
    CONSTRAINT users_role_check CHECK ((role = ANY (ARRAY['super_admin'::text, 'site_admin'::text, 'editor'::text, 'author'::text, 'subscriber'::text])))
);

--
-- Name: COLUMN users.deleted_at; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.users.deleted_at IS 'Non-NULL = soft-deleted. User cannot log in; their content is preserved.';

--
-- Name: wp_import_media_map; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.wp_import_media_map (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    old_url text NOT NULL,
    media_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: wp_import_post_map; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.wp_import_post_map (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    site_id uuid NOT NULL,
    wp_post_id text NOT NULL,
    post_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: app_settings app_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.app_settings
    ADD CONSTRAINT app_settings_pkey PRIMARY KEY (key);

--
-- Name: audit_log audit_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_log
    ADD CONSTRAINT audit_log_pkey PRIMARY KEY (id);

--
-- Name: builder_projects builder_projects_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.builder_projects
    ADD CONSTRAINT builder_projects_pkey PRIMARY KEY (id);

--
-- Name: comments comments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comments
    ADD CONSTRAINT comments_pkey PRIMARY KEY (id);

--
-- Name: email_changes email_changes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_changes
    ADD CONSTRAINT email_changes_pkey PRIMARY KEY (id);

--
-- Name: email_providers email_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_providers
    ADD CONSTRAINT email_providers_pkey PRIMARY KEY (id);

--
-- Name: form_blocks form_blocks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_blocks
    ADD CONSTRAINT form_blocks_pkey PRIMARY KEY (site_id, form_name);

--
-- Name: form_submissions form_submissions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_submissions
    ADD CONSTRAINT form_submissions_pkey PRIMARY KEY (id);

--
-- Name: forms forms_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.forms
    ADD CONSTRAINT forms_pkey PRIMARY KEY (id);

--
-- Name: forms forms_site_id_slug_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.forms
    ADD CONSTRAINT forms_site_id_slug_key UNIQUE (site_id, slug);

--
-- Name: mail_log mail_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mail_log
    ADD CONSTRAINT mail_log_pkey PRIMARY KEY (id);

--
-- Name: media_folders media_folders_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media_folders
    ADD CONSTRAINT media_folders_pkey PRIMARY KEY (id);

--
-- Name: media_folders media_folders_site_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media_folders
    ADD CONSTRAINT media_folders_site_id_name_key UNIQUE (site_id, name);

--
-- Name: media media_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media
    ADD CONSTRAINT media_pkey PRIMARY KEY (id);

--
-- Name: nav_menu_items nav_menu_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menu_items
    ADD CONSTRAINT nav_menu_items_pkey PRIMARY KEY (id);

--
-- Name: nav_menus nav_menus_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menus
    ADD CONSTRAINT nav_menus_pkey PRIMARY KEY (id);

--
-- Name: nav_menus nav_menus_site_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menus
    ADD CONSTRAINT nav_menus_site_id_name_key UNIQUE (site_id, name);

--
-- Name: page_compositions page_compositions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.page_compositions
    ADD CONSTRAINT page_compositions_pkey PRIMARY KEY (id);

--
-- Name: password_resets password_resets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.password_resets
    ADD CONSTRAINT password_resets_pkey PRIMARY KEY (id);

--
-- Name: poll_votes poll_votes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.poll_votes
    ADD CONSTRAINT poll_votes_pkey PRIMARY KEY (id);

--
-- Name: polls polls_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.polls
    ADD CONSTRAINT polls_pkey PRIMARY KEY (id);

--
-- Name: polls polls_site_id_slug_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.polls
    ADD CONSTRAINT polls_site_id_slug_key UNIQUE (site_id, slug);

--
-- Name: post_meta post_meta_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_meta
    ADD CONSTRAINT post_meta_pkey PRIMARY KEY (id);

--
-- Name: post_meta post_meta_post_id_meta_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_meta
    ADD CONSTRAINT post_meta_post_id_meta_key_key UNIQUE (post_id, meta_key);

--
-- Name: post_taxonomies post_taxonomies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_taxonomies
    ADD CONSTRAINT post_taxonomies_pkey PRIMARY KEY (post_id, taxonomy_id);

--
-- Name: post_views post_views_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_views
    ADD CONSTRAINT post_views_pkey PRIMARY KEY (post_id, ip_hash, viewed_date);

--
-- Name: posts posts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_pkey PRIMARY KEY (id);

--
-- Name: posts posts_site_slug_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_site_slug_unique UNIQUE (site_id, slug);

--
-- Name: saved_posts saved_posts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_posts
    ADD CONSTRAINT saved_posts_pkey PRIMARY KEY (user_id, post_id);

--
-- Name: site_join_requests site_join_requests_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_join_requests
    ADD CONSTRAINT site_join_requests_pkey PRIMARY KEY (id);

--
-- Name: site_plugins site_plugins_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_plugins
    ADD CONSTRAINT site_plugins_pkey PRIMARY KEY (site_id, plugin_name);

--
-- Name: site_users site_users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_users
    ADD CONSTRAINT site_users_pkey PRIMARY KEY (id);

--
-- Name: site_users site_users_site_user_role_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_users
    ADD CONSTRAINT site_users_site_user_role_unique UNIQUE (site_id, user_id, role);

--
-- Name: sites sites_hostname_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sites
    ADD CONSTRAINT sites_hostname_key UNIQUE (hostname);

--
-- Name: sites sites_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sites
    ADD CONSTRAINT sites_pkey PRIMARY KEY (id);

--
-- Name: taxonomies taxonomies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomies
    ADD CONSTRAINT taxonomies_pkey PRIMARY KEY (id);

--
-- Name: taxonomies taxonomies_site_name_tax_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomies
    ADD CONSTRAINT taxonomies_site_name_tax_unique UNIQUE (site_id, name, taxonomy);

--
-- Name: taxonomies taxonomies_site_slug_tax_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomies
    ADD CONSTRAINT taxonomies_site_slug_tax_unique UNIQUE (site_id, slug, taxonomy);

--
-- Name: theme_options theme_options_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.theme_options
    ADD CONSTRAINT theme_options_pkey PRIMARY KEY (site_id, theme_name, option_key);

--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);

--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

--
-- Name: wp_import_media_map wp_import_media_map_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_media_map
    ADD CONSTRAINT wp_import_media_map_pkey PRIMARY KEY (id);

--
-- Name: wp_import_media_map wp_import_media_map_site_id_old_url_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_media_map
    ADD CONSTRAINT wp_import_media_map_site_id_old_url_key UNIQUE (site_id, old_url);

--
-- Name: wp_import_post_map wp_import_post_map_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_post_map
    ADD CONSTRAINT wp_import_post_map_pkey PRIMARY KEY (id);

--
-- Name: wp_import_post_map wp_import_post_map_site_id_wp_post_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_post_map
    ADD CONSTRAINT wp_import_post_map_site_id_wp_post_id_key UNIQUE (site_id, wp_post_id);

--
-- Name: builder_projects_active_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX builder_projects_active_idx ON public.builder_projects USING btree (site_id) WHERE (is_active = true);

--
-- Name: builder_projects_site_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX builder_projects_site_id_idx ON public.builder_projects USING btree (site_id);

--
-- Name: email_changes_token_hash_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_changes_token_hash_idx ON public.email_changes USING btree (token_hash);

--
-- Name: email_changes_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_changes_user_id_idx ON public.email_changes USING btree (user_id);

--
-- Name: email_providers_site_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX email_providers_site_id_idx ON public.email_providers USING btree (site_id);

--
-- Name: idx_audit_log_actor; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_log_actor ON public.audit_log USING btree (actor_user_id);

--
-- Name: idx_audit_log_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_log_created ON public.audit_log USING btree (created_at DESC);

--
-- Name: idx_audit_log_site_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_log_site_created ON public.audit_log USING btree (site_id, created_at DESC);

--
-- Name: idx_comments_author_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_comments_author_id ON public.comments USING btree (author_id);

--
-- Name: idx_comments_parent_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_comments_parent_id ON public.comments USING btree (parent_id);

--
-- Name: idx_comments_post_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_comments_post_id ON public.comments USING btree (post_id);

--
-- Name: idx_form_submissions_form_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_form_submissions_form_id ON public.form_submissions USING btree (form_id) WHERE (form_id IS NOT NULL);

--
-- Name: idx_form_submissions_site_form; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_form_submissions_site_form ON public.form_submissions USING btree (site_id, form_name, submitted_at DESC);

--
-- Name: idx_mail_log_form_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mail_log_form_created ON public.mail_log USING btree (form_id, created_at DESC) WHERE (form_id IS NOT NULL);

--
-- Name: idx_mail_log_site_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mail_log_site_created ON public.mail_log USING btree (site_id, created_at DESC);

--
-- Name: idx_media_mime_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_media_mime_type ON public.media USING btree (mime_type);

--
-- Name: idx_media_uploaded_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_media_uploaded_by ON public.media USING btree (uploaded_by);

--
-- Name: idx_poll_votes_poll_ip; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_poll_votes_poll_ip ON public.poll_votes USING btree (poll_id, ip_address) WHERE (ip_address IS NOT NULL);

--
-- Name: idx_poll_votes_poll_option; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_poll_votes_poll_option ON public.poll_votes USING btree (poll_id, option_key);

--
-- Name: idx_poll_votes_poll_voter; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_poll_votes_poll_voter ON public.poll_votes USING btree (poll_id, voter_token);

--
-- Name: idx_post_meta_key; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_post_meta_key ON public.post_meta USING btree (meta_key);

--
-- Name: idx_post_meta_post_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_post_meta_post_id ON public.post_meta USING btree (post_id);

--
-- Name: idx_post_taxonomies_taxonomy_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_post_taxonomies_taxonomy_id ON public.post_taxonomies USING btree (taxonomy_id);

--
-- Name: idx_post_views_post_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_post_views_post_id ON public.post_views USING btree (post_id);

--
-- Name: idx_posts_author_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_author_id ON public.posts USING btree (author_id);

--
-- Name: idx_posts_post_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_post_type ON public.posts USING btree (post_type);

--
-- Name: idx_posts_published_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_published_at ON public.posts USING btree (published_at DESC);

--
-- Name: idx_posts_slug; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_slug ON public.posts USING btree (slug);

--
-- Name: idx_posts_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_status ON public.posts USING btree (status);

--
-- Name: idx_posts_status_type_published; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_posts_status_type_published ON public.posts USING btree (status, post_type, published_at DESC);

--
-- Name: idx_site_users_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_site_users_user_id ON public.site_users USING btree (user_id);

--
-- Name: idx_sites_hostname; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sites_hostname ON public.sites USING btree (hostname);

--
-- Name: idx_taxonomies_slug; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_taxonomies_slug ON public.taxonomies USING btree (slug);

--
-- Name: idx_taxonomies_taxonomy; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_taxonomies_taxonomy ON public.taxonomies USING btree (taxonomy);

--
-- Name: idx_users_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_active ON public.users USING btree (id) WHERE (deleted_at IS NULL);

--
-- Name: idx_users_email; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_email ON public.users USING btree (email);

--
-- Name: idx_users_role; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_role ON public.users USING btree (role);

--
-- Name: idx_users_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_username ON public.users USING btree (username);

--
-- Name: idx_wp_import_media_map_site_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_wp_import_media_map_site_id ON public.wp_import_media_map USING btree (site_id);

--
-- Name: idx_wp_import_post_map_site_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_wp_import_post_map_site_id ON public.wp_import_post_map USING btree (site_id);

--
-- Name: nav_menu_items_menu_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX nav_menu_items_menu_id_idx ON public.nav_menu_items USING btree (menu_id);

--
-- Name: nav_menu_items_parent_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX nav_menu_items_parent_id_idx ON public.nav_menu_items USING btree (parent_id);

--
-- Name: page_compositions_homepage_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX page_compositions_homepage_idx ON public.page_compositions USING btree (site_id) WHERE (is_homepage = true);

--
-- Name: page_compositions_project_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX page_compositions_project_id_idx ON public.page_compositions USING btree (project_id);

--
-- Name: page_compositions_site_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX page_compositions_site_id_idx ON public.page_compositions USING btree (site_id);

--
-- Name: page_compositions_slug_project_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX page_compositions_slug_project_idx ON public.page_compositions USING btree (project_id, slug) WHERE (slug IS NOT NULL);

--
-- Name: password_resets_token_hash_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX password_resets_token_hash_idx ON public.password_resets USING btree (token_hash);

--
-- Name: password_resets_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX password_resets_user_id_idx ON public.password_resets USING btree (user_id);

--
-- Name: posts_parent_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX posts_parent_id_idx ON public.posts USING btree (parent_id);

--
-- Name: saved_posts_user_site_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX saved_posts_user_site_idx ON public.saved_posts USING btree (user_id, site_id);

--
-- Name: site_join_requests_token_hash_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX site_join_requests_token_hash_idx ON public.site_join_requests USING btree (token_hash);

--
-- Name: site_join_requests_user_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX site_join_requests_user_id_idx ON public.site_join_requests USING btree (user_id);

--
-- Name: site_settings_global_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX site_settings_global_key_idx ON public.site_settings USING btree (key) WHERE (site_id IS NULL);

--
-- Name: site_settings_site_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX site_settings_site_key_idx ON public.site_settings USING btree (site_id, key) WHERE (site_id IS NOT NULL);

--
-- Name: users_email_lower_unique; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX users_email_lower_unique ON public.users USING btree (lower(email));

--
-- Name: audit_log audit_log_actor_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_log
    ADD CONSTRAINT audit_log_actor_user_id_fkey FOREIGN KEY (actor_user_id) REFERENCES public.users(id) ON DELETE SET NULL;

--
-- Name: builder_projects builder_projects_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.builder_projects
    ADD CONSTRAINT builder_projects_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;

--
-- Name: builder_projects builder_projects_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.builder_projects
    ADD CONSTRAINT builder_projects_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: comments comments_author_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comments
    ADD CONSTRAINT comments_author_id_fkey FOREIGN KEY (author_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: comments comments_parent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comments
    ADD CONSTRAINT comments_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.comments(id) ON DELETE CASCADE;

--
-- Name: comments comments_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comments
    ADD CONSTRAINT comments_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: comments comments_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comments
    ADD CONSTRAINT comments_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: email_changes email_changes_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_changes
    ADD CONSTRAINT email_changes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: email_providers email_providers_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.email_providers
    ADD CONSTRAINT email_providers_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: users fk_users_avatar_media; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT fk_users_avatar_media FOREIGN KEY (avatar_media_id) REFERENCES public.media(id) ON DELETE SET NULL;

--
-- Name: form_blocks form_blocks_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_blocks
    ADD CONSTRAINT form_blocks_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: form_submissions form_submissions_form_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_submissions
    ADD CONSTRAINT form_submissions_form_id_fkey FOREIGN KEY (form_id) REFERENCES public.forms(id) ON DELETE SET NULL;

--
-- Name: form_submissions form_submissions_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.form_submissions
    ADD CONSTRAINT form_submissions_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: forms forms_email_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.forms
    ADD CONSTRAINT forms_email_provider_id_fkey FOREIGN KEY (email_provider_id) REFERENCES public.email_providers(id) ON DELETE SET NULL;

--
-- Name: forms forms_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.forms
    ADD CONSTRAINT forms_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: mail_log mail_log_form_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mail_log
    ADD CONSTRAINT mail_log_form_id_fkey FOREIGN KEY (form_id) REFERENCES public.forms(id) ON DELETE SET NULL;

--
-- Name: mail_log mail_log_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mail_log
    ADD CONSTRAINT mail_log_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: media media_folder_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media
    ADD CONSTRAINT media_folder_id_fkey FOREIGN KEY (folder_id) REFERENCES public.media_folders(id) ON DELETE SET NULL;

--
-- Name: media_folders media_folders_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media_folders
    ADD CONSTRAINT media_folders_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: media media_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media
    ADD CONSTRAINT media_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: media media_uploaded_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.media
    ADD CONSTRAINT media_uploaded_by_fkey FOREIGN KEY (uploaded_by) REFERENCES public.users(id) ON DELETE RESTRICT;

--
-- Name: nav_menu_items nav_menu_items_menu_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menu_items
    ADD CONSTRAINT nav_menu_items_menu_id_fkey FOREIGN KEY (menu_id) REFERENCES public.nav_menus(id) ON DELETE CASCADE;

--
-- Name: nav_menu_items nav_menu_items_page_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menu_items
    ADD CONSTRAINT nav_menu_items_page_id_fkey FOREIGN KEY (page_id) REFERENCES public.posts(id) ON DELETE SET NULL;

--
-- Name: nav_menu_items nav_menu_items_parent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menu_items
    ADD CONSTRAINT nav_menu_items_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.nav_menu_items(id) ON DELETE CASCADE;

--
-- Name: nav_menus nav_menus_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.nav_menus
    ADD CONSTRAINT nav_menus_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: page_compositions page_compositions_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.page_compositions
    ADD CONSTRAINT page_compositions_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;

--
-- Name: page_compositions page_compositions_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.page_compositions
    ADD CONSTRAINT page_compositions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.builder_projects(id) ON DELETE CASCADE;

--
-- Name: page_compositions page_compositions_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.page_compositions
    ADD CONSTRAINT page_compositions_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: password_resets password_resets_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.password_resets
    ADD CONSTRAINT password_resets_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: poll_votes poll_votes_poll_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.poll_votes
    ADD CONSTRAINT poll_votes_poll_id_fkey FOREIGN KEY (poll_id) REFERENCES public.polls(id) ON DELETE CASCADE;

--
-- Name: poll_votes poll_votes_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.poll_votes
    ADD CONSTRAINT poll_votes_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: polls polls_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.polls
    ADD CONSTRAINT polls_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: post_meta post_meta_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_meta
    ADD CONSTRAINT post_meta_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: post_taxonomies post_taxonomies_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_taxonomies
    ADD CONSTRAINT post_taxonomies_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: post_taxonomies post_taxonomies_taxonomy_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_taxonomies
    ADD CONSTRAINT post_taxonomies_taxonomy_id_fkey FOREIGN KEY (taxonomy_id) REFERENCES public.taxonomies(id) ON DELETE CASCADE;

--
-- Name: post_views post_views_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.post_views
    ADD CONSTRAINT post_views_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: posts posts_author_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_author_id_fkey FOREIGN KEY (author_id) REFERENCES public.users(id) ON DELETE RESTRICT;

--
-- Name: posts posts_featured_image_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_featured_image_id_fkey FOREIGN KEY (featured_image_id) REFERENCES public.media(id) ON DELETE SET NULL;

--
-- Name: posts posts_parent_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.posts(id) ON DELETE SET NULL;

--
-- Name: posts posts_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: saved_posts saved_posts_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_posts
    ADD CONSTRAINT saved_posts_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: saved_posts saved_posts_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_posts
    ADD CONSTRAINT saved_posts_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: saved_posts saved_posts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_posts
    ADD CONSTRAINT saved_posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: site_join_requests site_join_requests_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_join_requests
    ADD CONSTRAINT site_join_requests_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: site_join_requests site_join_requests_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_join_requests
    ADD CONSTRAINT site_join_requests_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: site_plugins site_plugins_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_plugins
    ADD CONSTRAINT site_plugins_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: site_settings site_settings_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_settings
    ADD CONSTRAINT site_settings_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: site_users site_users_invited_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_users
    ADD CONSTRAINT site_users_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES public.users(id) ON DELETE SET NULL;

--
-- Name: site_users site_users_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_users
    ADD CONSTRAINT site_users_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: site_users site_users_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.site_users
    ADD CONSTRAINT site_users_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

--
-- Name: sites sites_owner_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sites
    ADD CONSTRAINT sites_owner_user_id_fkey FOREIGN KEY (owner_user_id) REFERENCES public.users(id) ON DELETE SET NULL;

--
-- Name: sites sites_parent_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sites
    ADD CONSTRAINT sites_parent_site_id_fkey FOREIGN KEY (parent_site_id) REFERENCES public.sites(id) ON DELETE SET NULL;

--
-- Name: taxonomies taxonomies_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.taxonomies
    ADD CONSTRAINT taxonomies_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: users users_default_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_default_site_id_fkey FOREIGN KEY (default_site_id) REFERENCES public.sites(id) ON DELETE SET NULL;

--
-- Name: wp_import_media_map wp_import_media_map_media_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_media_map
    ADD CONSTRAINT wp_import_media_map_media_id_fkey FOREIGN KEY (media_id) REFERENCES public.media(id) ON DELETE CASCADE;

--
-- Name: wp_import_media_map wp_import_media_map_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_media_map
    ADD CONSTRAINT wp_import_media_map_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- Name: wp_import_post_map wp_import_post_map_post_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_post_map
    ADD CONSTRAINT wp_import_post_map_post_id_fkey FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;

--
-- Name: wp_import_post_map wp_import_post_map_site_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.wp_import_post_map
    ADD CONSTRAINT wp_import_post_map_site_id_fkey FOREIGN KEY (site_id) REFERENCES public.sites(id) ON DELETE CASCADE;

--
-- PostgreSQL database dump complete
--

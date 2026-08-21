<?php
/**
 * Populates the local WordPress dev install with a large volume of test
 * content, for exercising SynapCMS's WP importer
 * (core/src/handlers/admin/wp_import.rs) end-to-end: authors (some of which
 * the importer will need to create fresh Synap accounts for), categories,
 * tags, media (including images referenced inside post content, to test
 * URL rewriting), posts and pages (with parent/child nesting), a mix of
 * WP statuses (publish/draft/pending/future/private), featured images, and
 * custom fields.
 *
 * Not meant to be run directly — invoked by scripts/populate-wp-test-data.sh
 * via `wp eval-file`, which bootstraps WordPress first.
 *
 * Everything it creates is prefixed/tagged "synaptest" (usernames, the
 * `synaptest_source` postmeta) so a future cleanup pass has something to
 * search for, and re-running is safe — existing users/terms with a matching
 * name are reused rather than duplicated (posts/pages are not de-duplicated;
 * re-running adds more of them each time, which is usually what you want for
 * a bigger import test). Media is additive too: each run downloads a fresh
 * batch of new images on top of whatever "Synaptest Image N" attachments
 * already exist, so the media library actually grows — useful for testing
 * the media manager, not just the importer.
 */

$total_posts = isset($args[0]) ? max(10, (int) $args[0]) : 200;
$new_media   = isset($args[1]) ? max(0, (int) $args[1]) : 15;
$num_pages   = min(20, max(5, intdiv($total_posts, 10)));
$num_posts   = $total_posts - $num_pages;

WP_CLI::log("Populating {$total_posts} items ({$num_posts} posts, {$num_pages} pages)...");

// ── Users / authors ─────────────────────────────────────────────────────
$author_defs = [
    ['synaptest-editor1', 'editor'],
    ['synaptest-editor2', 'editor'],
    ['synaptest-author1', 'author'],
    ['synaptest-author2', 'author'],
    ['synaptest-author3', 'author'],
    ['synaptest-contrib1', 'contributor'],
];
$author_ids = [get_current_user_id() ?: 1];
foreach ($author_defs as [$login, $role]) {
    $existing = get_user_by('login', $login);
    if ($existing) {
        $author_ids[] = $existing->ID;
        continue;
    }
    $uid = wp_insert_user([
        'user_login'   => $login,
        'user_email'   => $login . '@synaptest.local',
        'user_pass'    => wp_generate_password(16, true),
        'role'         => $role,
        'display_name' => ucfirst(preg_replace('/[0-9]+$/', '', substr($login, strlen('synaptest-')))) . ' ' . substr($login, -1),
    ]);
    if (is_wp_error($uid)) {
        WP_CLI::warning("user {$login}: " . $uid->get_error_message());
        continue;
    }
    $author_ids[] = $uid;
}
WP_CLI::success(count($author_ids) . ' authors ready.');

// ── Categories & tags ────────────────────────────────────────────────────
$cat_names = ['News', 'Tutorials', 'Reviews', 'Opinion', 'How-To', 'Interviews', 'Announcements', 'Case Studies', 'Guides', 'Events'];
$cat_ids = [];
foreach ($cat_names as $name) {
    $term = term_exists($name, 'category');
    if (!$term) $term = wp_insert_term($name, 'category');
    $cat_ids[] = is_wp_error($term) ? null : (int) $term['term_id'];
}
$cat_ids = array_values(array_filter($cat_ids));

$tag_pool = ['rust', 'wordpress', 'migration', 'performance', 'security', 'design', 'ux', 'api', 'database', 'hosting',
    'cms', 'plugins', 'themes', 'seo', 'accessibility', 'testing', 'deployment', 'caching', 'media', 'forms'];
$tag_ids = [];
foreach ($tag_pool as $name) {
    $term = term_exists($name, 'post_tag');
    if (!$term) $term = wp_insert_term($name, 'post_tag');
    $tag_ids[] = is_wp_error($term) ? null : (int) $term['term_id'];
}
$tag_ids = array_values(array_filter($tag_ids));
WP_CLI::success(count($cat_ids) . ' categories, ' . count($tag_ids) . ' tags ready.');

// ── Media (featured images + in-content images for rewrite testing) ──────
require_once ABSPATH . 'wp-admin/includes/media.php';
require_once ABSPATH . 'wp-admin/includes/file.php';
require_once ABSPATH . 'wp-admin/includes/image.php';

$media_ids = [];
$max_existing = 0;
$existing_attachments = get_posts([
    'post_type'      => 'attachment',
    'post_status'    => 'any',
    'posts_per_page' => -1,
    's'              => 'Synaptest Image ',
    'fields'         => 'ids',
]);
foreach ($existing_attachments as $aid) {
    $title = get_the_title($aid);
    if (preg_match('/^Synaptest Image (\d+)$/', $title, $m)) {
        $media_ids[] = $aid;
        $max_existing = max($max_existing, (int) $m[1]);
    }
}
for ($i = $max_existing + 1; $i <= $max_existing + $new_media; $i++) {
    $url = "https://picsum.photos/seed/synaptest{$i}/800/600";
    $tmp = download_url($url);
    if (is_wp_error($tmp)) {
        WP_CLI::warning("media {$i}: " . $tmp->get_error_message());
        continue;
    }
    $id = media_handle_sideload(['name' => "synaptest-image-{$i}.jpg", 'tmp_name' => $tmp], 0, "Synaptest Image {$i}");
    if (is_wp_error($id)) {
        @unlink($tmp);
        WP_CLI::warning("media {$i} sideload: " . $id->get_error_message());
        continue;
    }
    $media_ids[] = $id;
}
WP_CLI::success(count($media_ids) . ' media items ready (' . $new_media . ' new).');

// ── Pages (some nested under an earlier top-level page) ──────────────────
$page_titles = ['About Us', 'Contact', 'Services', 'Privacy Policy', 'Terms of Service', 'Team', 'Careers', 'FAQ', 'Support', 'Pricing',
    'Our Story', 'Partners', 'Press', 'Blog', 'Portfolio', 'Testimonials', 'Locations', 'Downloads', 'Community', 'Sitemap'];
$page_ids = [];
$top_level_pages = [];
for ($i = 0; $i < $num_pages; $i++) {
    $title = $page_titles[$i % count($page_titles)] . ($i >= count($page_titles) ? ' ' . $i : '');
    $author = $author_ids[array_rand($author_ids)];
    $parent = 0;
    if ($i >= 5 && mt_rand(1, 100) <= 30 && !empty($top_level_pages)) {
        $parent = $top_level_pages[array_rand($top_level_pages)];
    }
    $id = wp_insert_post([
        'post_title'   => $title,
        'post_content' => synaptest_lorem($media_ids),
        'post_status'  => 'publish',
        'post_type'    => 'page',
        'post_author'  => $author,
        'post_parent'  => $parent,
    ]);
    if (is_wp_error($id) || !$id) continue;
    if ($parent === 0) $top_level_pages[] = $id;
    $page_ids[] = $id;
    if (!empty($media_ids) && mt_rand(1, 100) <= 40) {
        set_post_thumbnail($id, $media_ids[array_rand($media_ids)]);
    }
}
WP_CLI::success(count($page_ids) . ' pages created.');

// ── Posts ──────────────────────────────────────────────────────────────
$statuses = array_merge(
    array_fill(0, 70, 'publish'),
    array_fill(0, 10, 'draft'),
    array_fill(0, 8, 'pending'),
    array_fill(0, 6, 'future'),
    array_fill(0, 6, 'private'),
);
$created_posts = 0;
for ($i = 0; $i < $num_posts; $i++) {
    $author = $author_ids[array_rand($author_ids)];
    $status = $statuses[array_rand($statuses)];
    $post_args = [
        'post_title'   => synaptest_title($i),
        'post_content' => synaptest_lorem($media_ids),
        'post_excerpt' => 'Synaptest excerpt for post ' . ($i + 1) . '.',
        'post_status'  => $status,
        'post_type'    => 'post',
        'post_author'  => $author,
    ];
    if ($status === 'future') {
        $post_args['post_date'] = date('Y-m-d H:i:s', strtotime('+' . mt_rand(1, 30) . ' days'));
        $post_args['edit_date'] = true;
    }
    $id = wp_insert_post($post_args);
    if (is_wp_error($id) || !$id) {
        WP_CLI::warning("post {$i}: " . (is_wp_error($id) ? $id->get_error_message() : 'unknown error'));
        continue;
    }
    $created_posts++;

    $shuffled_cats = $cat_ids;
    shuffle($shuffled_cats);
    wp_set_post_categories($id, array_slice($shuffled_cats, 0, mt_rand(1, 3)));

    $shuffled_tags = $tag_ids;
    shuffle($shuffled_tags);
    wp_set_post_tags($id, array_slice($shuffled_tags, 0, mt_rand(1, 4)));

    if (!empty($media_ids) && mt_rand(1, 100) <= 60) {
        set_post_thumbnail($id, $media_ids[array_rand($media_ids)]);
    }

    if (mt_rand(1, 100) <= 30) {
        update_post_meta($id, 'synaptest_rating', mt_rand(1, 5));
    }
    update_post_meta($id, 'synaptest_source', 'populate-wp-test-data');

    if ($created_posts % 25 === 0) {
        WP_CLI::log("  ...{$created_posts} posts so far");
    }
}
WP_CLI::success("{$created_posts} posts created.");

WP_CLI::success('Done. Export via WP Admin -> Tools -> Export -> All Content, or: wp export --dir=/tmp --allow-root --path=' . ABSPATH);

function synaptest_title($i) {
    $adjectives = ['Complete', 'Practical', 'Modern', 'Essential', 'Quick', 'Deep', 'Simple', 'Advanced', 'Honest', 'Behind-the-Scenes'];
    $topics = ['Guide to Migration', 'Look at Performance', 'Review of CMS Tools', 'Tutorial', 'Case Study', 'Overview', 'Checklist', 'Comparison', 'Walkthrough', 'Retrospective'];
    return $adjectives[array_rand($adjectives)] . ' ' . $topics[array_rand($topics)] . ' #' . ($i + 1);
}

function synaptest_lorem($media_ids = []) {
    $paras = [];
    $count = mt_rand(3, 6);
    for ($p = 0; $p < $count; $p++) {
        $words = [];
        for ($w = 0, $n = mt_rand(20, 50); $w < $n; $w++) {
            $words[] = synaptest_word();
        }
        $paras[] = '<p>' . implode(' ', $words) . '.</p>';
    }
    if (!empty($media_ids) && mt_rand(1, 100) <= 50) {
        $url = wp_get_attachment_url($media_ids[array_rand($media_ids)]);
        if ($url) {
            $insert_at = mt_rand(1, count($paras));
            array_splice($paras, $insert_at, 0, ['<img src="' . esc_url($url) . '" alt="test image" />']);
        }
    }
    return implode("\n\n", $paras);
}

function synaptest_word() {
    static $words = ['lorem', 'ipsum', 'dolor', 'sit', 'amet', 'consectetur', 'adipiscing', 'elit', 'sed', 'do', 'eiusmod',
        'tempor', 'incididunt', 'ut', 'labore', 'et', 'dolore', 'magna', 'aliqua', 'enim', 'minim', 'veniam', 'quis',
        'nostrud', 'exercitation', 'ullamco', 'laboris', 'nisi', 'aliquip', 'ex', 'ea', 'commodo', 'consequat'];
    return $words[array_rand($words)];
}

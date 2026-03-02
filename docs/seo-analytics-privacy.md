# SEO, Analytics & Privacy

> Built into every site out of the box — no plugins required.

---

## SEO Meta Tags

Every page on your site automatically includes the meta tags that search engines and social platforms look for. You don't need to configure anything for this to work.

**What's included on every page:**

- **Description** — tells search engines what the page is about
- **Canonical URL** — prevents duplicate content penalties by pointing to the authoritative version of each page
- **Open Graph tags** — controls how your content appears when shared on Facebook, LinkedIn, and similar platforms (title, description, image, type)
- **Twitter Card tags** — same as above but for X/Twitter

**On blog posts and pages**, the tags are filled in automatically from the post's content:
- Title comes from the post title
- Description comes from the post excerpt (set this in the post editor for best results)
- Image comes from the post's featured image — if one is set, links will show a large preview image when shared; without one, a smaller summary card is used

**On the home page, archive, and search results**, the tags use your site name and site description. Set your site description at **Admin → Settings → Site Description**.

---

## Sitemap

Your sitemap is always available at `/sitemap.xml` (e.g. `https://yoursite.com/sitemap.xml`).

It updates automatically as you publish content — no manual steps needed. Every published post and page appears in the sitemap with its URL and last modified date.

**Submit it to search engines** to help them discover your content faster:
- Google: [Google Search Console](https://search.google.com/search-console) → Sitemaps → paste your sitemap URL
- Bing: Bing Webmaster Tools → Sitemaps

---

## Cookie Consent Banner

A GDPR-compliant cookie consent banner is shown to first-time visitors. It gives them a clear choice before any tracking takes place.

- **Accept** — the visitor consents to cookies; analytics will load if configured
- **Dismiss** — the visitor declines; no analytics or tracking scripts load

The visitor's choice is remembered in their browser. The banner won't show again on return visits unless they clear their browser data.

**No configuration needed** — the banner is on by default.

---

## Google Analytics

Google Analytics is ready to be switched on but is **off by default**. No tracking happens until you add your Measurement ID.

**To enable:**

1. Go to **Admin → Appearance → Edit Theme**
2. Open `base.html`
3. Find this line near the bottom:
   ```
   var GA_ID = '';
   ```
4. Replace the empty quotes with your Measurement ID:
   ```
   var GA_ID = 'G-XXXXXXXXXX';
   ```
5. Save the file

That's it. Analytics will start tracking on the next page load.

**GA is tied to cookie consent** — even after you add your Measurement ID, the tracking script only loads for visitors who click Accept on the cookie banner. Visitors who click Dismiss are never tracked. This satisfies GDPR requirements for analytics cookies without any extra configuration.

**Don't have a Measurement ID yet?** Create a free account at [analytics.google.com](https://analytics.google.com), set up a property for your site, and Google will give you an ID starting with `G-`.

---

## Other Analytics Providers

The same approach works for any script-based analytics tool (Plausible, Fathom, Matomo, etc.). Open `base.html` in the theme editor, find the analytics section near the bottom, and paste your provider's snippet inside the `loadAnalytics()` function. It will automatically respect the visitor's cookie consent choice.

---

## Tips

- **Set your site description** — it's the single biggest thing you can do to improve how your site appears in search results and social shares. Admin → Settings → Site Description.
- **Write post excerpts** — if you leave the excerpt blank, the description tag falls back to the beginning of the post content, which may be cut off awkwardly. A hand-written excerpt gives you control over what appears in Google results and link previews.
- **Use featured images** — posts with a featured image get large preview cards when shared on social media. Posts without one get a small text-only card.

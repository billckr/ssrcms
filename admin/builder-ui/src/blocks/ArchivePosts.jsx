import { ColorField, PADDING_OPTIONS } from './ColorField'

const PLACEHOLDER_POSTS = [
  'The Art of Minimalist Design',
  'Building for the Web in 2026',
  'Why Rust is the Future',
  'Exploring Mountain Trails',
  'A Guide to Slow Living',
  'Photography on a Budget',
  'Urban Gardening Tips',
  'The Case for Deep Work',
  'Fermentation at Home',
]

export const ArchivePostsBlock = {
  label: 'Archive Posts',
  fields: {
    padding: {
      type: 'select',
      label: 'Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    bgColor: {
      type: 'custom',
      label: 'Background Color',
      render: ({ value, onChange }) => (
        <ColorField label="Background Color" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    headingColor: {
      type: 'custom',
      label: 'Heading Color',
      render: ({ value, onChange }) => (
        <ColorField label="Heading Color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    linkColor: {
      type: 'custom',
      label: 'Post Title Color',
      render: ({ value, onChange }) => (
        <ColorField label="Post Title Color" value={value ?? '#2563eb'} onChange={onChange} />
      ),
    },
    textColor: {
      type: 'custom',
      label: 'Text Color',
      render: ({ value, onChange }) => (
        <ColorField label="Text Color" value={value ?? '#374151'} onChange={onChange} />
      ),
    },
    showExcerpt: {
      type: 'custom',
      label: 'Show Excerpt',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show post excerpt
        </label>
      ),
    },
    showDate: {
      type: 'custom',
      label: 'Show Date',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show published date
        </label>
      ),
    },
  },
  defaultProps: {
    padding: '40px',
    bgColor: '#ffffff',
    headingColor: '#111827',
    linkColor: '#2563eb',
    textColor: '#374151',
    showExcerpt: true,
    showDate: true,
  },
  render: ({ bgColor, padding, headingColor, linkColor, textColor, showExcerpt, showDate }) => (
    <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
      <div style={{ maxWidth: 1200, margin: '0 auto' }}>
        {/* Archive heading placeholder */}
        <h1 style={{ color: headingColor, fontSize: 28, fontWeight: 700, margin: '0 0 8px' }}>
          Category: Lifestyle
        </h1>
        <p style={{ color: textColor, fontSize: 14, margin: '0 0 32px' }}>
          12 posts
        </p>
        {/* 3-column grid */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 24 }}>
          {PLACEHOLDER_POSTS.map((title, i) => (
            <div key={i} style={{ border: '1px solid #e5e7eb', borderRadius: 8, padding: 20 }}>
              <a href="#" style={{ color: linkColor, fontWeight: 600, fontSize: 15, textDecoration: 'none' }}>
                {title}
              </a>
              {showDate !== false && (
                <p style={{ color: textColor, fontSize: 12, margin: '6px 0 0' }}>January 1, 2026</p>
              )}
              {showExcerpt !== false && (
                <p style={{ color: textColor, fontSize: 13, margin: '8px 0 0', lineHeight: 1.5 }}>
                  A short excerpt previewing what this post is about…
                </p>
              )}
            </div>
          ))}
        </div>
        {/* Pagination placeholder */}
        <div style={{ marginTop: 32, display: 'flex', gap: 8, justifyContent: 'center' }}>
          {['← Prev', '1', '2', '3', 'Next →'].map((l, i) => (
            <span key={i} style={{
              padding: '6px 12px', border: '1px solid #e5e7eb', borderRadius: 4,
              fontSize: 13, color: i === 1 ? '#fff' : textColor,
              background: i === 1 ? linkColor : 'transparent',
            }}>{l}</span>
          ))}
        </div>
      </div>
    </div>
  ),
}

import { ColorField, PADDING_OPTIONS } from './ColorField'

const LIMIT_OPTIONS = [
  { label: '3',  value: 3 },
  { label: '5',  value: 5 },
  { label: '10', value: 10 },
  { label: '20', value: 20 },
]

const LAYOUT_OPTIONS = [
  { label: 'Cards', value: 'cards' },
  { label: 'List',  value: 'list' },
]

const PLACEHOLDER_POSTS = [
  { title: 'Post title one',   date: 'Jan 1, 2025', excerpt: 'A short excerpt for this post.' },
  { title: 'Post title two',   date: 'Jan 8, 2025', excerpt: 'A short excerpt for this post.' },
  { title: 'Post title three', date: 'Jan 15, 2025', excerpt: 'A short excerpt for this post.' },
]

export const PostsBlock = {
  label: 'Posts',
  fields: {
    heading: { type: 'text', label: 'Heading' },
    limit: {
      type: 'select',
      label: 'Number of posts',
      options: LIMIT_OPTIONS,
    },
    layout: {
      type: 'select',
      label: 'Layout',
      options: LAYOUT_OPTIONS,
    },
    showExcerpt: {
      type: 'custom',
      label: 'Show excerpt',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
          Show excerpt
        </label>
      ),
    },
    showDate: {
      type: 'custom',
      label: 'Show date',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
          Show date
        </label>
      ),
    },
    padding: {
      type: 'select',
      label: 'Outer padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    bgColor: {
      type: 'custom',
      label: 'Background color',
      render: ({ value, onChange }) => (
        <ColorField label="Background color" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    headingColor: {
      type: 'custom',
      label: 'Heading color',
      render: ({ value, onChange }) => (
        <ColorField label="Heading color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    textColor: {
      type: 'custom',
      label: 'Text color',
      render: ({ value, onChange }) => (
        <ColorField label="Text color" value={value ?? '#374151'} onChange={onChange} />
      ),
    },
    linkColor: {
      type: 'custom',
      label: 'Link color',
      render: ({ value, onChange }) => (
        <ColorField label="Link color" value={value ?? '#2563eb'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    heading:     'Latest Posts',
    limit:       5,
    layout:      'cards',
    showExcerpt: true,
    showDate:    true,
    padding:     '32px 40px',
    bgColor:     '#ffffff',
    headingColor: '#111827',
    textColor:   '#374151',
    linkColor:   '#2563eb',
  },
  render: ({ heading, limit, layout, showExcerpt, showDate, padding, bgColor, headingColor, textColor, linkColor }) => {
    const preview = PLACEHOLDER_POSTS.slice(0, Math.min(limit, 3))
    const isCards = layout === 'cards'
    return (
      <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
        {heading && (
          <h2 style={{ color: headingColor, marginTop: 0, marginBottom: 24, fontSize: 24, fontWeight: 700 }}>
            {heading}
          </h2>
        )}
        <div style={{
          display: isCards ? 'grid' : 'block',
          gridTemplateColumns: isCards ? 'repeat(3, 1fr)' : undefined,
          gap: isCards ? 24 : undefined,
          maxWidth: 1200,
          margin: '0 auto',
        }}>
          {preview.map((p, i) => (
            <div key={i} style={{
              border: isCards ? '1px solid #e5e7eb' : 'none',
              borderBottom: !isCards ? '1px solid #e5e7eb' : undefined,
              borderRadius: isCards ? 8 : 0,
              padding: isCards ? 20 : '16px 0',
            }}>
              <a href="#" style={{ color: linkColor, fontWeight: 600, textDecoration: 'none', fontSize: 16 }}>
                {p.title}
              </a>
              {showDate && <p style={{ color: textColor, fontSize: 12, margin: '4px 0' }}>{p.date}</p>}
              {showExcerpt && <p style={{ color: textColor, fontSize: 14, margin: '8px 0 0' }}>{p.excerpt}</p>}
            </div>
          ))}
        </div>
        {limit > 3 && (
          <p style={{ color: textColor, fontSize: 12, marginTop: 12, fontStyle: 'italic' }}>
            + {limit - 3} more posts on the live page
          </p>
        )}
      </div>
    )
  },
}

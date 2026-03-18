import { ColorField, PADDING_OPTIONS } from './ColorField'

export const PostContentBlock = {
  label: 'Post Content',
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
    titleColor: {
      type: 'custom',
      label: 'Title Color',
      render: ({ value, onChange }) => (
        <ColorField label="Title Color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    textColor: {
      type: 'custom',
      label: 'Text Color',
      render: ({ value, onChange }) => (
        <ColorField label="Text Color" value={value ?? '#374151'} onChange={onChange} />
      ),
    },
    metaColor: {
      type: 'custom',
      label: 'Meta / Date Color',
      render: ({ value, onChange }) => (
        <ColorField label="Meta / Date Color" value={value ?? '#6b7280'} onChange={onChange} />
      ),
    },
    showFeaturedImage: {
      type: 'custom',
      label: 'Show Featured Image',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show featured image
        </label>
      ),
    },
    showMeta: {
      type: 'custom',
      label: 'Show Meta (date, author, categories)',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show date, author & categories
        </label>
      ),
    },
    showTags: {
      type: 'custom',
      label: 'Show Tags',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show tags
        </label>
      ),
    },
  },
  defaultProps: {
    padding: '40px',
    bgColor: '#ffffff',
    titleColor: '#111827',
    textColor: '#374151',
    metaColor: '#6b7280',
    showFeaturedImage: true,
    showMeta: true,
    showTags: true,
  },
  render: ({ bgColor, padding, titleColor, textColor, metaColor, showFeaturedImage, showMeta, showTags }) => (
    <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
      <div style={{ maxWidth: 800, margin: '0 auto' }}>
        {/* Placeholder featured image */}
        {showFeaturedImage !== false && (
          <div style={{
            width: '100%', height: 280, background: '#e5e7eb', borderRadius: 8,
            marginBottom: 24, display: 'flex', alignItems: 'center', justifyContent: 'center',
            color: '#9ca3af', fontSize: 14,
          }}>
            Featured Image
          </div>
        )}
        {/* Title */}
        <h1 style={{ color: titleColor, fontSize: 32, fontWeight: 700, margin: '0 0 12px', lineHeight: 1.2 }}>
          Post Title Will Appear Here
        </h1>
        {/* Meta */}
        {showMeta !== false && (
          <div style={{ color: metaColor, fontSize: 13, marginBottom: 24, display: 'flex', gap: 16, flexWrap: 'wrap' }}>
            <span>January 1, 2026</span>
            <span>By Author Name</span>
            <span>Category</span>
          </div>
        )}
        {/* Content placeholder */}
        <div style={{ color: textColor, lineHeight: 1.7, fontSize: 16 }}>
          <p style={{ margin: '0 0 16px' }}>
            The full post content will appear here. This block renders the title, featured image,
            date, author, categories, body content, and tags for whichever post the visitor is reading.
          </p>
          <p style={{ margin: '0 0 16px', color: metaColor, fontStyle: 'italic' }}>
            Add this block to a page, then mark that page as the Post Template from the builder page list.
          </p>
        </div>
        {/* Tags */}
        {showTags !== false && (
          <div style={{ marginTop: 24, display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {['tag-one', 'tag-two', 'tag-three'].map(t => (
              <span key={t} style={{
                background: '#f3f4f6', color: metaColor, fontSize: 12,
                padding: '4px 10px', borderRadius: 999,
              }}>{t}</span>
            ))}
          </div>
        )}
      </div>
    </div>
  ),
}

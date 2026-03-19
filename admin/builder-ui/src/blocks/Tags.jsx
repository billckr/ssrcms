import { ColorField, PADDING_OPTIONS, MAX_WIDTH_OPTIONS } from './ColorField'

const PLACEHOLDER_TAGS = ['design', 'react', 'rust', 'tutorial', 'tips', 'news']

export const TagsBlock = {
  label: 'Tags',
  fields: {
    heading: { type: 'text', label: 'Heading' },
    showCount: {
      type: 'custom',
      label: 'Show post count',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
          Show post count
        </label>
      ),
    },
    padding: {
      type: 'select',
      label: 'Outer padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    maxWidth: {
      type: 'select',
      label: 'Content max width',
      options: MAX_WIDTH_OPTIONS,
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
    tagBgColor: {
      type: 'custom',
      label: 'Tag background',
      render: ({ value, onChange }) => (
        <ColorField label="Tag background" value={value ?? '#f3f4f6'} onChange={onChange} />
      ),
    },
    tagTextColor: {
      type: 'custom',
      label: 'Tag text color',
      render: ({ value, onChange }) => (
        <ColorField label="Tag text color" value={value ?? '#374151'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    heading:      'Tags',
    showCount:    false,
    padding:      '32px 40px',
    maxWidth:     '1200px',
    bgColor:      '#ffffff',
    headingColor: '#111827',
    tagBgColor:   '#f3f4f6',
    tagTextColor: '#374151',
  },
  render: ({ heading, showCount, padding, maxWidth, bgColor, headingColor, tagBgColor, tagTextColor }) => (
    <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
      <div style={{ maxWidth: maxWidth || '1200px', margin: '0 auto' }}>
      {heading && (
        <h2 style={{ color: headingColor, marginTop: 0, marginBottom: 16, fontSize: 20, fontWeight: 700 }}>
          {heading}
        </h2>
      )}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
        {PLACEHOLDER_TAGS.map((tag, i) => (
          <a key={i} href="#" style={{
            background: tagBgColor, color: tagTextColor,
            padding: '4px 12px', borderRadius: 20,
            fontSize: 13, textDecoration: 'none', display: 'inline-block',
          }}>
            {tag}{showCount && <span style={{ marginLeft: 4, opacity: 0.6 }}>(2)</span>}
          </a>
        ))}
      </div>
      </div>
    </div>
  ),
}

import { ColorField, PADDING_OPTIONS } from './ColorField'

const PLACEHOLDER_CATS = ['Technology', 'Design', 'Business', 'Lifestyle']

export const CategoriesBlock = {
  label: 'Categories',
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
    linkColor: {
      type: 'custom',
      label: 'Link color',
      render: ({ value, onChange }) => (
        <ColorField label="Link color" value={value ?? '#2563eb'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    heading:      'Categories',
    showCount:    true,
    padding:      '32px 40px',
    bgColor:      '#ffffff',
    headingColor: '#111827',
    linkColor:    '#2563eb',
  },
  render: ({ heading, showCount, padding, bgColor, headingColor, linkColor }) => (
    <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
      {heading && (
        <h2 style={{ color: headingColor, marginTop: 0, marginBottom: 16, fontSize: 20, fontWeight: 700 }}>
          {heading}
        </h2>
      )}
      <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
        {PLACEHOLDER_CATS.map((cat, i) => (
          <li key={i} style={{ padding: '6px 0', borderBottom: '1px solid #f3f4f6' }}>
            <a href="#" style={{ color: linkColor, textDecoration: 'none', fontSize: 14 }}>
              {cat}{showCount && <span style={{ color: '#9ca3af', marginLeft: 6 }}>(4)</span>}
            </a>
          </li>
        ))}
      </ul>
    </div>
  ),
}

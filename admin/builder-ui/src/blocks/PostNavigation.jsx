import { ColorField, PADDING_OPTIONS } from './ColorField'

export const PostNavigationBlock = {
  label: 'Post Navigation',
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
    cardBgColor: {
      type: 'custom',
      label: 'Card Background',
      render: ({ value, onChange }) => (
        <ColorField label="Card Background" value={value ?? '#f9fafb'} onChange={onChange} />
      ),
    },
    borderColor: {
      type: 'custom',
      label: 'Card Border',
      render: ({ value, onChange }) => (
        <ColorField label="Card Border" value={value ?? '#e5e7eb'} onChange={onChange} />
      ),
    },
    labelColor: {
      type: 'custom',
      label: 'Label Color',
      render: ({ value, onChange }) => (
        <ColorField label="Label Color" value={value ?? '#6b7280'} onChange={onChange} />
      ),
    },
    titleColor: {
      type: 'custom',
      label: 'Title Color',
      render: ({ value, onChange }) => (
        <ColorField label="Title Color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    showLabels: {
      type: 'custom',
      label: 'Show direction labels',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={value !== false} onChange={e => onChange(e.target.checked)} />
          Show "Previous Post" / "Next Post" labels
        </label>
      ),
    },
  },
  defaultProps: {
    padding: '24px 40px',
    bgColor: '#ffffff',
    cardBgColor: '#f9fafb',
    borderColor: '#e5e7eb',
    labelColor: '#6b7280',
    titleColor: '#111827',
    showLabels: true,
  },
  render: ({ bgColor, padding, cardBgColor, borderColor, labelColor, titleColor, showLabels }) => {
    const cardStyle = {
      flex: 1,
      background: cardBgColor,
      border: `1px solid ${borderColor}`,
      borderRadius: 8,
      padding: '16px 20px',
      minWidth: 0,
    }
    return (
      <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
        <div style={{ maxWidth: 800, margin: '0 auto', display: 'flex', gap: 16 }}>
          {/* Previous */}
          <div style={cardStyle}>
            {showLabels !== false && (
              <div style={{ color: labelColor, fontSize: 12, marginBottom: 6 }}>← Previous Post</div>
            )}
            <div style={{ color: titleColor, fontWeight: 600, fontSize: 15, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              An older post title
            </div>
          </div>
          {/* Next */}
          <div style={{ ...cardStyle, textAlign: 'right' }}>
            {showLabels !== false && (
              <div style={{ color: labelColor, fontSize: 12, marginBottom: 6 }}>Next Post →</div>
            )}
            <div style={{ color: titleColor, fontWeight: 600, fontSize: 15, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              A newer post title
            </div>
          </div>
        </div>
      </div>
    )
  },
}

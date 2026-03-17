import { ColorField, PADDING_OPTIONS } from './ColorField'

export const SearchBlock = {
  label: 'Search',
  fields: {
    label: { type: 'text', label: 'Button label' },
    padding: {
      type: 'select',
      label: 'Outer padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    buttonColor: {
      type: 'custom',
      label: 'Button color',
      render: ({ value, onChange }) => (
        <ColorField label="Button color" value={value ?? '#2563eb'} onChange={onChange} />
      ),
    },
    fieldTextColor: {
      type: 'custom',
      label: 'Input text color',
      render: ({ value, onChange }) => (
        <ColorField label="Input text color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    fieldBgColor: {
      type: 'custom',
      label: 'Input background',
      render: ({ value, onChange }) => (
        <ColorField label="Input background" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    label: 'Search',
    padding: '32px 40px',
    buttonColor: '#2563eb',
    fieldTextColor: '#111827',
    fieldBgColor: '#ffffff',
  },
  render: ({ label, padding, buttonColor, fieldTextColor, fieldBgColor }) => (
    <div style={{ padding, boxSizing: 'border-box', width: '100%' }}>
      <div style={{ maxWidth: 600, margin: '0 auto', display: 'flex', gap: 8 }}>
        <input
          type="text"
          placeholder="Search…"
          style={{
            flex: 1, padding: '10px 16px', fontSize: 15,
            color: fieldTextColor, background: fieldBgColor,
            border: '1px solid #d1d5db', borderRadius: 6, boxSizing: 'border-box',
          }}
        />
        <button style={{
          padding: '10px 20px', background: buttonColor, color: '#fff',
          border: 'none', borderRadius: 6, fontWeight: 600, cursor: 'pointer', fontSize: 15,
        }}>
          {label}
        </button>
      </div>
    </div>
  ),
}

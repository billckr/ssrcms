import { ColorField, PADDING_OPTIONS } from './ColorField'

export const HeaderBlock = {
  label: 'Header',
  fields: {
    bgColor: {
      type: 'custom',
      label: 'Background Color',
      render: ({ value, onChange }) => (
        <ColorField label="Background Color" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    padding: {
      type: 'select',
      label: 'Inner Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
  },
  defaultProps: {
    bgColor: '#ffffff',
    padding: '32px 40px',
  },
  render: ({ bgColor, padding }) => (
    <header style={{
      background: bgColor,
      padding,
      boxSizing: 'border-box',
      width: '100%',
      borderBottom: '1px solid rgba(0,0,0,0.08)',
    }}>
      <div style={{ maxWidth: 1200, margin: '0 auto', minHeight: 40 }} />
    </header>
  ),
}

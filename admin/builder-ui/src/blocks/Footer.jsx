import { ColorField, PADDING_OPTIONS } from './ColorField'

export const FooterBlock = {
  label: 'Footer',
  fields: {
    bgColor: {
      type: 'custom',
      label: 'Background Color',
      render: ({ value, onChange }) => (
        <ColorField label="Background Color" value={value ?? '#1a1a2e'} onChange={onChange} />
      ),
    },
    padding: {
      type: 'select',
      label: 'Inner Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
  },
  defaultProps: {
    bgColor: '#1a1a2e',
    padding: '32px 40px',
  },
  render: ({ bgColor, padding }) => (
    <footer style={{
      background: bgColor,
      padding,
      boxSizing: 'border-box',
      width: '100%',
      borderTop: '1px solid rgba(255,255,255,0.08)',
    }}>
      <div style={{ maxWidth: 1200, margin: '0 auto', minHeight: 40 }} />
    </footer>
  ),
}

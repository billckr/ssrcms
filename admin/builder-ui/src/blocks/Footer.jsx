import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS } from './ColorField'

const COLUMN_OPTIONS = [
  { label: '1 Column',  value: 1 },
  { label: '2 Columns', value: 2 },
  { label: '3 Columns', value: 3 },
]

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

export const FooterBlock = {
  label: 'Footer',
  fields: {
    columns: {
      type: 'select',
      label: 'Columns',
      options: COLUMN_OPTIONS,
    },
    gap: {
      type: 'select',
      label: 'Gap between columns',
      options: GAP_OPTIONS,
    },
    padding: {
      type: 'select',
      label: 'Inner Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    bgColor: {
      type: 'custom',
      label: 'Background Color',
      render: ({ value, onChange }) => (
        <ColorField label="Background Color" value={value ?? '#1a1a2e'} onChange={onChange} />
      ),
    },
    fixed: {
      type: 'custom',
      label: 'Fixed to bottom of viewport',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
          Fixed footer (always visible)
        </label>
      ),
    },
  },
  defaultProps: {
    columns: 1,
    gap: '24px',
    bgColor: '#1a1a2e',
    padding: '32px 40px',
    fixed: false,
  },
  render: ({ bgColor, padding, columns, gap, fixed }) => {
    const cols = Number(columns) || 1
    return (
      <footer style={{
        background: bgColor,
        padding,
        boxSizing: 'border-box',
        width: '100%',
        borderTop: '1px solid rgba(255,255,255,0.08)',
      }}>
        <div style={{
          maxWidth: 1200,
          margin: '0 auto',
          display: 'grid',
          gridTemplateColumns: `repeat(${cols}, 1fr)`,
          gap,
          minHeight: 40,
        }}>
          {Array.from({ length: cols }, (_, i) => (
            <div key={i} style={{ minHeight: 40 }}>
              <DropZone zone={`col${i}`} />
            </div>
          ))}
        </div>
      </footer>
    )
  },
}

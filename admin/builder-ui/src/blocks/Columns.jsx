import { DropZone } from '@measured/puck'
import { ColorField, PADDING_OPTIONS } from './ColorField'

const COLUMN_OPTIONS = [
  { label: '2 Columns', value: 2 },
  { label: '3 Columns', value: 3 },
]

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

export const ColumnsBlock = {
  label: 'Columns',
  fields: {
    columns: {
      type: 'select',
      label: 'Number of columns',
      options: COLUMN_OPTIONS,
    },
    gap: {
      type: 'select',
      label: 'Gap between columns',
      options: GAP_OPTIONS,
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
  },
  defaultProps: {
    columns: 2,
    gap: '24px',
    padding: '32px 40px',
    bgColor: '#ffffff',
  },
  render: ({ columns, gap, padding, bgColor }) => {
    const cols = Number(columns) || 2
    return (
      <div style={{
        background: bgColor,
        padding,
        boxSizing: 'border-box',
        width: '100%',
      }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${cols}, 1fr)`,
          gap,
          maxWidth: 1200,
          margin: '0 auto',
        }}>
          {Array.from({ length: cols }, (_, i) => (
            <div key={i} style={{ minHeight: 80 }}>
              <DropZone zone={`col${i}`} />
            </div>
          ))}
        </div>
      </div>
    )
  },
}

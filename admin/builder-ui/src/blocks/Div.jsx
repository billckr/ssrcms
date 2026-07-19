import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS, MAX_WIDTH_OPTIONS } from './ColorField'

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '8px' },
  { label: 'Medium', value: '16px' },
  { label: 'Large',  value: '24px' },
  { label: 'XLarge', value: '40px' },
]

const MIN_HEIGHT_OPTIONS = [
  { label: 'None',           value: '0px' },
  { label: 'Small (80px)',   value: '80px' },
  { label: 'Default (96px)', value: '96px' },
  { label: 'Medium (200px)', value: '200px' },
  { label: 'Large (400px)',  value: '400px' },
  { label: 'Full screen',    value: '100vh' },
]

export const DivBlock = {
  label: 'Div',
  fields: {
    sections: {
      type: 'select',
      label: 'Sections',
      options: [
        { value: '1', label: '1' },
        { value: '2', label: '2' },
        { value: '3', label: '3' },
        { value: '4', label: '4' },
      ],
    },
    direction: {
      type: 'select',
      label: 'Layout',
      options: [
        { value: 'row',    label: 'Side by side (horizontal)' },
        { value: 'column', label: 'Stacked (vertical)' },
      ],
    },
    gap: {
      type: 'select',
      label: 'Gap between sections',
      options: GAP_OPTIONS,
    },
    minHeight: {
      type: 'select',
      label: 'Min height when empty',
      options: MIN_HEIGHT_OPTIONS,
    },
    bgColor: {
      type: 'custom',
      label: 'Background color',
      render: ({ value, onChange }) => (
        <ColorField label="Background color" value={value ?? 'transparent'} onChange={onChange} />
      ),
    },
    padding: {
      type: 'select',
      label: 'Outer padding',
      options: PADDING_OPTIONS,
    },
    maxWidth: {
      type: 'select',
      label: 'Content max width',
      options: MAX_WIDTH_OPTIONS,
    },
  },
  defaultProps: {
    sections:  '1',
    direction: 'row',
    gap:       '0px',
    minHeight: '96px',
    bgColor:   'transparent',
    padding:   '32px 40px',
    maxWidth:  '1200px',
  },
  render: ({ sections, direction, gap, minHeight, bgColor, padding, maxWidth }) => {
    const count = parseInt(sections) || 1
    return (
      <div style={{ background: bgColor, padding, boxSizing: 'border-box' }}>
        <div style={{
          display:       'flex',
          flexDirection: direction,
          flexWrap:      direction === 'row' ? 'wrap' : 'nowrap',
          gap,
          maxWidth,
          margin:        '0 auto',
          width:         '100%',
          boxSizing:     'border-box',
        }}>
          {Array.from({ length: count }, (_, i) => (
            <div key={i} style={{ flex: '1 1 200px' }}>
              <DropZone zone={`zone${i}`} minEmptyHeight={minHeight} />
            </div>
          ))}
        </div>
      </div>
    )
  },
}

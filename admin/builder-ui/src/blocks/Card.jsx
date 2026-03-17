import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS } from './ColorField'

const CARD_COUNT_OPTIONS = [
  { label: '1 Card',  value: 1 },
  { label: '2 Cards', value: 2 },
  { label: '3 Cards', value: 3 },
  { label: '4 Cards', value: 4 },
]

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

const BORDER_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Thin',   value: '1px' },
  { label: 'Medium', value: '2px' },
  { label: 'Thick',  value: '4px' },
]

const RADIUS_OPTIONS = [
  { label: 'Square',  value: '0px' },
  { label: 'Rounded', value: '12px' },
]

export const CardBlock = {
  label: 'Cards',
  fields: {
    cards: {
      type: 'select',
      label: 'Number of cards',
      options: CARD_COUNT_OPTIONS,
    },
    gap: {
      type: 'select',
      label: 'Gap between cards',
      options: GAP_OPTIONS,
    },
    padding: {
      type: 'select',
      label: 'Outer padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    cardBgColor: {
      type: 'custom',
      label: 'Card background',
      render: ({ value, onChange }) => (
        <ColorField label="Card background" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    cardTextColor: {
      type: 'custom',
      label: 'Card text color',
      render: ({ value, onChange }) => (
        <ColorField label="Card text color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
    radius: {
      type: 'select',
      label: 'Corner style',
      options: RADIUS_OPTIONS,
    },
    borderWidth: {
      type: 'select',
      label: 'Border thickness',
      options: BORDER_OPTIONS,
    },
    borderColor: {
      type: 'custom',
      label: 'Border color',
      render: ({ value, onChange }) => (
        <ColorField label="Border color" value={value ?? '#e2e8f0'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    cards: 2,
    gap: '24px',
    padding: '32px 40px',
    cardBgColor: '#ffffff',
    cardTextColor: '#111827',
    radius: '12px',
    borderWidth: '1px',
    borderColor: '#e2e8f0',
  },
  render: ({ cards, gap, padding, cardBgColor, cardTextColor, radius, borderWidth, borderColor }) => {
    const count = Number(cards) || 2
    const border = borderWidth && borderWidth !== '0px'
      ? `${borderWidth} solid ${borderColor}`
      : 'none'
    return (
      <div style={{ padding, boxSizing: 'border-box', width: '100%' }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${count}, 1fr)`,
          gap,
          maxWidth: 1200,
          margin: '0 auto',
        }}>
          {Array.from({ length: count }, (_, i) => (
            <div key={i} style={{
              background: cardBgColor,
              color: cardTextColor,
              borderRadius: radius,
              border,
              padding: '24px',
              minHeight: 120,
              boxSizing: 'border-box',
            }}>
              <DropZone zone={`col${i}`} />
            </div>
          ))}
        </div>
      </div>
    )
  },
}

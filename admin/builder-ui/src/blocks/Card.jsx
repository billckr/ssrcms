import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS } from './ColorField'

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

const VALIGN_OPTIONS = [
  { label: 'Top',    value: 'flex-start' },
  { label: 'Middle', value: 'center' },
  { label: 'Bottom', value: 'flex-end' },
]

const CARD_DEFAULT = {
  bgColor:     '#ffffff',
  textColor:   '#111827',
  radius:      '12px',
  borderWidth: '1px',
  borderColor: '#e2e8f0',
  valign:      'flex-start',
}

export const CardBlock = {
  label: 'Cards',
  fields: {
    cards: {
      type: 'array',
      label: 'Cards (add/remove to change count)',
      max: 4,
      min: 1,
      getItemSummary: (_, i) => `Card ${i + 1}`,
      arrayFields: {
        bgColor: {
          type: 'custom',
          label: 'Background',
          render: ({ value, onChange }) => (
            <ColorField label="Background" value={value ?? '#ffffff'} onChange={onChange} />
          ),
        },
        textColor: {
          type: 'custom',
          label: 'Text color',
          render: ({ value, onChange }) => (
            <ColorField label="Text color" value={value ?? '#111827'} onChange={onChange} />
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
        valign: {
          type: 'select',
          label: 'Vertical alignment',
          options: VALIGN_OPTIONS,
        },
      },
      defaultItemProps: { ...CARD_DEFAULT },
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
  },
  defaultProps: {
    cards: [
      { ...CARD_DEFAULT },
      { ...CARD_DEFAULT },
    ],
    gap: '24px',
    padding: '32px 40px',
  },
  render: ({ cards, gap, padding }) => {
    const cardList = cards || []
    const count = cardList.length
    return (
      <div style={{ padding, boxSizing: 'border-box', width: '100%' }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${count}, 1fr)`,
          gap,
          maxWidth: 1200,
          margin: '0 auto',
        }}>
          {cardList.map((card, i) => {
            const border = card.borderWidth && card.borderWidth !== '0px'
              ? `${card.borderWidth} solid ${card.borderColor}`
              : 'none'
            const topSpacer    = card.valign === 'center' || card.valign === 'flex-end'
            const bottomSpacer = card.valign === 'flex-start' || card.valign === 'center'
            return (
              <div key={i} style={{
                background:    card.bgColor,
                color:         card.textColor,
                borderRadius:  card.radius,
                border,
                padding:       '24px',
                minHeight:     160,
                boxSizing:     'border-box',
                display:       'flex',
                flexDirection: 'column',
              }}>
                {topSpacer    && <div style={{ flex: 1 }} />}
                <DropZone zone={`col${i}`} style={{ flex: 'none', height: 'auto' }} />
                {bottomSpacer && <div style={{ flex: 1 }} />}
              </div>
            )
          })}
        </div>
      </div>
    )
  },
}

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
  { label: 'Square',       value: '0px' },
  { label: 'Slightly rounded', value: '6px' },
  { label: 'Rounded',      value: '12px' },
  { label: 'Very rounded', value: '24px' },
]

const VALIGN_OPTIONS = [
  { label: 'Top',    value: 'flex-start' },
  { label: 'Middle', value: 'center' },
  { label: 'Bottom', value: 'flex-end' },
]

const SHADOW_OPTIONS = [
  { label: 'None',    value: 'none' },
  { label: 'Dreamy',  value: '0 1px 2px rgba(0,0,0,0.07),0 2px 4px rgba(0,0,0,0.07),0 4px 8px rgba(0,0,0,0.07),0 8px 16px rgba(0,0,0,0.07),0 16px 32px rgba(0,0,0,0.07),0 32px 64px rgba(0,0,0,0.07)' },
  { label: 'Diffuse', value: '0 1px 1px rgba(0,0,0,0.08),0 2px 2px rgba(0,0,0,0.12),0 4px 4px rgba(0,0,0,0.16),0 8px 8px rgba(0,0,0,0.2)' },
  { label: 'Sharp',   value: '0 1px 1px rgba(0,0,0,0.25),0 2px 2px rgba(0,0,0,0.2),0 4px 4px rgba(0,0,0,0.15),0 8px 8px rgba(0,0,0,0.1),0 16px 16px rgba(0,0,0,0.05)' },
  { label: 'Shorter', value: '0 1px 1px rgba(0,0,0,0.11),0 2px 2px rgba(0,0,0,0.11),0 4px 4px rgba(0,0,0,0.11),0 6px 8px rgba(0,0,0,0.11),0 8px 16px rgba(0,0,0,0.11)' },
  { label: 'Longer',  value: '0 2px 1px rgba(0,0,0,0.09),0 4px 2px rgba(0,0,0,0.09),0 8px 4px rgba(0,0,0,0.09),0 16px 8px rgba(0,0,0,0.09),0 32px 16px rgba(0,0,0,0.09)' },
  { label: 'Level 4', value: '0 1px 1px rgba(0,0,0,0.15),0 2px 2px rgba(0,0,0,0.15),0 4px 4px rgba(0,0,0,0.15),0 8px 8px rgba(0,0,0,0.15)' },
  { label: 'Level 5', value: '0 1px 1px rgba(0,0,0,0.12),0 2px 2px rgba(0,0,0,0.12),0 4px 4px rgba(0,0,0,0.12),0 8px 8px rgba(0,0,0,0.12),0 16px 16px rgba(0,0,0,0.12)' },
  { label: 'Level 6', value: '0 1px 1px rgba(0,0,0,0.11),0 2px 2px rgba(0,0,0,0.11),0 4px 4px rgba(0,0,0,0.11),0 8px 8px rgba(0,0,0,0.11),0 16px 16px rgba(0,0,0,0.11),0 32px 32px rgba(0,0,0,0.11)' },
]

const INNER_PADDING_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '12px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

const MIN_HEIGHT_OPTIONS = [
  { label: 'None',        value: '0px' },
  { label: 'Small (120px)',  value: '120px' },
  { label: 'Medium (200px)', value: '200px' },
  { label: 'Large (300px)',  value: '300px' },
  { label: 'XLarge (400px)', value: '400px' },
]

const MAX_WIDTH_OPTIONS = [
  { label: 'Full',          value: '100%' },
  { label: 'Wide (1400px)', value: '1400px' },
  { label: 'Medium (1200px)', value: '1200px' },
  { label: 'Narrow (900px)', value: '900px' },
]

const COLUMNS_OPTIONS = [
  { label: 'Auto (match card count)', value: '0' },
  { label: '1', value: '1' },
  { label: '2', value: '2' },
  { label: '3', value: '3' },
  { label: '4', value: '4' },
]

const CARD_DEFAULT = {
  bgColor:      '#ffffff',
  textColor:    '#111827',
  radius:       '12px',
  borderWidth:  '1px',
  borderColor:  '#e2e8f0',
  shadow:       'none',
  valign:       'flex-start',
  innerPadding: '24px',
  minHeight:    '160px',
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
        shadow: {
          type: 'select',
          label: 'Shadow',
          options: SHADOW_OPTIONS,
        },
        innerPadding: {
          type: 'select',
          label: 'Inner padding',
          options: INNER_PADDING_OPTIONS,
        },
        minHeight: {
          type: 'select',
          label: 'Min height',
          options: MIN_HEIGHT_OPTIONS,
        },
        valign: {
          type: 'select',
          label: 'Vertical alignment',
          options: VALIGN_OPTIONS,
        },
      },
      defaultItemProps: { ...CARD_DEFAULT },
    },
    columns: {
      type: 'select',
      label: 'Columns',
      options: COLUMNS_OPTIONS,
    },
    gap: {
      type: 'select',
      label: 'Gap between cards',
      options: GAP_OPTIONS,
    },
    sectionBg: {
      type: 'custom',
      label: 'Section background',
      render: ({ value, onChange }) => (
        <ColorField label="Section background" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    maxWidth: {
      type: 'select',
      label: 'Max width',
      options: MAX_WIDTH_OPTIONS,
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
    columns:   '0',
    gap:       '24px',
    sectionBg: '#ffffff',
    maxWidth:  '1200px',
    padding:   '32px 40px',
  },
  render: ({ cards, columns, gap, sectionBg, maxWidth, padding }) => {
    const cardList = cards || []
    const count = cardList.length
    const cols = parseInt(columns) || count
    return (
      <div style={{ background: sectionBg, padding, boxSizing: 'border-box', width: '100%' }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${cols}, 1fr)`,
          gap,
          maxWidth,
          margin: '0 auto',
        }}>
          {cardList.map((card, i) => {
            const border = card.borderWidth && card.borderWidth !== '0px'
              ? `${card.borderWidth} solid ${card.borderColor}`
              : 'none'
            return (
              <div key={i} style={{
                background:   card.bgColor,
                color:        card.textColor,
                borderRadius: card.radius,
                border,
                boxShadow:    card.shadow || 'none',
                padding:      card.innerPadding || '24px',
                minHeight:    card.minHeight || '160px',
                boxSizing:    'border-box',
                display:      'flex',
                flexDirection: 'column',
                justifyContent: card.valign || 'flex-start',
              }}>
                <DropZone zone={`col${i}`} style={{ flex: 'none', height: 'auto' }} />
              </div>
            )
          })}
        </div>
      </div>
    )
  },
}

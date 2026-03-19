import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS, MAX_WIDTH_OPTIONS } from './ColorField'

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

const VALIGN_OPTIONS = [
  { label: 'Top',    value: 'flex-start' },
  { label: 'Middle', value: 'center' },
  { label: 'Bottom', value: 'flex-end' },
]

const COL_DEFAULT = {
  bgColor: 'transparent',
  valign:  'flex-start',
}

export const ColumnsBlock = {
  label: 'Columns',
  fields: {
    columns: {
      type: 'array',
      label: 'Columns (add/remove to change count)',
      max: 3,
      min: 2,
      getItemSummary: (_, i) => `Column ${i + 1}`,
      arrayFields: {
        bgColor: {
          type: 'custom',
          label: 'Background color',
          render: ({ value, onChange }) => (
            <ColorField label="Background color" value={value ?? '#ffffff'} onChange={onChange} />
          ),
        },
        valign: {
          type: 'select',
          label: 'Vertical alignment',
          options: VALIGN_OPTIONS,
        },
      },
      defaultItemProps: { ...COL_DEFAULT },
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
      label: 'Outer background color',
      render: ({ value, onChange }) => (
        <ColorField label="Outer background color" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    maxWidth: {
      type: 'select',
      label: 'Content max width',
      options: MAX_WIDTH_OPTIONS,
    },
  },
  defaultProps: {
    columns: [
      { ...COL_DEFAULT },
      { ...COL_DEFAULT },
    ],
    gap:      '24px',
    padding:  '32px 40px',
    bgColor:  '#ffffff',
    maxWidth: '1200px',
  },
  render: ({ columns, gap, padding, bgColor, maxWidth }) => {
    const colList = columns || []
    const count   = colList.length
    return (
      <div style={{
        background:    bgColor,
        padding,
        boxSizing:     'border-box',
        width:         '100%',
      }}>
        <div style={{
          display:               'grid',
          gridTemplateColumns:   `repeat(${count}, 1fr)`,
          gap,
          maxWidth:              maxWidth || '1200px',
          margin:                '0 auto',
        }}>
          {colList.map((col, i) => {
            const topSpacer    = col.valign === 'center' || col.valign === 'flex-end'
            const bottomSpacer = col.valign === 'flex-start' || col.valign === 'center'
            return (
              <div key={i} style={{
                background:    col.bgColor === 'transparent' ? 'transparent' : col.bgColor,
                minHeight:     80,
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

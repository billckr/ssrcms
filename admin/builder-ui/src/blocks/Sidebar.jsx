import { DropZone } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS, MAX_WIDTH_OPTIONS } from './ColorField'

const SIDEBAR_WIDTH_OPTIONS = Array.from({ length: 9 }, (_, i) => {
  const pct = (i + 1) * 10
  return { label: `${pct}%`, value: `${pct}%` }
})

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

export const SidebarBlock = {
  label: 'Sidebar',
  fields: {
    sidebarSide: {
      type: 'radio',
      label: 'Sidebar side',
      options: [
        { value: 'left',  label: 'Left' },
        { value: 'right', label: 'Right' },
      ],
    },
    sidebarWidth: {
      type: 'select',
      label: 'Sidebar width',
      options: SIDEBAR_WIDTH_OPTIONS,
    },
    gap: {
      type: 'select',
      label: 'Gap',
      options: GAP_OPTIONS,
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
    bgColor: {
      type: 'custom',
      label: 'Background color',
      render: ({ value, onChange }) => (
        <ColorField label="Background color" value={value ?? 'transparent'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    sidebarSide:  'left',
    sidebarWidth: '30%',
    gap:          '24px',
    padding:      '32px 40px',
    maxWidth:     '1200px',
    bgColor:      'transparent',
  },
  render: ({ sidebarSide, sidebarWidth, gap, padding, maxWidth, bgColor }) => {
    const sidebar = <div style={{ width: sidebarWidth, flexShrink: 0 }}><DropZone zone="sidebar" minEmptyHeight="96px" /></div>
    const main    = <div style={{ flex: '1 1 0', minWidth: 0 }}><DropZone zone="main" minEmptyHeight="96px" /></div>

    return (
      <div style={{ background: bgColor, padding, boxSizing: 'border-box', width: '100%' }}>
        <div style={{
          display:   'flex',
          flexDirection: sidebarSide === 'right' ? 'row-reverse' : 'row',
          gap,
          maxWidth,
          margin:    '0 auto',
          flexWrap:  'wrap',
        }}>
          {sidebar}
          {main}
        </div>
      </div>
    )
  },
}

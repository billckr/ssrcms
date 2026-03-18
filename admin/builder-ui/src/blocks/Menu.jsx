import { ColorField, PADDING_OPTIONS } from './ColorField'

const INIT_MENUS = (window.__builderInit?.menus) || []

const GAP_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '12px' },
  { label: 'Medium', value: '24px' },
  { label: 'Large',  value: '40px' },
]

const FONT_SIZE_OPTIONS = [
  { label: 'Small',  value: '13px' },
  { label: 'Medium', value: '15px' },
  { label: 'Large',  value: '18px' },
]

const PLACEHOLDER_ITEMS = ['Home', 'About', 'Blog', 'Contact']

export const MenuBlock = {
  label: 'Menu',
  fields: {
    menuId: {
      type: 'select',
      label: 'Menu',
      options: INIT_MENUS.length
        ? INIT_MENUS.map(m => ({ label: m.name, value: m.id }))
        : [{ label: '(no menus created)', value: '' }],
    },
    orientation: {
      type: 'select',
      label: 'Orientation',
      options: [
        { label: 'Horizontal', value: 'horizontal' },
        { label: 'Vertical',   value: 'vertical' },
      ],
    },
    padding: {
      type: 'select',
      label: 'Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
    gap: {
      type: 'select',
      label: 'Gap between items',
      options: GAP_OPTIONS,
    },
    fontSize: {
      type: 'select',
      label: 'Font Size',
      options: FONT_SIZE_OPTIONS,
    },
    bgColor: {
      type: 'custom',
      label: 'Background Color',
      render: ({ value, onChange }) => (
        <ColorField label="Background Color" value={value ?? 'transparent'} onChange={onChange} />
      ),
    },
    linkColor: {
      type: 'custom',
      label: 'Link Color',
      render: ({ value, onChange }) => (
        <ColorField label="Link Color" value={value ?? '#111111'} onChange={onChange} />
      ),
    },
    hoverColor: {
      type: 'custom',
      label: 'Hover Color',
      render: ({ value, onChange }) => (
        <ColorField label="Hover Color" value={value ?? '#555555'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    menuId: INIT_MENUS[0]?.id ?? '',
    orientation: 'horizontal',
    padding: '0px',
    gap: '24px',
    fontSize: '15px',
    bgColor: 'transparent',
    linkColor: '#111111',
    hoverColor: '#555555',
  },
  render: ({ menuId, orientation, padding, bgColor, linkColor, gap, fontSize }) => {
    const menu = INIT_MENUS.find(m => m.id === menuId)
    const menuName = menu?.name ?? 'Menu'
    const isVertical = orientation === 'vertical'

    return (
      <nav style={{ background: bgColor || 'transparent', padding, boxSizing: 'border-box' }}>
        <div style={{
          display: 'flex',
          flexDirection: isVertical ? 'column' : 'row',
          gap,
          flexWrap: isVertical ? 'nowrap' : 'wrap',
          alignItems: isVertical ? 'flex-start' : 'center',
          listStyle: 'none',
          margin: 0,
          padding: 0,
        }}>
          <span style={{ fontSize: 11, color: '#94a3b8', fontStyle: 'italic', marginRight: isVertical ? 0 : 4 }}>
            [{menuName}]
          </span>
          {PLACEHOLDER_ITEMS.map(item => (
            <span key={item} style={{ fontSize, color: linkColor, cursor: 'pointer' }}>
              {item}
            </span>
          ))}
        </div>
      </nav>
    )
  },
}

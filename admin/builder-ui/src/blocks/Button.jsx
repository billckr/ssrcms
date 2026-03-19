import { useState } from 'react'
import { ColorField, PADDING_OPTIONS } from './ColorField'

const SHAPE_OPTIONS = [
  { value: '0px',   label: 'Square' },
  { value: '4px',   label: 'Slight' },
  { value: '8px',   label: 'Rounded' },
  { value: '999px', label: 'Pill' },
]

export const ButtonBlock = {
  label: 'Button',
  fields: {
    label: { type: 'text', label: 'Label' },
    url:   { type: 'text', label: 'URL' },
    bgColor: {
      type: 'custom', label: 'Button color',
      render: ({ value, onChange }) => <ColorField label="Button color" value={value ?? '#1652F0'} onChange={onChange} />,
    },
    textColor: {
      type: 'custom', label: 'Text color',
      render: ({ value, onChange }) => <ColorField label="Text color" value={value ?? '#ffffff'} onChange={onChange} />,
    },
    customization: {
      type: 'object',
      label: 'Customization',
      objectFields: {
        borderWidth: {
          type: 'select', label: 'Border width',
          options: [
            { value: '0px', label: 'None' },
            { value: '1px', label: '1px' },
            { value: '2px', label: '2px' },
            { value: '5px', label: '5px' },
          ],
        },
        borderColor: {
          type: 'custom', label: 'Border color',
          render: ({ value, onChange }) => <ColorField label="Border color" value={value ?? '#000000'} onChange={onChange} />,
        },
        borderRadius: {
          type: 'select', label: 'Shape',
          options: SHAPE_OPTIONS,
        },
        hoverEffect: {
          type: 'select', label: 'Hover',
          options: [
            { value: 'dim',    label: 'Dim' },
            { value: 'custom', label: 'Custom color' },
            { value: 'none',   label: 'No change' },
          ],
        },
        hoverColor: {
          type: 'custom', label: 'Hover color (when Custom is selected)',
          render: ({ value, onChange }) => <ColorField label="Hover color" value={value ?? '#0a46e4'} onChange={onChange} />,
        },
        hoverAnimation: {
          type: 'select', label: 'Animation',
          options: [
            { value: 'none', label: 'None' },
            { value: 'up',   label: 'Shift up' },
            { value: 'down', label: 'Shift down' },
          ],
        },
      },
    },
    layout: {
      type: 'object',
      label: 'Size & Alignment',
      objectFields: {
        align: {
          type: 'select', label: 'Alignment',
          options: [
            { value: 'left',   label: 'Left' },
            { value: 'center', label: 'Center' },
            { value: 'right',  label: 'Right' },
          ],
        },
        size: {
          type: 'select', label: 'Size',
          options: [
            { value: 'small',  label: 'Small' },
            { value: 'medium', label: 'Medium' },
            { value: 'large',  label: 'Large' },
          ],
        },
        padding: {
          type: 'select', label: 'Outer padding',
          options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
        },
      },
    },
  },
  defaultProps: {
    label:     'Click here',
    url:       '/',
    bgColor:   '#1652F0',
    textColor: '#ffffff',
    customization: {
      borderWidth:    '0px',
      borderColor:    '#000000',
      borderRadius:   '6px',
      hoverEffect:    'dim',
      hoverColor:     '#0a46e4',
      hoverAnimation: 'none',
    },
    layout: {
      align:   'center',
      size:    'medium',
      padding: '16px 24px',
    },
  },
  render: ({ label, url, bgColor, textColor, customization, layout }) => {
    const [hovered, setHovered] = useState(false)
    const { align = 'center', size = 'medium', padding = '16px 24px' } = layout || {}
    const {
      borderWidth = '0px', borderColor = '#000000', borderRadius = '6px',
      hoverEffect = 'dim', hoverColor = '#0a46e4', hoverAnimation = 'none',
    } = customization || {}

    const btnPadding = size === 'small' ? '8px 20px' : size === 'large' ? '18px 40px' : '12px 28px'
    const fontSize   = size === 'small' ? '0.875rem' : size === 'large' ? '1.125rem' : '1rem'
    const hasBorder  = borderWidth && borderWidth !== '0px'

    const baseStyle = {
      display:        'inline-block',
      padding:        btnPadding,
      fontSize,
      fontWeight:     600,
      textDecoration: 'none',
      cursor:         'pointer',
      transition:     'all 0.15s ease',
      background:     bgColor,
      color:          textColor,
      border:         hasBorder ? `${borderWidth} solid ${borderColor}` : 'none',
      borderRadius:   borderRadius,
    }

    const hoverStyle = hovered ? {
      background: hoverEffect === 'custom' ? hoverColor : bgColor,
      filter:     hoverEffect === 'dim'    ? 'brightness(0.85)' : 'none',
      transform:  hoverAnimation === 'up'  ? 'translateY(-3px)' : hoverAnimation === 'down' ? 'translateY(3px)' : 'none',
    } : {}

    return (
      <div style={{ padding, textAlign: align, boxSizing: 'border-box' }}>
        <a
          href={url}
          style={{ ...baseStyle, ...hoverStyle }}
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
        >
          {label}
        </a>
      </div>
    )
  },
}

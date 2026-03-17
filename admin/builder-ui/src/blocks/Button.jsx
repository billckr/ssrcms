import { ColorField } from './ColorField'

export const ButtonBlock = {
  label: 'Button',
  fields: {
    label:   { type: 'text', label: 'Label' },
    url:     { type: 'text', label: 'URL' },
    bgColor: {
      type: 'custom', label: 'Button color',
      render: ({ value, onChange }) => <ColorField label="Button color" value={value ?? '#e2e8f0'} onChange={onChange} />,
    },
    textColor: {
      type: 'custom', label: 'Text color',
      render: ({ value, onChange }) => <ColorField label="Text color" value={value ?? '#1e293b'} onChange={onChange} />,
    },
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
  },
  defaultProps: {
    label:     'Click here',
    url:       '/',
    bgColor:   '#e2e8f0',
    textColor: '#1e293b',
    align:     'center',
    size:      'medium',
  },
  render: ({ label, url, bgColor, textColor, align, size }) => {
    const padding = size === 'small' ? '8px 20px' : size === 'large' ? '18px 40px' : '12px 28px'
    const fontSize = size === 'small' ? '0.875rem' : size === 'large' ? '1.125rem' : '1rem'
    return (
      <div style={{ padding: '16px 24px', textAlign: align, boxSizing: 'border-box' }}>
        <a
          href={url}
          style={{
            display: 'inline-block',
            background: bgColor,
            color: textColor,
            padding,
            fontSize,
            fontWeight: 600,
            borderRadius: 6,
            textDecoration: 'none',
          }}
        >
          {label}
        </a>
      </div>
    )
  },
}

import { HexColorPicker } from 'react-colorful'
import { useState } from 'react'

function ColorField({ value, onChange }) {
  const [open, setOpen] = useState(false)
  return (
    <div style={{ position: 'relative', display: 'inline-block' }}>
      <div
        onClick={() => setOpen(!open)}
        style={{
          width: 32, height: 32, borderRadius: 4, cursor: 'pointer',
          background: value, border: '2px solid #ccc',
        }}
      />
      {open && (
        <div style={{ position: 'absolute', zIndex: 1000, top: 36, left: 0 }}>
          <HexColorPicker color={value} onChange={onChange} />
          <button
            onClick={() => setOpen(false)}
            style={{ marginTop: 8, width: '100%', cursor: 'pointer' }}
          >
            Close
          </button>
        </div>
      )}
    </div>
  )
}

export const HeroBlock = {
  label: 'Hero Section',
  fields: {
    heading:    { type: 'text', label: 'Heading' },
    subheading: { type: 'textarea', label: 'Subheading' },
    bgColor:    { type: 'custom', label: 'Background colour', render: ({ value, onChange }) => <ColorField value={value} onChange={onChange} /> },
    textColor:  { type: 'custom', label: 'Text colour',       render: ({ value, onChange }) => <ColorField value={value} onChange={onChange} /> },
    ctaLabel:   { type: 'text', label: 'Button label' },
    ctaUrl:     { type: 'text', label: 'Button URL' },
    ctaBg:      { type: 'custom', label: 'Button colour',     render: ({ value, onChange }) => <ColorField value={value} onChange={onChange} /> },
    ctaText:    { type: 'custom', label: 'Button text colour',render: ({ value, onChange }) => <ColorField value={value} onChange={onChange} /> },
    minHeight:  { type: 'text', label: 'Min height (e.g. 480px)' },
    align:      { type: 'select', label: 'Alignment', options: [
      { value: 'left', label: 'Left' },
      { value: 'center', label: 'Center' },
      { value: 'right', label: 'Right' },
    ]},
  },
  defaultProps: {
    heading:    'Welcome to our site',
    subheading: 'We build great things. Fast, secure, and beautifully simple.',
    bgColor:    '#1a1a2e',
    textColor:  '#ffffff',
    ctaLabel:   'Get Started',
    ctaUrl:     '/contact',
    ctaBg:      '#e94560',
    ctaText:    '#ffffff',
    minHeight:  '480px',
    align:      'center',
  },
  render: ({ heading, subheading, bgColor, textColor, ctaLabel, ctaUrl, ctaBg, ctaText, minHeight, align }) => (
    <section style={{
      background: bgColor,
      color: textColor,
      minHeight,
      display: 'flex',
      flexDirection: 'column',
      alignItems: align === 'left' ? 'flex-start' : align === 'right' ? 'flex-end' : 'center',
      justifyContent: 'center',
      padding: '60px 40px',
      textAlign: align,
      boxSizing: 'border-box',
    }}>
      <h1 style={{ margin: '0 0 16px', fontSize: '3rem', fontWeight: 700, lineHeight: 1.15 }}>
        {heading}
      </h1>
      <p style={{ margin: '0 0 32px', fontSize: '1.25rem', maxWidth: '640px', opacity: 0.9 }}>
        {subheading}
      </p>
      {ctaLabel && (
        <a
          href={ctaUrl}
          style={{
            background: ctaBg,
            color: ctaText,
            padding: '14px 32px',
            borderRadius: '6px',
            textDecoration: 'none',
            fontWeight: 600,
            fontSize: '1rem',
          }}
        >
          {ctaLabel}
        </a>
      )}
    </section>
  ),
}

import { ColorField } from './ColorField'

export const HeroBlock = {
  label: 'Hero Section',
  fields: {
    heading:    { type: 'text', label: 'Heading' },
    subheading: { type: 'textarea', label: 'Subheading' },
    align:      { type: 'select', label: 'Alignment', options: [
      { value: 'left', label: 'Left' },
      { value: 'center', label: 'Center' },
      { value: 'right', label: 'Right' },
    ]},
    bgColor:    { type: 'custom', label: 'Background color', render: ({ value, onChange }) => <ColorField label="Background color" value={value} onChange={onChange} /> },
    textColor:  { type: 'custom', label: 'Text color',       render: ({ value, onChange }) => <ColorField label="Text color"       value={value} onChange={onChange} /> },
    showButton: { type: 'radio', label: 'Show button', options: [{ value: true, label: 'Yes' }, { value: false, label: 'No' }] },
    ctaLabel:   { type: 'text', label: 'Button label' },
    ctaUrl:     { type: 'text', label: 'Button URL' },
    ctaBg:      { type: 'custom', label: 'Button color',      render: ({ value, onChange }) => <ColorField label="Button color"      value={value} onChange={onChange} /> },
    ctaText:    { type: 'custom', label: 'Button text color', render: ({ value, onChange }) => <ColorField label="Button text color" value={value} onChange={onChange} /> },
    minHeight:  { type: 'select', label: 'Min height', options: [
      { value: '0px',   label: 'None' },
      { value: '100px', label: 'XSmall (100px)' },
      { value: '320px', label: 'Small (320px)' },
      { value: '480px', label: 'Medium (480px)' },
      { value: '600px', label: 'Large (600px)' },
      { value: '100vh', label: 'Full screen' },
    ]},
    paddingV:   { type: 'select', label: 'Vertical padding (top/bottom)', options: [
      { value: '0px',   label: 'None' },
      { value: '16px',  label: 'XSmall (16px)' },
      { value: '32px',  label: 'Small (32px)' },
      { value: '60px',  label: 'Medium (60px)' },
      { value: '80px',  label: 'Large (80px)' },
      { value: '120px', label: 'XLarge (120px)' },
    ]},
    paddingH:   { type: 'select', label: 'Horizontal padding (left/right)', options: [
      { value: '0px',  label: 'None' },
      { value: '16px', label: 'Small (16px)' },
      { value: '40px', label: 'Medium (40px)' },
      { value: '80px', label: 'Large (80px)' },
    ]},
  },
  defaultProps: {
    heading:    'Welcome to our site',
    subheading: 'We build great things. Fast, secure, and beautifully simple.',
    bgColor:    '#1a1a2e',
    textColor:  '#ffffff',
    showButton: true,
    ctaLabel:   'Get Started',
    ctaUrl:     '/contact',
    ctaBg:      '#e94560',
    ctaText:    '#ffffff',
    minHeight:  '480px',
    paddingV:   '60px',
    paddingH:   '40px',
    align:      'center',
  },
  render: ({ heading, subheading, bgColor, textColor, showButton, ctaLabel, ctaUrl, ctaBg, ctaText, minHeight, paddingV, paddingH, align }) => (
    <section style={{
      background: bgColor,
      color: textColor,
      minHeight,
      display: 'flex',
      flexDirection: 'column',
      alignItems: align === 'left' ? 'flex-start' : align === 'right' ? 'flex-end' : 'center',
      justifyContent: 'center',
      padding: `${paddingV || '60px'} ${paddingH || '40px'}`,
      textAlign: align,
      boxSizing: 'border-box',
    }}>
      <h1 style={{ margin: '0 0 16px', fontSize: '3rem', fontWeight: 700, lineHeight: 1.15 }}>
        {heading}
      </h1>
      <p style={{ margin: '0 0 32px', fontSize: '1.25rem', maxWidth: '640px', opacity: 0.9 }}>
        {subheading}
      </p>
      {showButton && ctaLabel && (
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

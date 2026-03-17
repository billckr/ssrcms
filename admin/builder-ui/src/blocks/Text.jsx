import { ColorField } from './ColorField'

const ALIGN_OPTIONS = [
  { label: 'Left',   value: 'left' },
  { label: 'Center', value: 'center' },
  { label: 'Right',  value: 'right' },
]

const SIZE_OPTIONS = [
  { label: 'Small',  value: '14px' },
  { label: 'Medium', value: '16px' },
  { label: 'Large',  value: '20px' },
  { label: 'XLarge', value: '28px' },
]

export const TextBlock = {
  label: 'Text',
  fields: {
    text: { type: 'textarea', label: 'Text' },
    align: {
      type: 'select',
      label: 'Alignment',
      options: ALIGN_OPTIONS,
    },
    fontSize: {
      type: 'select',
      label: 'Font size',
      options: SIZE_OPTIONS,
    },
    textColor: {
      type: 'custom',
      label: 'Text color',
      render: ({ value, onChange }) => (
        <ColorField label="Text color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    text: 'Your text here',
    align: 'left',
    fontSize: '16px',
    textColor: '#111827',
  },
  render: ({ text, align, fontSize, textColor }) => (
    <p style={{
      textAlign: align,
      fontSize,
      color: textColor,
      margin: 0,
      whiteSpace: 'pre-wrap',
    }}>
      {text}
    </p>
  ),
}

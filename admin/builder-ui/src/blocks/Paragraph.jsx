import { RichTextMenu } from '@puckeditor/core'
import { ColorField, PADDING_OPTIONS } from './ColorField'

export const ParagraphBlock = {
  label: 'Paragraph',
  fields: {
    text: {
      type: 'richtext',
      renderMenu: () => (
        <RichTextMenu>
          <RichTextMenu.Group>
            <RichTextMenu.HeadingSelect />
          </RichTextMenu.Group>
          <RichTextMenu.Group>
            <RichTextMenu.Bold />
            <RichTextMenu.Italic />
            <RichTextMenu.Underline />
            <RichTextMenu.Strikethrough />
          </RichTextMenu.Group>
          <RichTextMenu.Group>
            <RichTextMenu.AlignSelect />
          </RichTextMenu.Group>
          <RichTextMenu.Group>
            <RichTextMenu.BulletList />
            <RichTextMenu.OrderedList />
          </RichTextMenu.Group>
        </RichTextMenu>
      ),
    },
    fontSize: {
      type: 'select',
      label: 'Font size',
      options: [
        { value: '0.875rem', label: 'Small (14px)' },
        { value: '1rem',     label: 'Normal (16px)' },
        { value: '1.125rem', label: 'Medium (18px)' },
        { value: '1.25rem',  label: 'Large (20px)' },
        { value: '1.5rem',   label: 'XLarge (24px)' },
      ],
    },
    lineHeight: {
      type: 'select',
      label: 'Line height',
      options: [
        { value: '1.4', label: 'Tight (1.4)' },
        { value: '1.6', label: 'Normal (1.6)' },
        { value: '1.8', label: 'Relaxed (1.8)' },
        { value: '2.0', label: 'Loose (2.0)' },
      ],
    },
    maxWidth: {
      type: 'select',
      label: 'Max width',
      options: [
        { value: '100%',   label: 'Full width' },
        { value: '900px',  label: 'Wide (900px)' },
        { value: '720px',  label: 'Medium (720px)' },
        { value: '600px',  label: 'Narrow (600px)' },
        { value: '480px',  label: 'XNarrow (480px)' },
      ],
    },
    align: {
      type: 'select',
      label: 'Alignment',
      options: [
        { value: 'left',    label: 'Left' },
        { value: 'center',  label: 'Center' },
        { value: 'right',   label: 'Right' },
        { value: 'justify', label: 'Justify' },
      ],
    },
    textColor: {
      type: 'custom',
      label: 'Text color',
      render: ({ value, onChange }) => (
        <ColorField label="Text color" value={value ?? '#374151'} onChange={onChange} />
      ),
    },
    bgColor: {
      type: 'custom',
      label: 'Background color',
      render: ({ value, onChange }) => (
        <ColorField label="Background color" value={value ?? '#ffffff'} onChange={onChange} />
      ),
    },
    padding: {
      type: 'select',
      label: 'Padding',
      options: PADDING_OPTIONS.map(o => ({ label: o.label, value: o.value })),
    },
  },
  defaultProps: {
    text:       'Your paragraph text here.',
    fontSize:   '1rem',
    lineHeight: '1.6',
    maxWidth:   '100%',
    align:      'left',
    textColor:  '#374151',
    bgColor:    '#ffffff',
    padding:    '32px 40px',
  },
  render: ({ text, fontSize, lineHeight, maxWidth, align, textColor, bgColor, padding }) => (
    <div style={{ background: bgColor, padding, boxSizing: 'border-box' }}>
      <div
        style={{
          maxWidth,
          margin: '0 auto',
          fontSize,
          lineHeight,
          textAlign: align,
          color: textColor,
        }}
      >
        {text}
      </div>
    </div>
  ),
}

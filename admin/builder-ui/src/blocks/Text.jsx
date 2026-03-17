import { RichTextMenu } from '@puckeditor/core'
import { ColorField } from './ColorField'

export const TextBlock = {
  label: 'Text',
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
    textColor: '#111827',
  },
  render: ({ text, textColor }) => (
    <div style={{ color: textColor }}>{text}</div>
  ),
}

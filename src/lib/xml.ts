/**
 * 轻量 XML 格式化：仅重新缩进元素层级，不改动任何文本/属性内容。
 * - 纯空白文本节点视为缩进丢弃重建
 * - 非空白文本节点原样保留并内联（不额外引入换行，避免改变 XML 文本内容）
 * - 含子元素的元素按层级换行缩进；纯文本元素保持单行
 */
export function formatXml(xml: string): string {
  const lines: string[] = []
  let depth = 0
  let cur = ''
  let lastText = false

  const indentStr = (d: number) => '  '.repeat(d)

  const flush = () => {
    if (cur !== '') {
      lines.push(cur)
      cur = ''
    }
  }

  /** 追加一个标签：跟在文本后则内联；否则另起一行（ownLineIndent 为缩进层级） */
  const pushTag = (tag: string, ownLineIndent: number) => {
    if (cur === '') {
      cur = indentStr(ownLineIndent) + tag
    } else if (lastText) {
      cur += tag
    } else {
      flush()
      cur = indentStr(ownLineIndent) + tag
    }
    lastText = false
  }

  const pushText = (text: string) => {
    cur += text
    lastText = true
  }

  let i = 0
  const n = xml.length
  let inText = false
  let textStart = 0

  const flushText = () => {
    if (!inText) return
    inText = false
    const text = xml.slice(textStart, i)
    if (text.trim() !== '') pushText(text)
  }

  while (i < n) {
    const ch = xml[i]
    if (ch !== '<') {
      if (!inText) {
        inText = true
        textStart = i
      }
      i++
      continue
    }

    flushText()

    // 注释
    if (xml.startsWith('<!--', i)) {
      const end = xml.indexOf('-->', i + 4)
      const raw = end === -1 ? xml.slice(i) : xml.slice(i, end + 3)
      pushTag(raw, depth)
      i += raw.length
      continue
    }
    // CDATA
    if (xml.startsWith('<![CDATA[', i)) {
      const end = xml.indexOf(']]>', i + 9)
      const raw = end === -1 ? xml.slice(i) : xml.slice(i, end + 3)
      pushTag(raw, depth)
      i += raw.length
      continue
    }
    // 处理指令 / XML 声明
    if (xml.startsWith('<?', i)) {
      const end = xml.indexOf('?>', i + 2)
      const raw = end === -1 ? xml.slice(i) : xml.slice(i, end + 2)
      pushTag(raw, depth)
      i += raw.length
      continue
    }
    // 其他 <!...>（如 DOCTYPE），保持原样不改变层级
    if (xml.startsWith('<!', i)) {
      let j = i + 2
      let quote = ''
      while (j < n) {
        const c = xml[j]
        if (quote) {
          if (c === quote) quote = ''
        } else if (c === '"' || c === "'") {
          quote = c
        } else if (c === '>') {
          break
        }
        j++
      }
      const raw = xml.slice(i, Math.min(j + 1, n))
      pushTag(raw, depth)
      i += raw.length
      continue
    }
    // 闭合标签
    if (xml.startsWith('</', i)) {
      const end = xml.indexOf('>', i + 2)
      const raw = xml.slice(i, end === -1 ? n : end + 1)
      if (depth > 0) depth--
      pushTag(raw, depth)
      i += raw.length
      continue
    }
    // 开始标签或自闭合标签：扫描到 ">"，注意引号内的字符
    let j = i + 1
    let quote = ''
    while (j < n) {
      const c = xml[j]
      if (quote) {
        if (c === quote) quote = ''
      } else if (c === '"' || c === "'") {
        quote = c
      } else if (c === '>') {
        break
      }
      j++
    }
    const raw = xml.slice(i, Math.min(j + 1, n))
    const selfClosing = /\/\s*>$/.test(raw)
    pushTag(raw, depth)
    if (!selfClosing) depth++
    i += raw.length
  }
  flushText()
  flush()
  return lines.join('\n')
}

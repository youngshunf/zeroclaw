/**
 * Simple link preprocessor for markdown content.
 * Converts raw URLs to markdown links, skipping code blocks.
 */

const URL_REGEX = /(?<![(\[])https?:\/\/[^\s<>)\]]+/g

interface CodeRange {
  start: number
  end: number
}

function findCodeRanges(text: string): CodeRange[] {
  const ranges: CodeRange[] = []
  const fencedRegex = /```[\s\S]*?```/g
  let match
  while ((match = fencedRegex.exec(text)) !== null) {
    ranges.push({ start: match.index, end: match.index + match[0].length })
  }
  const inlineRegex = /(?<!`)`(?!`)([^`\n]+)`(?!`)/g
  while ((match = inlineRegex.exec(text)) !== null) {
    const pos = match.index
    const insideFenced = ranges.some(r => pos >= r.start && pos < r.end)
    if (!insideFenced) {
      ranges.push({ start: pos, end: pos + match[0].length })
    }
  }
  return ranges
}

function findMarkdownLinkRanges(text: string): CodeRange[] {
  const ranges: CodeRange[] = []
  const inlineLinkRegex = /\[(?:[^\[\]]|\\\[|\\\])*\]\([^)]*\)/g
  let match
  while ((match = inlineLinkRegex.exec(text)) !== null) {
    ranges.push({ start: match.index, end: match.index + match[0].length })
  }
  return ranges
}

function isInRange(pos: number, ranges: CodeRange[]): boolean {
  return ranges.some(r => pos >= r.start && pos < r.end)
}

export function preprocessLinks(text: string): string {
  if (!text.includes('http://') && !text.includes('https://')) {
    return text
  }
  
  const codeRanges = findCodeRanges(text)
  const linkRanges = findMarkdownLinkRanges(text)
  
  URL_REGEX.lastIndex = 0
  let result = ''
  let lastIndex = 0
  let match
  
  while ((match = URL_REGEX.exec(text)) !== null) {
    if (isInRange(match.index, codeRanges) || isInRange(match.index, linkRanges)) continue
    result += text.slice(lastIndex, match.index)
    result += `[${match[0]}](${match[0]})`
    lastIndex = match.index + match[0].length
  }
  
  result += text.slice(lastIndex)
  return result
}

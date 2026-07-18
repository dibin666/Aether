import { describe, expect, it } from 'vitest'

import { OpenAIParser } from '../openai'
import type { MessageRenderBlock, TextRenderBlock } from '../render'

function buildWavBinary(): string {
  const bytes = new Uint8Array(44 + 32_000)
  const view = new DataView(bytes.buffer)
  const write = (offset: number, value: string) => {
    for (let index = 0; index < value.length; index += 1) bytes[offset + index] = value.charCodeAt(index)
  }
  write(0, 'RIFF')
  view.setUint32(4, 36 + 32_000, true)
  write(8, 'WAVEfmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true)
  view.setUint16(22, 1, true)
  view.setUint32(24, 16_000, true)
  view.setUint32(28, 32_000, true)
  view.setUint16(32, 2, true)
  view.setUint16(34, 16, true)
  write(36, 'data')
  view.setUint32(40, 32_000, true)
  return Array.from(bytes, byte => String.fromCharCode(byte)).join('')
}

function requestBody(): Record<string, unknown> {
  const boundary = 'transcription-test-boundary'
  const multipart = [
    `--${boundary}\r\nContent-Disposition: form-data; name="model"\r\n\r\nwhisper-large-v3-turbo\r\n`,
    `--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="sample.wav"\r\nContent-Type: audio/wav\r\n\r\n`,
    buildWavBinary(),
    `\r\n--${boundary}--\r\n`,
  ].join('')
  return { body_bytes_b64: Buffer.from(multipart, 'binary').toString('base64') }
}

function firstMessage(result: ReturnType<OpenAIParser['renderRequest']>): MessageRenderBlock {
  return result.blocks[0] as MessageRenderBlock
}

describe('OpenAI transcription conversation rendering', () => {
  it('renders multipart WAV request metadata', () => {
    const parser = new OpenAIParser()
    const parsed = parser.parseRequest(requestBody())
    const rendered = firstMessage(parser.renderRequest(requestBody()))

    expect(parsed.model).toBe('whisper-large-v3-turbo')
    expect(parsed.messages[0]?.content[0]).toMatchObject({
      type: 'text',
      text: expect.stringContaining('音频文件：sample.wav'),
    })
    expect(rendered.role).toBe('user')
    expect(rendered.roleLabel).toBe('Audio')
    expect((rendered.content[0] as TextRenderBlock).content).toContain('时长：1.00 秒')
    expect(rendered.badges?.map(badge => badge.label)).toContain('whisper-large-v3-turbo')
  })

  it('renders verbose transcription response text and metadata', () => {
    const parser = new OpenAIParser()
    const response = {
      text: '一番だったのに!',
      segments: [{ start: 0, end: 1, text: '一番だったのに!' }],
      transcription_info: { duration: 59.976, language: 'ja' },
      usage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
      word_count: 25,
    }
    const parsed = parser.parseResponse(response)
    const rendered = parser.renderResponse(response).blocks[0] as MessageRenderBlock

    expect(parsed.messages[0]?.content[0]).toEqual({ type: 'text', text: '一番だったのに!' })
    expect(rendered.role).toBe('assistant')
    expect(rendered.roleLabel).toBe('Transcript')
    expect((rendered.content[0] as TextRenderBlock).content).toBe('一番だったのに!')
    expect(rendered.badges?.map(badge => badge.label)).toEqual([
      '语音转录',
      'ja',
      '59.98s',
      '25 words',
    ])
  })
})

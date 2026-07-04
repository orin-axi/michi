import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { renderToon, emptyState, renderHints, truncate } from '../index.js'

void describe('renderToon', () => {
  void it('renders a basic list', () => {
    const out = renderToon({
      typeName: 'issue',
      fields: ['number', 'title', 'state'],
      rows: [
        [
          { type: 'int', intVal: 42 },
          { type: 'str', strVal: 'Fix login' },
          { type: 'str', strVal: 'open' },
        ],
      ],
      totalCount: 100,
      hints: ['Call get_issue with number=<number>'],
    })
    assert.ok(out.startsWith('issue[1]{number,title,state}:'))
    assert.ok(out.includes('42,Fix login,open'))
    assert.ok(out.includes('totalCount: 100'))
    assert.ok(out.includes('help[1]:'))
  })

  void it('renders null values', () => {
    const out = renderToon({
      typeName: 'item',
      fields: ['a', 'b'],
      rows: [[{ type: 'str', strVal: 'x' }, { type: 'null' }]],
      hints: [],
    })
    assert.ok(out.includes('x,'))
  })
})

void describe('emptyState', () => {
  void it('returns empty block', () => {
    const out = emptyState('issue')
    assert.strictEqual(out, 'issue[0]{}:\ntotalCount: 0\n')
  })
})

void describe('renderHints', () => {
  void it('renders hint block', () => {
    const out = renderHints(['hint one', 'hint two'])
    assert.ok(out.startsWith('help[2]:'))
    assert.ok(out.includes('  hint one\n'))
  })

  void it('returns empty for no hints', () => {
    assert.strictEqual(renderHints([]), '')
  })
})

void describe('truncate', () => {
  void it('returns short content unchanged', () => {
    assert.strictEqual(truncate('hello', 100, 'full=true'), 'hello')
  })

  void it('truncates long content', () => {
    const out = truncate('a'.repeat(200), 50, 'full=true')
    assert.ok(out.includes('chars truncated'))
  })
})

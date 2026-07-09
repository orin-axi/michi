import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { renderToon, emptyState, renderHints, truncate, AgentResponse } from '../index.js'

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

  void it('throws a catchable JS error instead of hanging on an oversized rows array', () => {
    const rows = Array.from({ length: 100_001 }, () => [{ type: 'null' }])
    assert.throws(() => renderToon({ typeName: 'item', fields: ['a'], rows, hints: [] }), /rows length/)
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

void describe('AgentResponse', () => {
  void it('builds a TOON response with hints via chained calls', () => {
    const r = new AgentResponse('issues')
    r.items([[{ type: 'int', intVal: 1 }]], ['id'])
    r.hint('do this')
    const out = r.renderToon()
    assert.ok(out.startsWith('issues[1]{id}:'))
    assert.ok(out.includes('help[1]:'))
  })

  void it('builds a KV response', () => {
    const r = new AgentResponse('issue')
    r.kvItems([{ key: 'id', value: { type: 'int', intVal: 42 } }])
    assert.ok(r.renderKv().includes('id:'))
  })

  void it('renderJson reflects asError', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    r.asError()
    assert.ok(r.renderJson().includes('"isError":true'))
  })

  void it('mutators keep working after render calls (render takes &self, not &mut self)', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    r.renderKv()
    r.hint('still works')
    assert.ok(r.renderHintsOnly().includes('still works'))
  })
})

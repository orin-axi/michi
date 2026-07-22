import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { renderToon, emptyState, renderHints, appendHints, renderRecovery, truncate, AgentResponse } from '../index.js'

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

void describe('appendHints', () => {
  void it('appends a help block to an existing body', () => {
    assert.strictEqual(appendHints('body\n', ['do this']), 'body\nhelp[1]:\n  do this\n')
  })
})

void describe('renderRecovery', () => {
  void it('renders a recovery block', () => {
    const out = renderRecovery([{ tool: 'retry', reason: 'rate limited' }])
    assert.ok(out.startsWith('recovery[1]:\n  retry'))
    assert.ok(out.includes('rate limited'))
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

  void it('renderToon/renderKv are slot-specific, not last-call-wins', () => {
    const r = new AgentResponse('issues')
    r.items([[{ type: 'int', intVal: 1 }]], ['id'])
    // kvItems() is called last, but renderToon() must still read the
    // items/fields slot, not follow the last-populated slot.
    r.kvItems([{ key: 'id', value: { type: 'int', intVal: 99 } }])
    const toon = r.renderToon()
    const kv = r.renderKv()
    assert.ok(toon.startsWith('issues[1]{id}:'), `got: ${toon}`)
    assert.ok(kv.includes('id: 99'), `got: ${kv}`)
    assert.notStrictEqual(toon, kv)
  })
})

void describe('toCallToolResult', () => {
  void it('returns MCP-conformant content blocks with type/annotations.audience', () => {
    const r = new AgentResponse('issue')
    r.kvItems([{ key: 'id', value: { type: 'int', intVal: 1 } }])
    const result = r.toCallToolResult()
    assert.strictEqual(result.content.length, 1)
    assert.strictEqual(result.content[0].type, 'text')
    assert.deepStrictEqual(result.content[0].annotations.audience, ['assistant'])
    assert.strictEqual(result.isError, false)
    assert.strictEqual(typeof result.structuredContent, 'object')
    assert.strictEqual(result.structuredContent.isError, false)
  })

  void it('reflects isError and includes a user-audience block from humanContent', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    r.asError()
    r.humanContent('friendly summary')
    const result = r.toCallToolResult()
    assert.strictEqual(result.isError, true)
    assert.strictEqual(result.structuredContent.isError, true)
    assert.strictEqual(result.content.length, 2)
    assert.strictEqual(result.content[1].type, 'text')
    assert.deepStrictEqual(result.content[1].annotations.audience, ['user'])
  })
})

void describe('renderFor / hasHumanContent', () => {
  void it('assistant matches the agent rendering', () => {
    const r = new AgentResponse('issue')
    r.kvItems([{ key: 'id', value: { type: 'int', intVal: 1 } }])
    assert.strictEqual(r.renderFor('assistant'), r.renderKv())
  })

  void it('user returns humanContent when set', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    r.humanContent('hi there')
    assert.strictEqual(r.renderFor('user'), 'hi there')
    assert.strictEqual(r.hasHumanContent(), true)
  })

  void it('user falls back to agent rendering when humanContent was never set', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    assert.strictEqual(r.hasHumanContent(), false)
    assert.strictEqual(r.renderFor('user'), r.renderKv())
  })

  void it('rejects an unknown audience', () => {
    const r = new AgentResponse('t')
    r.kvItems([])
    assert.throws(() => r.renderFor('nonsense'), /nonsense/)
  })
})

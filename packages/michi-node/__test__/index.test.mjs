import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  renderToon,
  emptyState,
  renderHints,
  appendHints,
  renderRecovery,
  truncate,
  AgentResponse,
  renderKv,
  renderAlreadyDone,
  parseRetryAfter,
  nextRetryDelay,
  isRetryableStatus,
  renderDomainError,
  renderStatus,
} from '../index.js'

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

void describe('standalone helpers', () => {
  void it('renderKv formats key-value block', () => {
    const out = renderKv(
      [{ key: 'name', value: { type: 'str', strVal: 'michi' } }],
      1,
      ['hint 1']
    )
    assert.ok(out.includes('name:'))
    assert.ok(out.includes('michi'))
    assert.ok(out.includes('totalCount: 1'))
    assert.ok(out.includes('help[1]:'))
  })

  void it('renderAlreadyDone formats no-op block', () => {
    const out = renderAlreadyDone('create_issue', 'Already exists', ['view issue 42'])
    assert.ok(out.includes('status:    already_done'))
    assert.ok(out.includes('summary:   Already exists'))
    assert.ok(out.includes('help[1]:'))
  })

  void it('parseRetryAfter parses integer seconds', () => {
    assert.strictEqual(parseRetryAfter('120'), 120)
  })

  void it('nextRetryDelay calculates exponential backoff', () => {
    const delay = nextRetryDelay(3, 100, 1000, 0.0, 0.5, 0)
    assert.strictEqual(typeof delay, 'number')
    assert.ok(delay >= 100)
  })

  void it('nextRetryDelay throws on NaN delay input', () => {
    assert.throws(() => nextRetryDelay(3, NaN, 1000, 0.0, 0.5, 0), /finite non-negative/)
  })

  void it('nextRetryDelay throws on negative delay input', () => {
    assert.throws(() => nextRetryDelay(3, -1, 1000, 0.0, 0.5, 0), /finite non-negative/)
  })

  void it('nextRetryDelay throws on Infinity delay input', () => {
    assert.throws(() => nextRetryDelay(3, Infinity, 1000, 0.0, 0.5, 0), /finite non-negative/)
  })

  void it('isRetryableStatus identifies 429 and 503 as retryable', () => {
    assert.strictEqual(isRetryableStatus(429), true)
    assert.strictEqual(isRetryableStatus(503), true)
    assert.strictEqual(isRetryableStatus(404), false)
  })

  void it('renderDomainError formats error card and github annotation', () => {
    const card = renderDomainError('not_found', 'Item not found', ['check ID'])
    assert.ok(card.includes('error: not_found'))
    assert.ok(card.includes('message: Item not found'))

    const gh = renderDomainError('not_found', 'Item not found', [], true)
    assert.strictEqual(gh, '::error title=not_found::Item not found')
  })

  void it('renderDomainError throws on unknown error code', () => {
    assert.throws(() => renderDomainError('made_up_code', 'msg', []), /unknown error code/)
  })

  void it('renderStatus formats orientation block', () => {
    const out = renderStatus('my_tool', 'Does work', [
      { key: 'cache', value: { type: 'str', strVal: 'ready' }, health: 'ok' },
    ])
    assert.ok(out.includes('tool:        my_tool'))
    assert.ok(out.includes('description: Does work'))
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

void describe('int64 boundary (SPEC-NAPI-POINTFIX-001)', () => {
  void it('AC-001 renders a positive int beyond i32::MAX losslessly', () => {
    const out = renderToon({ typeName: 't', fields: ['a'], rows: [[{ type: 'int', intVal: 1755000000000 }]], hints: [] })
    assert.strictEqual(out, 't[1]{a}:\n  1755000000000\n')
  })
  void it('AC-002 renders a negative int beyond i32::MIN losslessly', () => {
    const out = renderToon({ typeName: 't', fields: ['a'], rows: [[{ type: 'int', intVal: -1755000000000 }]], hints: [] })
    assert.strictEqual(out, 't[1]{a}:\n  -1755000000000\n')
  })
  void it('AC-003 renderKv renders an int beyond i32::MAX losslessly', () => {
    const out = renderKv([{ key: 'id', value: { type: 'int', intVal: 1755000000000 } }], null, [])
    assert.strictEqual(out, 'id: 1755000000000\n')
  })
  void it('AC-008 truncates a non-integer intVal toward zero with no error', () => {
    let out
    assert.doesNotThrow(() => {
      out = renderToon({ typeName: 't', fields: ['a'], rows: [[{ type: 'int', intVal: 1.5 }]], hints: [] })
    })
    assert.strictEqual(out, 't[1]{a}:\n  1\n')
  })
})

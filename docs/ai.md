# AI assistance

The AI pass is **opt-in**. Rule-based conversion runs entirely locally; the AI only gets asked when the rules can't handle a file cleanly.

## When the AI kicks in

Convert feeds a file to the AI pass only if:

- the file has at least one `block`-severity finding **and** no deterministic fix is available, **or**
- the file is on the "hard" list surfaced by Analyze (platform-specific message pumps, custom allocators, hand-rolled IPC, inline assembly, etc.).

For every other file the deterministic path handles everything.

## Providers

| Provider | Setting | Notes |
|---|---|---|
| **Off** | default | No external calls; convert only uses rule-based rewrites |
| **Claude** | `provider = "claude"` + API key | claude.ai Messages API |
| **OpenAI-compatible** | `provider = "openai_compatible"` + base URL + API key | Any server that speaks the OpenAI chat-completions format: OpenAI itself, Azure, Groq, Together, local Ollama, LM Studio, vLLM, llama.cpp server, etc. |

The API key is pulled from the environment at convert time (`CROSSFIRE_AI_KEY`); it's never stored in the project file.

## What the AI gets

For each file: the source, the list of findings on it, the chosen target OS, and a prompt template that insists on:

- Never writing to disk or running commands
- Outputting a unified diff
- Including a self-confidence score in `[0.0, 1.0]`
- Flagging any assumption that needs a human to verify

The diffs are applied by the engine, not the model. The engine refuses to apply a diff whose confidence score is below a threshold you set (default `0.6`).

## What ends up in the report

Every AI-touched file gets an entry in `CROSSFIRE_REPORT.md`:

```
### src/platform/win32_message_pump.cpp

- rule finding: win.window.wndproc  (block)
- AI pass:      claude-5.1-opus      confidence 0.84
- diff:         +42 / -17
- assumes:      SDL event loop will be the top-level dispatcher
```

## Opt-out, mid-port

If you flip AI off after a convert, nothing is re-done; the AI-touched files stay as-is. If you flip it on for a re-convert, only files that have changed since are sent through again.

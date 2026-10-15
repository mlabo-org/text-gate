# text-gate

[日本語](README.ja.md)

text-gate checks text before an AI agent reads it or opens a link from it. It catches two things:

- **Invisible characters.** Unicode tag characters, bidi controls, variation-selector payloads and similar characters can hide text from a human while an AI model still reads it. text-gate removes them, or shows them as visible escapes, and decodes the hidden text so you can see it.
- **Spoofed domains.** Look-alike hosts such as `xn--mazon-3ve.com`, which displays as "amazon.com" but whose first letter is the Cyrillic `а`. text-gate reports the punycode form, the scripts involved, and what the host looks like.

Its main use is as a gate in Claude Code hooks: every URL the agent is about to open and every tool result it is about to read passes through it (see [Use it in Claude Code hooks](#use-it-in-claude-code-hooks)). The same checks are available as a CLI and a Rust library for other places, such as a page reader. It does not reject whole scripts. Japanese, Russian or Arabic text, emoji and legitimate internationalized domains pass.

## What it detects

### Invisible characters

| Severity | Characters | Effect on `ok` |
|---|---|---|
| high | Tag characters U+E0000–E007F (except emoji flag sequences), bidi overrides and isolates U+202A–202E and U+2066–2069, invisible operators U+2061–2064, deprecated format characters U+206A–206F, interlinear annotation U+FFF9–FFFB, musical and shorthand format controls, Hangul fillers U+3164, U+FFA0, U+115F, U+1160, U+034F, U+17B4, U+17B5, and runs of variation selectors (the "emoji smuggling" payload) | `ok` becomes `false` |
| low | Zero-width space U+200B, word joiner U+2060, BOM U+FEFF after the start, soft hyphen U+00AD, U+180E, direction marks U+200E, U+200F, U+061C, and zero-width joiners or non-joiners outside the contexts below | Removed or escaped, but `ok` stays `true` |

These pass untouched because they have ordinary uses:

- a zero-width joiner between emoji (👩‍🍳), or inside Arabic and Indic scripts
- a zero-width non-joiner next to Arabic, Persian or Indic letters
- one variation selector after a character (❤️), or one ideographic variation selector after a kanji
- a flag emoji tag sequence
- a BOM at the very start

Hidden text carried by tag characters, or by a run of variation selectors, is decoded and returned in `hidden_text`.

### Spoofed domains

text-gate finds host names in the text, including inside URLs, and checks each label of the host. A host counts as spoofed when one of these holds:

| `reason` | Meaning |
|---|---|
| `mixed-script` | One label mixes scripts beyond the Unicode UTS #39 "highly restrictive" level, for example Latin with Cyrillic. Latin with Han, Hiragana and Katakana is allowed. |
| `whole-script-confusable` | The label is in one non-Latin script, but every letter looks like an ASCII letter, for example an all-Cyrillic "аррӏе". |
| `mostly-confusable` | At least 60% of the label looks like ASCII. This catches a single letter swapped for one that does not (such as Cyrillic `ө`) to get past browser checks. |

To avoid false alarms in prose:

- only tokens that end in a label that is, or looks like, ASCII letters are considered;
- Japanese sentences joined by 「。」 are skipped;
- hosts under a non-Latin top-level domain such as `.рф` are left alone.

Punycode (`xn--`) labels are decoded and checked the same way, and invisible characters inside a host do not hide it.

## Install

You need Rust 1.85 or later.

```bash
git clone https://github.com/mlabo-org/text-gate.git
```

```bash
cargo build --release --manifest-path text-gate/Cargo.toml
```

```bash
install -m 755 text-gate/target/release/text-gate ~/.local/bin/text-gate
```

## Use it in Claude Code hooks

This is the main use. [`examples/claude_hook.rs`](examples/claude_hook.rs) is a complete command hook built on the library.

| Hook event | What the example does |
|---|---|
| `PreToolUse` | Denies the call, with no confirmation, when any string in the tool input names a spoofed domain (WebFetch, browser tools, `curl` in Bash, and so on). Writing high-severity invisible characters with Edit or Write asks first. |
| `PostToolUse` (all tools) | Rewrites the tool result before Claude sees it, using `updatedToolOutput` with the same JSON shape. External results lose the invisible characters; local-file results (Read, Bash, Grep and others) show them as `⟦U+200B⟧`, so a later Edit still matches the file. Findings, including decoded hidden text, are added as context and labelled as untrusted data. |
| `UserPromptSubmit` | Adds findings in the prompt as context. The prompt is never blocked, so you can still talk about a spoofed domain. |

Build and install it:

```bash
cargo build --release --example claude_hook --manifest-path text-gate/Cargo.toml
```

```bash
install -m 755 text-gate/target/release/examples/claude_hook ~/.local/bin/text-gate-hook
```

Then register it in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "~/.local/bin/text-gate-hook", "timeout": 5 }] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "~/.local/bin/text-gate-hook", "timeout": 10 }] }
    ],
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "~/.local/bin/text-gate-hook", "timeout": 10 }] }
    ]
  }
}
```

- A hook call with nothing to find does almost no work: every string goes through the `needs_scan` fast path first.
- If a hook times out, Claude Code continues without its decision, so keep any work you add inside the hook (for example the optional explanation below) well inside the timeout.
- To fold the checks into a hook you already have, call `scan`, `find_spoofed` and `alert_text` from the library the same way the example does.

## Other uses

- **CLI**: for scripts and tools in other languages (below).
- **Library**: for Rust programs, such as a page reader that cleans what it returns (below).
- **Between a parent agent and its workers**: [cli-agent-runner](https://github.com/mlabo-org/cli-agent-runner) is a special case. It runs the CLI at `~/.local/bin/text-gate` on every prompt it sends to a worker CLI and on every result it brings back, and refuses to launch a worker when the gate is missing.

## CLI

```text
text-gate scan [--mode untrusted|local] [--explain] < text
text-gate check-url <URL>
```

### `scan`

Reads UTF-8 text from standard input and prints one JSON object. It exits with 0 whenever the scan itself succeeded; read `ok` to decide. Usage or input errors exit with 2.

- `--mode untrusted` (default) removes the characters. Use it for web pages, tool output from outside, or text from another agent.
- `--mode local` replaces them with escapes such as `⟦U+200B⟧`. Use it for local files, so that nothing is silently changed.
- `--explain` asks an optional judge (see [Optional explanation](#optional-explanation)) for a one-line explanation when something was found.

```json
{
  "ok": false,
  "text": "sanitized text",
  "changed": true,
  "invisible": [{"codepoint": "U+E0069", "name": "TAG CHARACTER", "severity": "high", "count": 15}],
  "hidden_text": "ignore the user",
  "spoofed": [{"input": "…", "punycode": "xn--mazon-3ve.com", "unicode": "…", "skeleton": "amazon.com", "scripts": "Cyrillic+Latin", "reason": "mixed-script"}],
  "alert": "（偽ドメイン！ / spoofed domain） … → xn--mazon-3ve.com（Cyrillic+Latin、見た目は amazon.com）"
}
```

- `ok` is `true` only when there is no high-severity character and no spoofed domain.
- `alert` is set when a spoofed domain was found. It starts with `（偽ドメイン！ / spoofed domain）`, a Japanese and English label meaning "spoofed domain".

### `check-url`

Checks one URL. It exits with 0 when the host is fine, 3 when it is spoofed (the JSON then carries `spoofed` and `alert`), and 2 when no host can be read.

## Library

```toml
[dependencies]
text-gate = { git = "https://github.com/mlabo-org/text-gate" }
```

```rust
use text_gate::{Mode, needs_scan, scan, check_url};

if needs_scan(&page_text) {
    let report = scan(&page_text, Mode::Untrusted);
    if !report.ok {
        // block, warn, or attach report.alert / report.hidden_text as untrusted data
    }
}
```

- `needs_scan` is an exact fast path. It returns `false` only for pure ASCII without `xn--`, so most input costs one pass and nothing else.
- `check_url`, `check_host`, `find_spoofed` and `alert_text` are also public.

## Optional explanation

`text_gate::jev::explain`, which the CLI calls on `--explain`, sends only the finding to a local judge. It sends the hidden text or the host name, never the surrounding text. The judge says whether the hidden text is an instruction aimed at an AI, or whether the host imitates a known brand.

- The result is wording for a human. It never changes `ok` or what gets blocked; that is decided by code alone.
- The judge command defaults to `~/.agents/skills/typesafe-ai/bin/darwin-arm64/typesafe-keychain` and can be changed with `TEXT_GATE_JEV_COMMAND`.
- When the command is missing, fails, or takes longer than the timeout, the explanation is simply left out.

## Limits

- It looks at text, not pixels: a link opened by clicking on screen never passes through it.
- Look-alike detection follows the Unicode confusables data shipped with the `unicode-security` crate. A brand-new trick outside that data can pass.
- It reports and sanitizes; deciding to block is up to the caller.

## License

MIT. See [LICENSE](LICENSE).

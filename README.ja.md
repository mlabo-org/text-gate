# text-gate

[English](README.md)

text-gate は、AI エージェントが文を読む前や、文の中のリンクを開く前に、その文を調べる道具です。次の2つを見つけます。

- **見えない文字。** タグ文字、文字の向きを変える制御文字、異体字セレクタを並べた隠しデータなどを使うと、人には見えないまま AI にだけ読める文を埋め込めます。text-gate はこれを取り除くか、見える形に置き換え、隠れていた文を元に戻して見せます。
- **偽ドメイン。** たとえば `xn--mazon-3ve.com` は「amazon.com」に見えますが、先頭の「а」はキリル文字です。text-gate は、Punycode の形、混ざっている文字の種類、見た目の形を返します。

本来の使い方は、Claude Code のフックに組み込む門番です。エージェントが開こうとする URL と、読もうとするツールの結果を、すべてここに通します（「Claude Code のフックで使う」を見てください）。同じ検査はコマンドと Rust のライブラリとしても使えるので、ページの読み取り道具などにも組み込めます。文字の種類ごとに一律に拒否することはしません。日本語・ロシア語・アラビア語の文、絵文字、正当な国際化ドメインはそのまま通ります。

## 見つけるもの

### 見えない文字

| 重さ | 文字 | `ok` への影響 |
|---|---|---|
| high | タグ文字 U+E0000〜E007F（旗の絵文字の並びは除く）、文字の向きの上書き・分離 U+202A〜202E と U+2066〜2069、見えない演算子 U+2061〜2064、廃止された書式文字 U+206A〜206F、注釈 U+FFF9〜FFFB、楽譜と速記の書式文字、ハングルの空白文字 U+3164・U+FFA0・U+115F・U+1160、U+034F・U+17B4・U+17B5、異体字セレクタの連続（絵文字に隠しデータを載せる手口） | `ok` が `false` になる |
| low | ゼロ幅スペース U+200B、単語結合子 U+2060、先頭以外の BOM U+FEFF、ソフトハイフン U+00AD、U+180E、文字の向きの印 U+200E・U+200F・U+061C、下の場面以外のゼロ幅接合子・非接合子 | 取り除くか見える形にするが、`ok` は `true` のまま |

次のものは、普通の使い道があるのでそのまま通します。

- 絵文字の間のゼロ幅接合子（👩‍🍳）、アラビア文字やインド系の文字の中のゼロ幅接合子
- アラビア文字・ペルシャ語・インド系の文字の隣のゼロ幅非接合子
- 1文字の後の異体字セレクタ1つ（❤️）、漢字の後の漢字用異体字セレクタ1つ
- 旗の絵文字のタグの並び
- 先頭の BOM

タグ文字や異体字セレクタの連続に隠されていた文は、元に戻して `hidden_text` に入れます。

### 偽ドメイン

文の中（URL の中も含む）からホスト名を拾い、ドット区切りの各部分（ラベル）を調べます。次のどれかに当たれば偽ドメインとします。

| `reason` | 意味 |
|---|---|
| `mixed-script` | 1つのラベルの中で、Unicode の UTS #39 の「highly restrictive」を超えて文字の種類が混ざっている（ラテン文字とキリル文字など）。ラテン文字と漢字・ひらがな・カタカナの組み合わせは正当として通す。 |
| `whole-script-confusable` | ラベルがラテン文字以外の1種類の文字だけでできていて、どの文字も英字に見える（全部キリル文字の「аррӏе」など）。 |
| `mostly-confusable` | ラベルの6割以上が英字に見える。ブラウザの対策をすり抜けるために、1文字だけ英字に見えない文字（キリル文字の `ө` など）に替える手口を拾う。 |

普通の文での誤検出を避けるため、次のようにしています。

- 最後のラベルが英字か、英字に見える文字でできている語だけを調べる。
- 「。」でつながった日本語の文は調べない。
- `.рф` のように英字でない最上位ドメインの下のホストは対象にしない。

`xn--` で始まる Punycode のラベルは元の文字に戻してから同じように調べます。ホスト名の中に見えない文字を挟んでも、隠すことはできません。

## インストール

Rust 1.85 以降が必要です。

```bash
git clone https://github.com/mlabo-org/text-gate.git
```

```bash
cargo build --release --manifest-path text-gate/Cargo.toml
```

```bash
install -m 755 text-gate/target/release/text-gate ~/.local/bin/text-gate
```

## Claude Code のフックで使う

これが本来の使い方です。[`examples/claude_hook.rs`](examples/claude_hook.rs) は、ライブラリを使ったフックの完成例です。

| フックの時点 | 例のフックがすること |
|---|---|
| `PreToolUse` | ツールの入力の文字列のどれかに偽ドメインがあれば、確認なしで止めます（WebFetch、ブラウザーの道具、Bash の `curl` など）。Edit や Write で high の見えない文字を書き込むときは、確認に回します。 |
| `PostToolUse`（全ツール） | Claude が読む前に、`updatedToolOutput` でツールの結果を同じ JSON の形のまま書き換えます。外から来た結果からは見えない文字を取り除きます。手元のファイルの結果（Read、Bash、Grep など）は `⟦U+200B⟧` の形で見せます。そうしないと、次の Edit が元のファイルと一致しなくなるためです。見つけたもの（元に戻した隠し文を含む）は、信頼できないデータと明記して文脈に足します。 |
| `UserPromptSubmit` | 依頼文で見つけたものを文脈に足します。依頼文は止めないので、偽ドメインについて話すこともできます。 |

ビルドして置きます。

```bash
cargo build --release --example claude_hook --manifest-path text-gate/Cargo.toml
```

```bash
install -m 755 text-gate/target/release/examples/claude_hook ~/.local/bin/text-gate-hook
```

`~/.claude/settings.json` に登録します。

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

- 調べるものが無いときは、ほとんど何もしません。どの文字列も、まず `needs_scan` の近道を通ります。
- フックが時間切れになると、Claude Code はその判定なしで先へ進みます。フックに処理を足すとき（下の「説明の追加」など）は、時間切れより十分短く収めてください。
- すでにあるフックに組み込むときは、例と同じようにライブラリの `scan`、`find_spoofed`、`alert_text` を呼びます。

## ほかの使い方

- **コマンド**: ほかの言語のスクリプトや道具から使います（下の「コマンド」）。
- **ライブラリ**: Rust のプログラム、たとえば返す本文を掃除するページの読み取り道具に組み込みます（下の「ライブラリとして使う」）。
- **親エージェントと子のあいだ**: [cli-agent-runner](https://github.com/mlabo-org/cli-agent-runner) は特殊な使い方です。子の CLI に渡す依頼文と、持ち帰る結果のすべてを `~/.local/bin/text-gate` のコマンドに通し、text-gate が無いときは子を起動しません。

## コマンド

```text
text-gate scan [--mode untrusted|local] [--explain] < text
text-gate check-url <URL>
```

### `scan`

標準入力の UTF-8 の文を調べ、JSON を1つ出力します。調べること自体に成功すれば、終了コードは常に 0 です。止めるかどうかは `ok` を見て決めてください。使い方や入力の誤りは 2 で終わります。

- `--mode untrusted`（既定）は、該当する文字を取り除きます。Web ページ、外部からのツールの出力、他のエージェントから来た文に使います。
- `--mode local` は、`⟦U+200B⟧` のような見える形に置き換えます。手元のファイルに使います。黙って中身を変えないためです。
- `--explain` を付けると、何か見つかったときに、任意の判定役（下の「説明の追加（任意）」）に1行の説明を頼みます。

出力の形は [README.md](README.md#scan) の例と同じです。

- `ok` は、high の文字も偽ドメインも無いときだけ `true` です。
- `alert` は偽ドメインがあったときに入り、「（偽ドメイン！ / spoofed domain）」で始まります。

### `check-url`

URL を1つ調べます。終了コードは次のとおりです。

- 問題が無ければ 0
- 偽ドメインなら 3（JSON に `spoofed` と `alert` が入る）
- ホストが読み取れなければ 2

## ライブラリとして使う

```toml
[dependencies]
text-gate = { git = "https://github.com/mlabo-org/text-gate" }
```

```rust
use text_gate::{Mode, needs_scan, scan, check_url};

if needs_scan(&page_text) {
    let report = scan(&page_text, Mode::Untrusted);
    if !report.ok {
        // 止める・警告する、または report.alert や report.hidden_text を信頼できないデータとして添える
    }
}
```

- `needs_scan` は、取りこぼしのない近道です。`false` を返すのは、全部 ASCII で `xn--` を含まないときだけです。なので、ほとんどの入力は1回読み通すだけで終わります。
- `check_url`、`check_host`、`find_spoofed`、`alert_text` も公開しています。

## 説明の追加（任意）

`text_gate::jev::explain`（コマンドでは `--explain`）は、見つかったものだけを手元の判定役に渡します。渡すのは隠れていた文かホスト名だけで、周りの文は渡しません。判定役は、隠れていた文が AI への指示かどうか、ホストが有名なブランドをまねたものかどうかを答えます。

- 返るのは人に見せる説明だけです。`ok` や止めるかどうかは変わらず、それはコードだけで決まります。
- 判定役のコマンドは、既定では `~/.agents/skills/typesafe-ai/bin/darwin-arm64/typesafe-keychain` です。`TEXT_GATE_JEV_COMMAND` で差し替えられます。
- コマンドが無い、失敗した、時間内に終わらなかった、のどれかのときは、説明なしで動きます。

## 限界

- 調べるのは文字で、画面ではありません。画面をクリックして開くリンクは、ここを通りません。
- 見た目が似た文字の判定は、`unicode-security` クレートに入っている Unicode の confusables のデータに従います。そこに無い新しい手口は通る可能性があります。
- text-gate は見つけて無害化するだけで、止めるかどうかは呼び出す側が決めます。

## ライセンス

MIT です。[LICENSE](LICENSE) を見てください。

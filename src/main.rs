//! text-gate CLI。Rust 以外の道具（cli-agent-runner など）から同じ検査を使うための入口。
//!
//!   text-gate scan [--mode untrusted|local] [--explain]   標準入力の文を検査し、結果を JSON で出す（成功時は常に終了コード 0）
//!   text-gate check-url <URL>                             0 = 問題なし、3 = 偽ドメイン、2 = 入力の誤り

use std::io::Read;
use std::process::ExitCode;
use std::time::Duration;
use text_gate::{Mode, alert_text, check_url, jev, scan};

const USAGE: &str = "usage: text-gate scan [--mode untrusted|local] [--explain] < text\n       text-gate check-url <URL>";
const EXPLAIN_TIMEOUT: Duration = Duration::from_secs(3);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => run_scan(&args[1..]),
        Some("check-url") if args.len() == 2 => run_check_url(&args[1]),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn run_scan(args: &[String]) -> ExitCode {
    let mut mode = Mode::Untrusted;
    let mut explain = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--mode" => match iter.next().map(String::as_str) {
                Some("untrusted") => mode = Mode::Untrusted,
                Some("local") => mode = Mode::Local,
                _ => return usage(),
            },
            "--explain" => explain = true,
            _ => return usage(),
        }
    }
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("text-gate: 標準入力を UTF-8 として読めませんでした");
        return ExitCode::from(2);
    }
    let report = scan(&input, mode);
    let mut value = serde_json::to_value(&report).expect("report serializes");
    if explain && !report.ok {
        value["explanation"] = jev::explain(&report, EXPLAIN_TIMEOUT).into();
    }
    println!("{value}");
    ExitCode::SUCCESS
}

fn run_check_url(url: &str) -> ExitCode {
    match check_url(url) {
        Err(error) => {
            eprintln!("text-gate: {error}");
            ExitCode::from(2)
        }
        Ok(None) => {
            println!("{}", serde_json::json!({ "ok": true, "spoofed": [], "alert": null }));
            ExitCode::SUCCESS
        }
        Ok(Some(spoofed)) => {
            let spoofed = vec![spoofed];
            println!("{}", serde_json::json!({ "ok": false, "alert": alert_text(&spoofed), "spoofed": spoofed }));
            ExitCode::from(3)
        }
    }
}

//! Figlio di prova del lanciatore, senza parsing (piano 01-07): op `selftest` del job JSON.
//!
//! Modi: `alloc <MiB>` (alloca e tocca le pagine), `spawn-grandchild` (lancia un `sleep`
//! persistente e lo attende), `flood <byte>` (scrive su stdout e stderr alternati),
//! `argv-echo [<arg>]` (rimanda argv e cwd in `data`), `sleep <s>`, `write <byte>` (scrive un
//! file nella cartella del job). Ogni modo che termina stampa una riga con `protocol::emit`.
use std::io::Write;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use s02_parser::protocol::{ChildOutcome, emit};
use serde_json::json;

const MIB: usize = 1024 * 1024;
const PAGE: usize = 4096;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fake-child: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn num(arg: Option<&String>) -> anyhow::Result<u64> {
    let a = arg.ok_or_else(|| anyhow::anyhow!("argomento numerico mancante"))?;
    Ok(a.parse()?)
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let arg = args.get(2);
    let data = match mode {
        "alloc" => {
            let mib = usize::try_from(num(arg)?)?;
            let mut v = vec![0u8; mib.saturating_mul(MIB)];
            for i in (0..v.len()).step_by(PAGE) {
                v[i] = 1;
            }
            let touched = v.iter().step_by(PAGE).filter(|b| **b == 1).count();
            json!({"allocated_mib": mib, "pages_touched": touched})
        }
        "spawn-grandchild" => {
            let mut gc = Command::new("/bin/sleep")
                .arg("300")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let status = gc.wait()?;
            json!({"grandchild_exit": status.code()})
        }
        "flood" => {
            let total = usize::try_from(num(arg)?)?;
            let chunk = vec![b'x'; 64 * 1024];
            let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
            let mut left = total;
            while left > 0 {
                let k = left.min(chunk.len());
                out.write_all(&chunk[..k])?;
                err.write_all(&chunk[..k])?;
                left -= k;
            }
            out.flush()?;
            drop(out);
            drop(err);
            json!({"flooded": total})
        }
        "argv-echo" => {
            let cwd = std::env::current_dir()?;
            json!({"argv": args, "cwd": cwd.to_string_lossy()})
        }
        "sleep" => {
            let s = num(arg)?;
            std::thread::sleep(Duration::from_secs(s));
            json!({"slept_s": s})
        }
        "write" => {
            let total = usize::try_from(num(arg)?)?;
            let mut f = std::fs::File::create("write.bin")?;
            let chunk = vec![b'w'; 64 * 1024];
            let mut left = total;
            while left > 0 {
                let k = left.min(chunk.len());
                f.write_all(&chunk[..k])?;
                left -= k;
            }
            f.sync_all()?;
            json!({"written": total})
        }
        other => anyhow::bail!("modo sconosciuto: {other:?}"),
    };
    emit(&ChildOutcome::Ok {
        pages: None,
        output: None,
        data: Some(data),
    })
}

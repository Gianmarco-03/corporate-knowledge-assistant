//! Generatore di PDF strutturalmente validi di dimensione totale esatta (RISK-005),
//! deterministico per `(size, seed)`, scritto in streaming.
//!
//! Struttura: header, catalogo, albero delle pagine con una pagina vuota, un oggetto
//! stream non referenziato che porta il riempitivo, tabella xref con offset corretti e
//! trailer. Il riempitivo è esadecimale minuscolo `[0-9a-f]` derivato da
//! sha256(seed ‖ contatore): l'alfabeto non contiene `/`, quindi non crea falsi nomi
//! per il prefiltro di `validate`. La dimensione esatta si ottiene risolvendo la
//! lunghezza dello stream (e, se un salto di cifre lo impedisce, qualche byte di spazio
//! fra due oggetti, ammesso dalla sintassi).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

struct Layout {
    head: Vec<u8>,
    tail: Vec<u8>,
    data_len: u64,
}

impl Layout {
    fn total(&self) -> u64 {
        self.head.len() as u64 + self.data_len + self.tail.len() as u64
    }
}

/// Costruisce header e coda per uno stream di `data_len` byte e `pad` byte di spazio.
fn layout(data_len: u64, pad: usize) -> Layout {
    let mut head = Vec::new();
    let mut offsets = [0u64; 5];
    head.extend_from_slice(b"%PDF-1.7\n");
    let objs: [&str; 3] = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> >>",
    ];
    for (i, body) in objs.iter().enumerate() {
        offsets[i + 1] = head.len() as u64;
        head.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    head.extend(std::iter::repeat_n(b'\n', pad));
    offsets[4] = head.len() as u64;
    head.extend_from_slice(format!("4 0 obj\n<< /Length {data_len} >>\nstream\n").as_bytes());

    let mut tail = Vec::new();
    tail.extend_from_slice(b"\nendstream\nendobj\n");
    let xref_at = head.len() as u64 + data_len + tail.len() as u64;
    tail.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for off in &offsets[1..] {
        tail.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    tail.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes(),
    );
    Layout {
        head,
        tail,
        data_len,
    }
}

/// Risolve `(data_len, pad)` per una dimensione totale esatta.
fn solve(size: u64) -> std::io::Result<Layout> {
    for pad in 0..32usize {
        let fixed = layout(0, pad).total();
        if size < fixed {
            break;
        }
        let mut data_len = size - fixed;
        for _ in 0..8 {
            let l = layout(data_len, pad);
            let total = l.total();
            if total == size {
                return Ok(l);
            }
            if total > size {
                let over = total - size;
                if over > data_len {
                    break;
                }
                data_len -= over;
            } else {
                data_len += size - total;
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("dimensione {size} troppo piccola per un PDF valido"),
    ))
}

/// Scrive in `path` un PDF di esattamente `size` byte, deterministico per `(size, seed)`.
pub fn generate(path: &Path, size: u64, seed: u64) -> std::io::Result<()> {
    let l = solve(size)?;
    let mut w = BufWriter::with_capacity(256 * 1024, File::create(path)?);
    w.write_all(&l.head)?;
    let mut left = l.data_len;
    let mut counter: u64 = 0;
    let mut hexbuf = [0u8; 64];
    while left > 0 {
        let mut h = Sha256::new();
        h.update(b"s01-pdfgen");
        h.update(seed.to_le_bytes());
        h.update(counter.to_le_bytes());
        let digest = h.finalize();
        hex::encode_to_slice(digest, &mut hexbuf).expect("64 byte esatti");
        let n = left.min(hexbuf.len() as u64) as usize;
        w.write_all(&hexbuf[..n])?;
        left -= n as u64;
        counter += 1;
    }
    w.write_all(&l.tail)?;
    w.flush()?;
    w.get_ref().sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risolve_tutte_le_dimensioni_attorno_ai_salti_di_cifre() {
        for size in (900..1100u64).chain(9_990..10_050).chain(99_990..100_050) {
            let l = solve(size).expect("solve");
            assert_eq!(l.total(), size);
        }
    }
}

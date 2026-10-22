//! s02-parser: lanciatore di processi isolati, contratto del job JSON e prescan (piano 01-07, WIRING W17).
//!
//! - [`job`]: job JSON tipizzato, allowlist operazione → bin, validazione dei percorsi;
//! - [`isolate`]: `run_child` con limiti per figlio, budget aggregato e cleanup del gruppo;
//! - [`protocol`]: esiti tipizzati del figlio, una riga JSON con `emit`;
//! - [`prescan`]: tetti anti-bomb di PDF e DOCX, nel figlio prima di PDFium.
//!
//! Il contratto è chiuso dopo 01-07: i piani 01-08, 01-09, 01-13 aggiungono solo il proprio
//! bin figlio, 01-18 usa i driver di `parse-parent`.
pub mod isolate;
pub mod job;
pub mod prescan;
pub mod protocol;

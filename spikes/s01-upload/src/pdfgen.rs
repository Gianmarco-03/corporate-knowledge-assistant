//! Generatore di PDF strutturalmente validi di dimensione totale esatta (RISK-005),
//! deterministico per `(size, seed)`, scritto in streaming.

use std::path::Path;

/// Scrive in `path` un PDF di esattamente `size` byte, deterministico per `(size, seed)`.
pub fn generate(_path: &Path, _size: u64, _seed: u64) -> std::io::Result<()> {
    todo!()
}

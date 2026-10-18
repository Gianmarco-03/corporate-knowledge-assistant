#!/usr/bin/env bash
# scripts/py.sh — l'interprete Python 3 di questa postazione, risolto in un posto solo.
#
# Gli script dei gate (skills/*/*.py) si lanciano SEMPRE da qui, mai con `python` nudo: su
# Windows `python3` puo' essere il segnaposto dello Store che non esegue nulla, e un interprete
# assente non deve mai produrre un gate silenziosamente verde.
#
# Ordine di risoluzione (si verifica la VERSIONE, non il nome):
#   1. CKA_PYTHON, se impostata (percorso completo dell'interprete);
#   2. python3, poi python, dal PATH;
#   3. il percorso completo di questa postazione (Python 3.13 per utente).
# CKA_PYTHON sceglie QUALE interprete usare, non se eseguire i controlli: non e' un bypass.
#
# Uso:  scripts/py.sh skills/crate-boundary-check/boundary_check.py .
# PYTHONIOENCODING=utf-8 evita UnicodeEncodeError sull'output con accenti e frecce.
set -euo pipefail

e_python3() {
    "$1" -c 'import sys; raise SystemExit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1
}

PY=""
if [ -n "${CKA_PYTHON:-}" ]; then
    e_python3 "$CKA_PYTHON" || { echo "py.sh: CKA_PYTHON=$CKA_PYTHON non e' un interprete Python 3" >&2; exit 127; }
    PY="$CKA_PYTHON"
else
    for candidato in python3 python "C:/Users/Gianmarco/AppData/Local/Programs/Python/Python313/python.exe"; do
        if command -v "$candidato" >/dev/null 2>&1 && e_python3 "$candidato"; then
            PY="$candidato"
            break
        fi
    done
fi

[ -n "$PY" ] || { echo "py.sh: nessun interprete Python 3 trovato (imposta CKA_PYTHON)" >&2; exit 127; }
PYTHONIOENCODING=utf-8 exec "$PY" "$@"

# **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)

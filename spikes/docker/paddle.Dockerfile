# spikes/docker/paddle.Dockerfile — sfidante OCR PP-OCRv6 di S-02 (piano 01-13, D-09 emendata).
# Contesto di build: spikes/ (servizio `paddle` del compose di 01-02: network_mode none,
# utente 10001, 2 GiB, 8 CPU). Si costruisce SOLO dopo l'approvazione utente dei pacchetti
# del lock (Task 1, checkpoint bloccante): prima della risposta nessuna build.
#
# - base per digest, lo stesso scritto in testa a docker/paddle-requirements.lock;
# - un solo passo apt con versioni esatte per le librerie di sistema delle ruote (libgomp1,
#   libgl1, libglib2.0-0t64; decisione utente 2026-10-25), elenco installato in
#   /opt/paddle-tools/apt-installed.txt;
# - una sola installazione con pip, dal solo lock e con --require-hashes: nessun pacchetto fuori lock;
# - cache dei modelli di PaddleX in /opt/paddle-models (variabile PADDLE_PDX_CACHE_HOME,
#   [NV] nome verificato in build: lo step di selezione fallisce se i modelli non finiscono
#   sotto /opt/paddle-models);
# - verifica in build di [NV-v6-flag] e [NV-v6-lang]: valori letterali letti dalla CLI
#   installata e scritti in /opt/paddle-models/v6.selection.json con la riga d'aiuto da cui
#   vengono; v6 non selezionabile in modo esplicito o italiano non coperto da un --lang
#   esplicito → selectable=false con il motivo (l'esecutore si ferma, nessun ripiego);
# - precaricamento con la pipeline istanziata dai valori di v6.selection.json, device="cpu",
#   moduli di orientamento e unwarping spenti; configurazione risolta in v6.resolved.json con
#   family_check sui model_name di det e rec (prefisso PP-OCRv6_), altrimenti selectable=false;
# - manifest SHA-256 di ogni file sotto /opt/paddle-models (manifest.sha256), poi USER 10001.
# Nessun segreto, CPU soltanto, nessuna rete a runtime (la rete serve solo alla build).
#
# **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · immagine dello sfidante PP-OCRv6 con pesi precaricati e selezione verificata in build
# **Modifica:** Claude (gsd-executor 01-13 paddle) · 2026-10-25 · exit dell'aiuto registrato: un import fallito non si legge più come «--ocr_version assente»
# **Modifica:** Claude (gsd-executor 01-13 paddle) · 2026-10-25 · passo apt con versioni fissate per libgomp1, libgl1, libglib2.0-0t64 (decisione utente, opzione A); resolver condiviso build/sessione, modelli da _merged_paddlex_config, italiano da _PPOCRV6_LANGS

FROM python:3.12-slim@sha256:a0f57064cbdcd6285fa45f51237ab0b35ea8ebaed06a1f5973e89e19dfb86704

ENV PADDLE_PDX_CACHE_HOME=/opt/paddle-models \
    PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK=True \
    PYTHONDONTWRITEBYTECODE=1 \
    HOME=/opt/paddle-home

# Librerie di sistema richieste dalle ruote pip (decisione utente 2026-10-25, opzione A
# «versioni fissate»): libgomp1 per paddlepaddle (libgomp.so.1), libgl1 e libglib2.0-0t64
# per opencv-contrib-python di paddlex (libGL.so.1, libglib-2.0/libgthread-2.0). Versioni
# esatte = Candidate di trixie nell'archivio Debian della base fissata per digest
# (apt-cache policy del 2026-10-25); le dipendenze tirate da apt sono elencate in
# RISULTATI-ocr.md §Setup. Nessun altro pacchetto esplicito.
RUN dpkg-query -W -f='${Package}=${Version}\n' | sort > /tmp/dpkg-before \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      libgomp1=14.2.0-19 \
      libgl1=1.7.0-1+b2 \
      libglib2.0-0t64=2.84.4-3~deb13u5 \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /opt/paddle-tools \
 && dpkg-query -W -f='${Package}=${Version}\n' | sort | comm -13 /tmp/dpkg-before - \
      > /opt/paddle-tools/apt-installed.txt \
 && rm -f /tmp/dpkg-before

COPY docker/paddle-requirements.lock /tmp/requirements.lock
RUN pip install --no-cache-dir --require-hashes -r /tmp/requirements.lock \
 && rm -f /tmp/requirements.lock

# L'exit dell'aiuto si registra: un aiuto che non parte (import fallito) non è «flag assente».
RUN mkdir -p /opt/paddle-models /opt/paddle-home \
 && { paddleocr ocr --help > /opt/paddle-models/cli-help.txt 2>&1; \
      echo $? > /opt/paddle-models/cli-help.exit; }

# Configurazione risolta della pipeline v6: un solo script, usato in build (v6.resolved.json)
# e a ogni sessione di misura (resolved-session.json), così i due lati si confrontano sullo
# stesso codice. I model_name vengono dalla configurazione effettiva della pipeline creata
# (`_merged_paddlex_config`, SubModules TextDetection/TextRecognition); la cartella è quella
# della cache di PaddleX (`official_models/<model_name>`) e deve esistere.
RUN cat > /opt/paddle-tools/v6_resolve.py <<'PY'
"""Configurazione risolta della pipeline PP-OCRv6 (piano 01-13, D-09 emendata).

Uso: python3 v6_resolve.py <v6.selection.json> <uscita.json>
"""
import json
import os
import sys

ROOT = "/opt/paddle-models"
MODELS = os.path.join(ROOT, "official_models")


def kwargs_from_selection(sel):
    kwargs = dict(ocr_version=sel["ocr_version"], lang=sel["lang"], device="cpu",
                  use_doc_orientation_classify=False, use_doc_unwarping=False,
                  use_textline_orientation=False)
    tier = sel.get("tier")
    if tier:
        kwargs[tier["flag"].lstrip("-")] = tier["value"]
    return kwargs


def resolve(kwargs):
    from paddleocr import PaddleOCR
    pipeline = PaddleOCR(**kwargs)
    sub = pipeline._merged_paddlex_config["SubModules"]
    out = {}
    for part, key in (("det", "TextDetection"), ("rec", "TextRecognition")):
        name = sub[key]["model_name"]
        model_dir = sub[key].get("model_dir") or os.path.join(MODELS, name)
        files = None
        if os.path.isdir(model_dir):
            files = sorted(os.path.relpath(os.path.join(p, f), model_dir)
                           for p, _, fs in os.walk(model_dir) for f in fs)
        out[part] = {"model_name": name, "model_dir": model_dir, "files": files,
                     "config_model_dir": sub[key].get("model_dir")}
    out["family_check"] = [
        {"part": part, "expected_prefix": "PP-OCRv6_", "found": out[part]["model_name"] or "",
         "ok": (out[part]["model_name"] or "").startswith("PP-OCRv6_")
               and out[part]["files"] is not None}
        for part in ("det", "rec")]
    out["pipeline_kwargs"] = kwargs
    out["source"] = "PaddleOCR(**pipeline_kwargs)._merged_paddlex_config['SubModules']"
    return out


def write(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2, ensure_ascii=False)
        f.write("\n")


if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as f:
        sel = json.load(f)
    if sel.get("selectable") is not True:
        sys.exit("v6 non selezionabile: %s" % sel.get("reason"))
    res = resolve(kwargs_from_selection(sel))
    write(sys.argv[2], res)
    sys.exit(0 if all(c["ok"] for c in res["family_check"]) else 1)
PY

# Selezione di v6 dalla CLI installata ([NV-v6-flag], [NV-v6-lang]) e precaricamento.
RUN python3 - <<'PY'
import json, os, re, sys
sys.path.insert(0, "/opt/paddle-tools")
from v6_resolve import resolve, write as write_json

ROOT = "/opt/paddle-models"
help_text = open(os.path.join(ROOT, "cli-help.txt"), encoding="utf-8", errors="replace").read()
lines = help_text.splitlines()
help_exit = open(os.path.join(ROOT, "cli-help.exit"), encoding="utf-8").read().strip()

def write(name, obj):
    write_json(os.path.join(ROOT, name), obj)

def stop(reason, **extra):
    sel = {"selectable": False, "reason": reason}
    sel.update(extra)
    write("v6.selection.json", sel)
    print("v6.selection.json: selectable=false:", reason)
    sys.exit(0)

def help_block(flag):
    """Righe d'aiuto dell'opzione `flag` (la riga del flag e le successive indentate)."""
    out = []
    for i, l in enumerate(lines):
        if re.search(r"(^|\s)" + re.escape(flag) + r"\b", l):
            out.append(l.strip())
            for nxt in lines[i + 1:i + 6]:
                if nxt.strip().startswith("-"):
                    break
                out.append(nxt.strip())
            break
    return " ".join(out)

if help_exit != "0":
    stop("`paddleocr ocr --help` uscito con codice %s: %s"
         % (help_exit, lines[-1].strip() if lines else "nessun output"))
ver_help = help_block("--ocr_version")
if not ver_help:
    stop("--ocr_version assente da `paddleocr ocr --help`")
if "PP-OCRv6" not in ver_help:
    stop("--ocr_version non elenca PP-OCRv6 nell'aiuto", help_line=ver_help)
lang_help = help_block("--lang")
if not lang_help:
    stop("--lang assente da `paddleocr ocr --help`")

# Variante/tier: un flag dedicato se la CLI lo espone (nome con tier, variant o size).
# Si considera solo un flag il cui aiuto elenca il valore `medium` (PP-OCRv6_medium).
tier = None
for l in lines:
    m = re.search(r"(--[a-z_]*(tier|variant|size)[a-z_]*)", l)
    if m:
        block = help_block(m.group(1))
        if re.search(r"\bmedium\b", block):
            tier = {"flag": m.group(1), "value": "medium", "help_line": block}
            break

ocr_version = "PP-OCRv6"
lang = "it"

# [NV-v6-lang]: l'aiuto di --lang non elenca le lingue; la copertura si legge dal pacchetto
# installato (insieme delle lingue di PP-OCRv6 e funzione che sceglie i modelli).
try:
    from paddleocr._pipelines import ocr as _ocr_mod
    v6_langs = sorted(_ocr_mod._PPOCRV6_LANGS)
except Exception as e:
    stop("lingue di PP-OCRv6 non leggibili dal pacchetto: %s" % e)
if lang not in v6_langs:
    stop("lang %r non coperto da PP-OCRv6 nel pacchetto installato" % lang, v6_langs=v6_langs)

kwargs = dict(ocr_version=ocr_version, lang=lang, device="cpu",
              use_doc_orientation_classify=False, use_doc_unwarping=False,
              use_textline_orientation=False)
if tier is not None:
    kwargs[tier["flag"].lstrip("-")] = tier["value"]
try:
    resolved = resolve(kwargs)
except Exception as e:  # selezione rifiutata dal runtime: nessun ripiego
    stop("pipeline v6 non istanziabile con valori espliciti: %s" % e,
         ocr_version=ocr_version, lang=lang, tier=tier, help_line=ver_help)
write("v6.resolved.json", resolved)
checks = resolved["family_check"]
if not all(c["ok"] for c in checks):
    stop("family_check fallito: model_name di det o rec fuori dalla famiglia PP-OCRv6_ "
         "o cartella del modello assente",
         ocr_version=ocr_version, lang=lang, tier=tier, family_check=checks)

write("v6.selection.json", {
    "selectable": True,
    "ocr_version": ocr_version,
    "tier": None if tier is None else {"flag": tier["flag"], "value": tier["value"]},
    "lang": lang,
    "source": "paddleocr ocr --help",
    "help_line_ocr_version": ver_help,
    "help_line_lang": lang_help,
    "help_line_tier": None if tier is None else tier["help_line"],
    "lang_source": "paddleocr._pipelines.ocr._PPOCRV6_LANGS (pacchetto installato)",
    "v6_langs": v6_langs,
})
print("v6.selection.json: selectable=true",
      resolved["det"]["model_name"], resolved["rec"]["model_name"])
PY

RUN find /opt/paddle-models -type f ! -name manifest.sha256 -exec sha256sum {} + \
      | sort -k2 > /opt/paddle-models/manifest.sha256 \
 && chmod -R a+rX /opt/paddle-models /opt/paddle-home \
 && chown -R 10001:10001 /opt/paddle-home

USER 10001:10001
WORKDIR /opt/paddle-home
CMD ["sleep","infinity"]

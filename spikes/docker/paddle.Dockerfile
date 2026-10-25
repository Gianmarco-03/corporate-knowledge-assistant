# spikes/docker/paddle.Dockerfile — sfidante OCR PP-OCRv6 di S-02 (piano 01-13, D-09 emendata).
# Contesto di build: spikes/ (servizio `paddle` del compose di 01-02: network_mode none,
# utente 10001, 2 GiB, 8 CPU). Si costruisce SOLO dopo l'approvazione utente dei pacchetti
# del lock (Task 1, checkpoint bloccante): prima della risposta nessuna build.
#
# - base per digest, lo stesso scritto in testa a docker/paddle-requirements.lock;
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

FROM python:3.12-slim@sha256:a0f57064cbdcd6285fa45f51237ab0b35ea8ebaed06a1f5973e89e19dfb86704

ENV PADDLE_PDX_CACHE_HOME=/opt/paddle-models \
    PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK=True \
    PYTHONDONTWRITEBYTECODE=1 \
    HOME=/opt/paddle-home

COPY docker/paddle-requirements.lock /tmp/requirements.lock
RUN pip install --no-cache-dir --require-hashes -r /tmp/requirements.lock \
 && rm -f /tmp/requirements.lock

# L'exit dell'aiuto si registra: un aiuto che non parte (import fallito) non è «flag assente».
RUN mkdir -p /opt/paddle-models /opt/paddle-home \
 && { paddleocr ocr --help > /opt/paddle-models/cli-help.txt 2>&1; \
      echo $? > /opt/paddle-models/cli-help.exit; }

# Selezione di v6 dalla CLI installata ([NV-v6-flag], [NV-v6-lang]) e precaricamento.
RUN python3 - <<'PY'
import json, os, re, sys

ROOT = "/opt/paddle-models"
help_text = open(os.path.join(ROOT, "cli-help.txt"), encoding="utf-8", errors="replace").read()
lines = help_text.splitlines()
help_exit = open(os.path.join(ROOT, "cli-help.exit"), encoding="utf-8").read().strip()

def write(name, obj):
    with open(os.path.join(ROOT, name), "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2, ensure_ascii=False)
        f.write("\n")

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

kwargs = dict(ocr_version=ocr_version, lang=lang, device="cpu",
              use_doc_orientation_classify=False, use_doc_unwarping=False,
              use_textline_orientation=False)
if tier is not None:
    kwargs[tier["flag"].lstrip("-")] = tier["value"]
try:
    from paddleocr import PaddleOCR
    pipeline = PaddleOCR(**kwargs)
except Exception as e:  # selezione rifiutata dal runtime: nessun ripiego
    stop("pipeline v6 non istanziabile con valori espliciti: %s" % e,
         ocr_version=ocr_version, lang=lang, tier=tier, help_line=ver_help)

# Modelli risolti: le cartelle scaricate nella cache sono quelle che la pipeline carica.
models_root = None
for dirpath, dirnames, _ in os.walk(ROOT):
    if os.path.basename(dirpath) == "official_models":
        models_root = dirpath
        break
if models_root is None:
    stop("nessuna cartella official_models sotto /opt/paddle-models: variabile di cache [NV] non rispettata")

def model(kind):
    names = sorted(d for d in os.listdir(models_root) if d.endswith("_" + kind))
    if len(names) != 1:
        return {"model_name": None, "candidates": names}
    d = os.path.join(models_root, names[0])
    files = sorted(os.path.relpath(os.path.join(p, f), d)
                   for p, _, fs in os.walk(d) for f in fs)
    return {"model_name": names[0], "model_dir": d, "files": files}

det, rec = model("det"), model("rec")
checks = []
for part, m in (("det", det), ("rec", rec)):
    name = m.get("model_name") or ""
    checks.append({"part": part, "expected_prefix": "PP-OCRv6_", "found": name,
                   "ok": name.startswith("PP-OCRv6_")})
resolved = {"det": det, "rec": rec, "family_check": checks,
            "pipeline_kwargs": {k: v for k, v in kwargs.items()}}
write("v6.resolved.json", resolved)
if not all(c["ok"] for c in checks):
    stop("family_check fallito: model_name di det o rec fuori dalla famiglia PP-OCRv6_",
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
})
print("v6.selection.json: selectable=true", det["model_name"], rec["model_name"])
PY

RUN find /opt/paddle-models -type f ! -name manifest.sha256 -exec sha256sum {} + \
      | sort -k2 > /opt/paddle-models/manifest.sha256 \
 && chmod -R a+rX /opt/paddle-models /opt/paddle-home \
 && chown -R 10001:10001 /opt/paddle-home

USER 10001:10001
WORKDIR /opt/paddle-home
CMD ["sleep","infinity"]

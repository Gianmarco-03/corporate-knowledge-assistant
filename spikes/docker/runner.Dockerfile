# spikes/docker/runner.Dockerfile — ambiente Linux per i bin degli spike (WIRING W12,
# proprietario 01-02, chiuso). Serve a dhat, VmHWM, memory.peak, /proc e ss, che
# richiedono Linux, e porta oha, qpdf, pdftotext e imagemagick.
#
# Nessun segreto nell'immagine (le chiavi arrivano a runtime dall'env del servizio) e
# nessun client Docker: l'orchestrazione resta sull'host (spikes/tools/session.sh).

FROM rust:1.99.1-slim-trixie

# oha v1.16.0, asset ufficiale linux amd64 (github.com/hatoo/oha, release 2026-08-23),
# digest dalla release e ricalcolato sul binario scaricato il 2026-10-19
ARG OHA_VERSION=v1.16.0
ARG OHA_SHA256=620bb9e16fb53eabc9a3fc45f88bdb41fefa3fee5c05e75892011ce320391716

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      procps iproute2 curl findutils ca-certificates pkg-config \
      qpdf poppler-utils imagemagick python3 git \
 && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL -o /usr/local/bin/oha \
      "https://github.com/hatoo/oha/releases/download/${OHA_VERSION}/oha-linux-amd64" \
 && echo "${OHA_SHA256}  /usr/local/bin/oha" | sha256sum -c - \
 && chmod 0755 /usr/local/bin/oha

# Un volume nominato vuoto eredita proprietario e permessi dalla directory dell'immagine:
# /target, /work/tmp e il registry Cargo nascono di 10001. CARGO_HOME (/usr/local/cargo)
# resta scrivibile per git/ e .package-cache.
RUN groupadd --gid 10001 runner \
 && useradd --uid 10001 --gid 10001 --create-home --shell /bin/bash runner \
 && mkdir -p /target /work/tmp /usr/local/cargo/registry \
 && chown -R 10001:10001 /target /work /usr/local/cargo

ENV CARGO_TARGET_DIR=/target \
    TMPDIR=/work/tmp

USER 10001
WORKDIR /src

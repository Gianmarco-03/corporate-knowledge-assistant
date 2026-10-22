# spikes/docker/worker.Dockerfile — immagine del worker di s02-parser (piano 01-07, WIRING W12,
# proprietario 01-07). Contesto di build: spikes/ (.dockerignore esclude .env, data/, target/,
# results/). Compila tutto il crate s02-parser e copia tutti i suoi bin in /usr/local/bin: i bin
# figli di 01-08, 01-09, 01-13 entrano ricostruendo l'immagine, senza editare questo file.
#
# Runtime: debian:trixie-slim con Tesseract 5.5.0 da apt (versione di trixie, accettata con
# decisione vincolante: D-09 indica la CLI Tesseract e la patch è irrilevante), PDFium non-V8
# chromium/8100 di bblanchon/pdfium-binaries e tessdata_best ita, eng, osd dal ramo main, tutti
# scaricati via HTTPS e verificati con sha256sum -c sugli hash fissati qui sotto (calcolati al
# primo download, 2026-10-21). Nessun segreto. Il servizio worker del compose (01-02) aggiunge
# network_mode none, read_only, tmpfs /work, mem_limit 2g, cap_drop ALL, utente 10001.

FROM rust:1.99.1-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p s02-parser \
 && mkdir -p /out/bin \
 && find /src/target/release -maxdepth 1 -type f -perm -u+x -exec cp {} /out/bin/ \;

FROM debian:trixie-slim AS fetch
# PDFium chromium/8100 (MAJOR 157, BUILD 8100), build non-V8 e non-XFA (args.gn della release).
ARG PDFIUM_RELEASE=chromium%2F8100
ARG PDFIUM_SHA256=1c1894a12d4d96ee01400c3874444126254180c5c7d2ab628576b67f8120320a
ARG TESSDATA_ITA_SHA256=8df9c89176fb93f56bf4b2d4ede04c01c1f31d4b7697fbd76cc336df700f3f38
ARG TESSDATA_ENG_SHA256=8280aed0782fe27257a68ea10fe7ef324ca0f8d85bd2fd145d1c2b560bcb66ba
ARG TESSDATA_OSD_SHA256=9cf5d576fcc47564f11265841e5ca839001e7e6f38ff7f7aacf46d15a96b00ff
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /fetch
RUN curl -fsSL --retry 5 -o pdfium-linux-x64.tgz \
      "https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_RELEASE}/pdfium-linux-x64.tgz" \
 && echo "${PDFIUM_SHA256}  pdfium-linux-x64.tgz" | sha256sum -c - \
 && mkdir -p /opt/pdfium \
 && tar -xzf pdfium-linux-x64.tgz -C /opt/pdfium
RUN mkdir -p /opt/tessdata_best \
 && for lang in ita eng osd; do \
      curl -fsSL --retry 5 -o "/opt/tessdata_best/${lang}.traineddata" \
        "https://raw.githubusercontent.com/tesseract-ocr/tessdata_best/main/${lang}.traineddata" || exit 1; \
    done \
 && printf '%s  %s\n' \
      "${TESSDATA_ITA_SHA256}" /opt/tessdata_best/ita.traineddata \
      "${TESSDATA_ENG_SHA256}" /opt/tessdata_best/eng.traineddata \
      "${TESSDATA_OSD_SHA256}" /opt/tessdata_best/osd.traineddata \
    | sha256sum -c -

FROM debian:trixie-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends tesseract-ocr \
 && rm -rf /var/lib/apt/lists/*
COPY --from=fetch /opt/pdfium/lib /opt/pdfium/lib
COPY --from=fetch /opt/tessdata_best /opt/tessdata_best
COPY --from=build /out/bin/ /usr/local/bin/
ENV PDFIUM_DIR=/opt/pdfium/lib \
    TESSDATA_PREFIX=/opt/tessdata_best
USER 10001:10001
WORKDIR /work
CMD ["sleep","infinity"]

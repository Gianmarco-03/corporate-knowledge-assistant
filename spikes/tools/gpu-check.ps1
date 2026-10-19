# spikes/tools/gpu-check.ps1 — gate GPU di wave 0 (D-13, RESEARCH §6; WIRING W10).
# Proprietario: piano 01-02, chiuso.
#
# Si lancia dentro una sessione prenotata, che imposta CKA_RUN_ID:
#   bash spikes/tools/session.sh with 01-02 -- powershell -NoProfile -ExecutionPolicy Bypass -File spikes/tools/gpu-check.ps1
# Sequenza: nvidia-smi, wsl --version, docker version, nbody su GPU, TEI GPU con
# Qwen3-Embedding-0.6B e POST /embed (1024 valori), VRAM letta con TEI caricato.
# Esito in spikes/results/gpu-check.txt; ultima riga `VERDETTO: VERDE` solo se nbody
# stampa il nome della GPU e TEI restituisce il vettore, altrimenti
# `VERDETTO: ROSSO — <passo fallito>`. Ogni container porta l'etichetta cka.run_id e
# il container TEI si arresta anche su errore.

$ErrorActionPreference = 'Continue'
# output dei comandi nativi (docker logs, nvidia-smi) decodificato come UTF-8: con la
# code page OEM di default i caratteri non ASCII dei log TEI arrivano corrotti
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$runId = $env:CKA_RUN_ID
if (-not $runId -or $runId -notmatch '^[a-z0-9][a-z0-9-]{0,62}$' -or $runId -eq 'none') {
    Write-Error 'gpu-check.ps1: CKA_RUN_ID assente o non valido; lanciare dentro session.sh with'
    exit 2
}

$root = (git rev-parse --show-toplevel).Trim()
$outFile = Join-Path $root 'spikes/results/gpu-check.txt'
$lines = New-Object System.Collections.Generic.List[string]
$failed = $null

# Righe ripulite: niente sequenze ANSI dei log TEI, righe lunghe troncate (gli URL
# firmati del CDN Hugging Face non servono all'esito e non vanno versionati).
function Log([string]$text) {
    foreach ($l in ($text -split "`r?`n")) {
        $l = $l -replace '\x1b\[[0-9;]*m', ''
        if ($l.Length -gt 240) { $l = $l.Substring(0, 240) + ' ...[troncato]' }
        $lines.Add($l); Write-Host $l
    }
}
# Testo di un output nativo: le righe di stderr (ErrorRecord) senza l'involucro di PowerShell.
function Text($out) { (@($out) | ForEach-Object { $_.ToString() }) -join "`n" }
function Section([string]$title) { Log ''; Log "== $title" }
function Fail([string]$step) { if (-not $script:failed) { $script:failed = $step } }

$teiImage = 'ghcr.io/huggingface/text-embeddings-inference:86-1.9.4'
$nbodyImage = 'nvcr.io/nvidia/k8s/cuda-sample:nbody'
$teiName = 'cka-gpu-check'
$teiPort = 18081
$hfVolume = 'cka-spikes_hf-cache'

Log '# Gate GPU di wave 0 (D-13), spikes/tools/gpu-check.ps1'
Log ("timestamp_utc: " + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
Log "run_id: $runId"
$rev = (git rev-parse --short HEAD).Trim()
$dirty = git -C $root status --porcelain -- spikes/tools
if ($dirty) { $rev = "$rev (con modifiche locali in spikes/tools)" }
Log "git_rev: $rev"

# 1. nvidia-smi (ripiego senza compute_cap se il campo non esiste, RESEARCH §11.11)
Section '1. nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap'
$gpuName = $null
$smi = & nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap --format=csv 2>&1
if ($LASTEXITCODE -ne 0) {
    Log (Text $smi)
    Log 'compute_cap non disponibile: ripiego senza il campo'
    $smi = & nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv 2>&1
}
Log (Text $smi)
if ($LASTEXITCODE -ne 0) { Fail 'nvidia-smi' } else {
    $gpuName = (& nvidia-smi --query-gpu=name --format=csv,noheader | Select-Object -First 1).Trim()
}
$vram0 = & nvidia-smi --query-gpu=memory.used,memory.total --format=csv 2>&1
Log 'VRAM prima dei test:'
Log (Text $vram0)

# 2. WSL
Section '2. wsl --version'
$env:WSL_UTF8 = '1'
$wsl = & wsl --version 2>&1
Log (Text $wsl)
if ($LASTEXITCODE -ne 0) { Fail 'wsl --version' }

# 3. Docker
Section "3. docker version --format '{{.Server.Version}}'"
$dv = & docker version --format '{{.Server.Version}}' 2>&1
Log (Text $dv)
if ($LASTEXITCODE -ne 0) { Fail 'docker version' }

# 4. nbody su GPU
Section "4. docker run --rm --gpus=all $nbodyImage nbody -gpu -benchmark"
if (-not $failed) {
    $nb = & docker run --rm --gpus=all --label "cka.run_id=$runId" $nbodyImage nbody -gpu -benchmark 2>&1
    $nbText = (Text $nb)
    Log $nbText
    $nbOk = ($LASTEXITCODE -eq 0) -and $gpuName -and ($nbText -match [regex]::Escape($gpuName.Replace('NVIDIA ', ''))) -and ($nbText -match 'interactions per second')
    if (-not $nbOk) { Fail 'nbody (nome GPU o interactions per second assenti)' }
} else { Log "saltato: passo precedente fallito ($failed)" }

# 5. TEI GPU + /embed
Section "5. TEI $teiImage --model-id Qwen/Qwen3-Embedding-0.6B --dtype float16"
if (-not $failed) {
    try {
        # volume della cache HF condiviso col compose: se manca lo crea con le etichette
        # del progetto, così compose lo riconosce come suo
        & docker volume inspect $hfVolume *> $null
        if ($LASTEXITCODE -ne 0) {
            & docker volume create --label com.docker.compose.project=cka-spikes `
                --label com.docker.compose.volume=hf-cache $hfVolume | Out-Null
        }
        $health = (New-Object System.UriBuilder('http', '127.0.0.1', $teiPort, '/health')).Uri
        $embed = (New-Object System.UriBuilder('http', '127.0.0.1', $teiPort, '/embed')).Uri
        # Senza --rm: se TEI esce durante il download dei pesi i log restano leggibili.
        # Fino a 3 tentativi: un errore DNS transitorio di Docker Desktop durante il
        # primo download non e' un guasto della GPU; i file gia' scaricati restano nella
        # cache hf-cache e il tentativo successivo riparte da li'.
        $deadline = (Get-Date).AddMinutes(15)
        $ready = $false
        $t0 = Get-Date
        for ($attempt = 1; $attempt -le 3 -and -not $ready -and (Get-Date) -lt $deadline; $attempt++) {
            & docker rm -f $teiName *> $null
            $cid = & docker run -d --gpus=all --name $teiName --label "cka.run_id=$runId" `
                -p "127.0.0.1:${teiPort}:80" -v "${hfVolume}:/data" $teiImage `
                --model-id Qwen/Qwen3-Embedding-0.6B --dtype float16 2>&1
            Log "tentativo ${attempt}, container: $((Text $cid).Trim())"
            if ($LASTEXITCODE -ne 0) { throw 'avvio TEI fallito' }
            while ((Get-Date) -lt $deadline) {
                try {
                    $r = Invoke-WebRequest -Uri $health -UseBasicParsing -TimeoutSec 5
                    if ($r.StatusCode -eq 200) { $ready = $true; break }
                } catch { }
                $running = & docker ps -q --filter "name=^${teiName}$"
                if (-not $running) {
                    $logs = & docker logs --tail 12 $teiName 2>&1
                    Log "TEI uscito al tentativo ${attempt}; coda del log:"
                    Log (Text $logs)
                    break
                }
                Start-Sleep -Seconds 5
            }
        }
        Log ("health dopo {0:N0} s: {1}" -f ((Get-Date) - $t0).TotalSeconds, $ready)
        if (-not $ready) { throw 'TEI /health non pronto (3 tentativi o 15 minuti)' }
        $body = '{"inputs":["Ricavi consolidati 2025"]}'
        $resp = Invoke-RestMethod -Uri $embed -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 60
        $dim = @($resp[0]).Count
        Log "POST /embed: $(@($resp).Count) vettore/i, dimensione $dim"
        $vram1 = & nvidia-smi --query-gpu=memory.used,memory.total --format=csv 2>&1
        Log 'VRAM con TEI caricato:'
        Log (Text $vram1)
        $tl = & docker logs --tail 15 $teiName 2>&1
        Log 'log TEI (coda):'
        Log (Text $tl)
        if ($dim -ne 1024) { throw "embedding di dimensione $dim invece di 1024" }
    } catch {
        Log "ERRORE: $($_.Exception.Message)"
        Fail "TEI GPU ($($_.ToString()))"
    } finally {
        & docker stop $teiName *> $null
        & docker rm -f $teiName *> $null
        Log "container $teiName arrestato"
    }
} else { Log "saltato: passo precedente fallito ($failed)" }

Log ''
# trattino lungo come [char]: PowerShell 5.1 legge gli script senza BOM come ANSI
$dash = [char]0x2014
if ($failed) { Log "VERDETTO: ROSSO $dash $failed" } else { Log 'VERDETTO: VERDE' }

New-Item -ItemType Directory -Force -Path (Split-Path $outFile) | Out-Null
[System.IO.File]::WriteAllLines($outFile, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "esito scritto in $outFile"
if ($failed) { exit 1 } else { exit 0 }

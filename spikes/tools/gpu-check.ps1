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

$runId = $env:CKA_RUN_ID
if (-not $runId -or $runId -notmatch '^[a-z0-9][a-z0-9-]{0,62}$' -or $runId -eq 'none') {
    Write-Error 'gpu-check.ps1: CKA_RUN_ID assente o non valido; lanciare dentro session.sh with'
    exit 2
}

$root = (git rev-parse --show-toplevel).Trim()
$outFile = Join-Path $root 'spikes/results/gpu-check.txt'
$lines = New-Object System.Collections.Generic.List[string]
$failed = $null

function Log([string]$text) {
    foreach ($l in ($text -split "`r?`n")) { $lines.Add($l); Write-Host $l }
}
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
Log ("git_rev: " + (git rev-parse --short HEAD).Trim())

# 1. nvidia-smi (ripiego senza compute_cap se il campo non esiste, RESEARCH §11.11)
Section '1. nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap'
$gpuName = $null
$smi = & nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap --format=csv 2>&1
if ($LASTEXITCODE -ne 0) {
    Log ($smi | Out-String).TrimEnd()
    Log 'compute_cap non disponibile: ripiego senza il campo'
    $smi = & nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv 2>&1
}
Log ($smi | Out-String).TrimEnd()
if ($LASTEXITCODE -ne 0) { Fail 'nvidia-smi' } else {
    $gpuName = (& nvidia-smi --query-gpu=name --format=csv,noheader | Select-Object -First 1).Trim()
}
$vram0 = & nvidia-smi --query-gpu=memory.used,memory.total --format=csv 2>&1
Log 'VRAM prima dei test:'
Log ($vram0 | Out-String).TrimEnd()

# 2. WSL
Section '2. wsl --version'
$env:WSL_UTF8 = '1'
$wsl = & wsl --version 2>&1
Log ($wsl | Out-String).TrimEnd()
if ($LASTEXITCODE -ne 0) { Fail 'wsl --version' }

# 3. Docker
Section "3. docker version --format '{{.Server.Version}}'"
$dv = & docker version --format '{{.Server.Version}}' 2>&1
Log ($dv | Out-String).TrimEnd()
if ($LASTEXITCODE -ne 0) { Fail 'docker version' }

# 4. nbody su GPU
Section "4. docker run --rm --gpus=all $nbodyImage nbody -gpu -benchmark"
if (-not $failed) {
    $nb = & docker run --rm --gpus=all --label "cka.run_id=$runId" $nbodyImage nbody -gpu -benchmark 2>&1
    $nbText = ($nb | Out-String).TrimEnd()
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
        $cid = & docker run -d --rm --gpus=all --name $teiName --label "cka.run_id=$runId" `
            -p "127.0.0.1:${teiPort}:80" -v "${hfVolume}:/data" $teiImage `
            --model-id Qwen/Qwen3-Embedding-0.6B --dtype float16 2>&1
        Log "container: $(($cid | Out-String).Trim())"
        if ($LASTEXITCODE -ne 0) { throw 'avvio TEI fallito' }

        $health = (New-Object System.UriBuilder('http', '127.0.0.1', $teiPort, '/health')).Uri
        $embed = (New-Object System.UriBuilder('http', '127.0.0.1', $teiPort, '/embed')).Uri
        $deadline = (Get-Date).AddMinutes(15)
        $ready = $false
        $t0 = Get-Date
        while ((Get-Date) -lt $deadline) {
            try {
                $r = Invoke-WebRequest -Uri $health -UseBasicParsing -TimeoutSec 5
                if ($r.StatusCode -eq 200) { $ready = $true; break }
            } catch { }
            $running = & docker ps -q --filter "name=^${teiName}$"
            if (-not $running) { break }
            Start-Sleep -Seconds 5
        }
        Log ("health dopo {0:N0} s: {1}" -f ((Get-Date) - $t0).TotalSeconds, $ready)
        if (-not $ready) {
            $logs = & docker logs --tail 40 $teiName 2>&1
            Log ($logs | Out-String).TrimEnd()
            throw 'TEI /health non pronto entro 15 minuti'
        }
        $body = '{"inputs":["Ricavi consolidati 2025"]}'
        $resp = Invoke-RestMethod -Uri $embed -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 60
        $dim = @($resp[0]).Count
        Log "POST /embed: $(@($resp).Count) vettore/i, dimensione $dim"
        $vram1 = & nvidia-smi --query-gpu=memory.used,memory.total --format=csv 2>&1
        Log 'VRAM con TEI caricato:'
        Log ($vram1 | Out-String).TrimEnd()
        $apps = & nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv 2>&1
        Log ($apps | Out-String).TrimEnd()
        $tl = & docker logs --tail 15 $teiName 2>&1
        Log 'log TEI (coda):'
        Log ($tl | Out-String).TrimEnd()
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

$audioDir = Join-Path $PSScriptRoot '../assets/audio'
New-Item -ItemType Directory -Force $audioDir | Out-Null
function Write-Wav($name, $duration, $tone, $music) {
    $count = [int](22050 * $duration)
    $stream = [System.IO.File]::Create((Join-Path $audioDir $name))
    $writer = [System.IO.BinaryWriter]::new($stream)
    $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF')); $writer.Write([int](36 + $count * 2))
    $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt ')); $writer.Write([int]16)
    $writer.Write([int16]1); $writer.Write([int16]1); $writer.Write([int]22050); $writer.Write([int]44100); $writer.Write([int16]2); $writer.Write([int16]16)
    $writer.Write([Text.Encoding]::ASCII.GetBytes('data')); $writer.Write([int]($count * 2))
    $rng = [Random]::new([int]$tone)
    for ($i=0; $i -lt $count; $i++) {
        $t = $i / 22050.0
        if ($music) { $sample = ([Math]::Sin(2*[Math]::PI*55*$t)+[Math]::Sin(2*[Math]::PI*82.5*$t)+[Math]::Sin(2*[Math]::PI*110*$t)) * 0.05 * [Math]::Min(1,$t) * [Math]::Min(1,$duration-$t) }
        else { $sample = (($rng.NextDouble()*2-1)*0.75+[Math]::Sin(2*[Math]::PI*$tone*$t)*0.25)*[Math]::Exp(-$t*24) }
        $writer.Write([int16]($sample * 24000))
    }
    $writer.Dispose()
}
for ($i=0; $i -lt 5; $i++) { Write-Wav "shot$i.wav" 0.4 (90+$i*17) $false }
Write-Wav 'menu.wav' 8 55 $true

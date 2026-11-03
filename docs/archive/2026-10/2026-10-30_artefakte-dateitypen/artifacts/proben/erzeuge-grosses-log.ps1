# Schreibt eine ca. 20 MiB grosse Logdatei fuer den Groessen-Deckel des Betrachters.
# Nur ASCII, weil Windows PowerShell 5.1 Skripte ohne BOM als ANSI liest.
param([string]$Ziel = (Join-Path $PSScriptRoot 'gross.log'))
$levels = 'INFO', 'DEBUG', 'WARN', 'ERROR'
$writer = [System.IO.StreamWriter]::new($Ziel)
try {
  $index = 0
  while ($writer.BaseStream.Length -lt 20MB) {
    $level = $levels[$index % 4]
    $writer.WriteLine("2026-10-30 09:00:00 $level Zeile $index - Beispieltext fuer den Groessen-Deckel des Betrachters")
    $index++
  }
} finally {
  $writer.Dispose()
}

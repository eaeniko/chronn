# Soma a memória do work-timer.exe e dos msedgewebview2.exe descendentes:
# o working set (inclui páginas compartilhadas com outros processos) e a memória privada (só deles).
$raiz = Get-CimInstance Win32_Process -Filter "Name='work-timer.exe'"
if (-not $raiz) { Write-Output "Work Timer: nao esta rodando"; exit 1 }
$todos = Get-CimInstance Win32_Process
$ids = New-Object System.Collections.Generic.HashSet[int]
$fila = New-Object System.Collections.Queue
foreach ($r in $raiz) { [void]$ids.Add([int]$r.ProcessId); $fila.Enqueue([int]$r.ProcessId) }
while ($fila.Count -gt 0) {
  $pai = $fila.Dequeue()
  foreach ($p in $todos) {
    if ([int]$p.ParentProcessId -eq $pai -and $p.Name -eq 'msedgewebview2.exe' -and $ids.Add([int]$p.ProcessId)) {
      $fila.Enqueue([int]$p.ProcessId)
    }
  }
}
$soma = 0
$privada = 0
foreach ($id in $ids) {
  $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($proc) { $soma += $proc.WorkingSet64; $privada += $proc.PrivateMemorySize64 }
}
Write-Output ("Work Timer: {0} MB (working set) | {1} MB (privada) | {2} processos" -f [math]::Round($soma / 1MB), [math]::Round($privada / 1MB), $ids.Count)

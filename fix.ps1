$info = cargo sqlx migrate info
$current_version = $null
$updates = @()

foreach ($line in $info) {
    if ($line -match "^(\d+)/installed\s+\(different checksum\)") {
        $current_version = $matches[1]
    } elseif ($current_version -and $line -match "^local migration has checksum ([a-f0-9]+)") {
        $checksum = $matches[1]
        $updates += "UPDATE _sqlx_migrations SET checksum = '\x$checksum' WHERE version = $current_version;"
        $current_version = $null
    }
}

if ($updates.Count -gt 0) {
    $sql = $updates -join "`n"
    Write-Host "Executing:`n$sql"
    psql "postgresql://postgres:password1235@localhost:5433/livana_db" -c $sql
} else {
    Write-Host "No mismatches found"
}

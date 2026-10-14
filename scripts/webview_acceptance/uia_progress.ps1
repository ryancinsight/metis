# Two bounded diagnostic slots; Flush() publishes buffers, not durable storage.
# https://learn.microsoft.com/en-us/dotnet/api/system.io.filestream.flush
$script:UiaProgressStreams = @()
$script:UiaProgressPaths = @()
$script:UiaProgressStep = [long]0
$script:UiaProgressEncoding = [Text.UTF8Encoding]::new($false)

function Close-UiaProgress($primary = $null) {
    $failure = $primary
    if ($failure -is [Management.Automation.ErrorRecord]) { $failure = $failure.Exception }
    for ($index = 0; $index -lt $script:UiaProgressStreams.Count; $index++) {
        try { $script:UiaProgressStreams[$index].Dispose() }
        catch {
            if ($null -eq $failure) { $failure = $_.Exception }
            else { $failure.Data['uia_progress_dispose_' + $index] = $_.Exception }
        }
    }
    $script:UiaProgressStreams = @()
    if ($null -eq $primary -and $null -ne $failure) { throw $failure }
}

function Initialize-UiaProgress($configuration) {
    if ($script:UiaProgressStreams.Count -ne 0) { throw [InvalidOperationException]::new('UIA progress is already open') }
    $paths = @($configuration.paths)
    $maximum = [long]$configuration.maximum_bytes
    if ($paths.Count -ne 2 -or $maximum -le 0 -or $maximum -gt [int]::MaxValue) {
        throw [ArgumentException]::new('UIA progress requires two paths and a positive supported byte bound')
    }
    foreach ($path in $paths) {
        $full = [IO.Path]::GetFullPath($path)
        if (-not [IO.Path]::IsPathRooted($path) -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals([IO.Path]::GetPathRoot($path), [IO.Path]::GetPathRoot($full))) {
            throw [ArgumentException]::new('UIA progress paths must be absolute')
        }
    }
    $script:UiaProgressMaximum = $maximum
    $script:UiaProgressPaths = @()
    $script:UiaProgressStep = [long]0
    try {
        foreach ($path in $paths) {
            $stream = [IO.FileStream]::new($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
            $script:UiaProgressStreams += $stream
            $script:UiaProgressPaths += $path
        }
    } catch {
        $primary = $_
        Close-UiaProgress $primary
        for ($index = 0; $index -lt $script:UiaProgressPaths.Count; $index++) {
            try { [IO.File]::Delete($script:UiaProgressPaths[$index]) }
            catch { $primary.Exception.Data['uia_progress_delete_' + $index] = $_.Exception }
        }
        throw $primary
    }
}

function Set-UiaProgress([string]$phase, [long]$request_sequence, [int]$node_index = -1) {
    if ($script:UiaProgressStreams.Count -ne 2) { throw [InvalidOperationException]::new('UIA progress is not open') }
    if ($request_sequence -lt 0 -or $node_index -lt -1 -or $phase.Length -gt $script:UiaProgressMaximum -or
        $phase -cnotmatch '^[A-Za-z0-9_.:-]+$') {
        throw [ArgumentException]::new('UIA progress requires bounded ASCII phase and valid sequence/index')
    }
    if ($script:UiaProgressStep -eq [long]::MaxValue) { throw [OverflowException]::new('UIA progress step exhausted') }
    $step = $script:UiaProgressStep + [long]1
    $thread = [Threading.Thread]::CurrentThread
    $format = '{{"schema":1,"source":"powershell-uia-actor","phase":"{0}","request_sequence":"{1}","step":"{2}","stopwatch_ticks":"{3}","stopwatch_frequency":"{4}","process_id":{5},"managed_thread_id":{6},"apartment":"{7}","node_index":{8}}}'
    $text = [string]::Format([Globalization.CultureInfo]::InvariantCulture, $format,
        [object[]]@($phase, $request_sequence, $step, [Diagnostics.Stopwatch]::GetTimestamp(),
            [Diagnostics.Stopwatch]::Frequency, $PID, $thread.ManagedThreadId, $thread.GetApartmentState(), $node_index)) + "`n"
    $bytes = $script:UiaProgressEncoding.GetBytes($text)
    if ($bytes.Length -gt $script:UiaProgressMaximum) { throw [IO.InvalidDataException]::new('UIA progress record exceeds its byte bound') }
    $stream = $script:UiaProgressStreams[[int]($script:UiaProgressStep % 2)]
    $stream.Position = 0
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.SetLength($bytes.Length)
    $stream.Flush()
    $script:UiaProgressStep = $step
}

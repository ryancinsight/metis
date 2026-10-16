# Read-only target-owned ranges; endpoint comparisons specify ordering only.
# https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.textpattern.getselection
# https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.text.textpatternrange.compareendpoints
function Read-UiaSelection {
    param($Target, [long]$Sequence)
    $result = [ordered]@{
        available = $false; supported = $null
        range_count = $null; comparisons = $null
    }
    $pattern = $null
    Set-UiaProgress 'selection_pattern' $Sequence
    if (-not $Target.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
        return [pscustomobject]$result
    }
    $result.available = $true
    Set-UiaProgress 'selection_support' $Sequence
    # Public accessor calls preserve provider exceptions; PowerShell property
    # syntax can turn a failed getter into an unobserved null value.
    $result.supported = $pattern.get_SupportedTextSelection().ToString()
    # GetSelection throws when selection is unsupported. Absence is an
    # observation, never a successful caret or whole-document selection.
    if ($result.supported -eq 'None') { return [pscustomobject]$result }
    Set-UiaProgress 'selection_document' $Sequence
    $document = $pattern.get_DocumentRange()
    Set-UiaProgress 'selection_ranges' $Sequence
    $ranges = $pattern.GetSelection()
    $result.range_count = if ($null -eq $ranges) { 0 } else { $ranges.Count }
    if ($result.range_count -eq 1) {
        $selected = $ranges[0]
        $start = [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start
        $end = [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End
        Set-UiaProgress 'selection_compare_start' $Sequence
        $first = $selected.CompareEndpoints($start, $document, $start)
        Set-UiaProgress 'selection_compare_end' $Sequence
        $last = $selected.CompareEndpoints($end, $document, $end)
        Set-UiaProgress 'selection_compare_extent' $Sequence
        $extent = $selected.CompareEndpoints($start, $selected, $end)
        Set-UiaProgress 'selection_compare_document' $Sequence
        $documentExtent = $document.CompareEndpoints($start, $document, $end)
        $result.comparisons = [pscustomobject]@{
            start_to_document_start = $first; end_to_document_end = $last
            start_to_end = $extent; document_start_to_end = $documentExtent
        }
    }
    return [pscustomobject]$result
}

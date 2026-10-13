$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$scope = [System.Windows.Automation.TreeScope]::Descendants
$condition = [System.Windows.Automation.Condition]::TrueCondition

while ($null -ne ($line = [Console]::ReadLine())) {
    try {
        $command = $line | ConvertFrom-Json
        $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$command.handle)
        if ($null -eq $root) { throw 'Actual UI Automation root is unavailable' }
        $nodes = @($root.FindAll($scope, $condition))
        if ($nodes.Count -eq 0 -or $nodes.Count -gt $command.maximum_nodes) { throw 'Actual UIA tree is empty or exceeds its canonical node contract' }
        if ($command.action -eq 'focus') {
            $matched = @($nodes | Where-Object { $_.Current.AutomationId -eq $command.id })
            if ($matched.Count -ne 1) { throw 'Requested UIA focus control is absent or ambiguous' }
            $matched[0].SetFocus()
        } elseif ($command.action -ne 'snapshot') {
            throw 'Unknown UIA acceptance action'
        }
        $records = @($nodes | ForEach-Object {
            $current = $_.Current
            $value = $null
            $pattern = $null
            if ($_.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                $value = $pattern.Current.Value
            } elseif ($_.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$pattern)) {
                $value = $pattern.Current.Value.ToString([Globalization.CultureInfo]::InvariantCulture)
            }
            $text = [string]$current.Name
            if ($current.AutomationId -in @('result', 'host-status', 'theme-state', 'command-status')) {
                $texts = @($_.FindAll($scope, $condition) | Where-Object {
                    $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Text' -and $_.Current.Name
                } | ForEach-Object { $_.Current.Name })
                if ($texts.Count -gt 0) { $text = $texts -join ' ' }
            }
            $rectangle = $current.BoundingRectangle
            [pscustomobject]@{
                id = $current.AutomationId
                name = $current.Name
                role = $current.ControlType.ProgrammaticName
                enabled = $current.IsEnabled
                focused = $current.HasKeyboardFocus
                offscreen = $current.IsOffscreen
                value = $value
                text = $text
                rect = @($rectangle.Left, $rectangle.Top, $rectangle.Width, $rectangle.Height)
            }
        })
        [pscustomobject]@{ schema = 1; nodes = $records } | ConvertTo-Json -Depth 6 -Compress
    } catch {
        [pscustomobject]@{ schema = 1; error = $_.Exception.Message } | ConvertTo-Json -Compress
    }
}

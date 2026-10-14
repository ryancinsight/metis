$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$primary = $null
$sequence = 0
try {
    Set-UiaProgress 'assembly_client' $sequence
    Add-Type -AssemblyName UIAutomationClient
    Set-UiaProgress 'assembly_types' $sequence
    Add-Type -AssemblyName UIAutomationTypes
    $client = [System.Windows.Automation.AutomationElement].Assembly.GetName()
    $providers = [Reflection.AssemblyName]::new()
    $providers.Name = 'UIAutomationClientsideProviders'
    $providers.Version = $client.Version
    $providers.CultureInfo = $client.CultureInfo
    $providers.SetPublicKeyToken($client.GetPublicKeyToken())
    Set-UiaProgress 'provider_load' $sequence
    $loaded = [Reflection.Assembly]::Load($providers)
    if ($loaded.FullName -cne $providers.FullName) {
        throw "UIA providers: $($loaded.FullName) != $($providers.FullName)"
    }
    Set-UiaProgress 'provider_register' $sequence
    $register = [System.Windows.Automation.ClientSettings].GetMethod(
        'RegisterClientSideProviderAssembly', [type[]]@([Reflection.AssemblyName]))
    if ($null -eq $register -or -not $register.IsStatic) {
        throw [MissingMethodException]::new('Public static UIA provider assembly registration is unavailable')
    }
    $null = $register.Invoke($null, [object[]]@($providers))
    $scope = [System.Windows.Automation.TreeScope]::Descendants
    $condition = [System.Windows.Automation.Condition]::TrueCondition
    Set-UiaProgress 'stdin_read' $sequence
    while ($null -ne ($line = [Console]::ReadLine())) {
        try {
            $command = $line | ConvertFrom-Json
            $sequence = [long]$command.sequence
            Set-UiaProgress 'root_from_handle' $sequence
            $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$command.handle)
            if ($null -eq $root) { throw 'Actual UI Automation root is unavailable' }
            Set-UiaProgress 'root_find_all' $sequence
            $nodes = @($root.FindAll($scope, $condition))
            if ($nodes.Count -eq 0 -or $nodes.Count -gt $command.maximum_nodes) { throw 'Actual UIA tree is empty or exceeds its canonical node contract' }
            if ($command.action -eq 'focus') {
                $matched = @($nodes | Where-Object {
                    Set-UiaProgress 'focus_filter_id' $sequence
                    $_.Current.AutomationId -eq $command.id
                })
                if ($matched.Count -ne 1) { throw 'Requested UIA focus control is absent or ambiguous' }
                Set-UiaProgress 'control_set_focus' $sequence
                $matched[0].SetFocus()
            } elseif ($command.action -ne 'snapshot') {
                throw 'Unknown UIA acceptance action'
            }
            $nodeIndex = -1
            $records = @($nodes | ForEach-Object {
                $nodeIndex++
                Set-UiaProgress 'node_current' $sequence $nodeIndex
                $current = $_.Current
                $value = $null
                $pattern = $null
                Set-UiaProgress 'value_pattern' $sequence $nodeIndex
                if ($_.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                    Set-UiaProgress 'value_current' $sequence $nodeIndex
                    $information = $pattern.Current
                    Set-UiaProgress 'value_read' $sequence $nodeIndex
                    $value = $information.Value
                } else {
                    Set-UiaProgress 'range_pattern' $sequence $nodeIndex
                    if ($_.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$pattern)) {
                        Set-UiaProgress 'range_current' $sequence $nodeIndex
                        $information = $pattern.Current
                        Set-UiaProgress 'range_read' $sequence $nodeIndex
                        $value = $information.Value.ToString([Globalization.CultureInfo]::InvariantCulture)
                    }
                }
                Set-UiaProgress 'text_name' $sequence $nodeIndex
                $text = [string]$current.Name
                Set-UiaProgress 'status_id' $sequence $nodeIndex
                if ($current.AutomationId -in @('result', 'host-status', 'theme-state', 'command-status')) {
                    Set-UiaProgress 'status_find_all' $sequence $nodeIndex
                    $texts = @($_.FindAll($scope, $condition) | Where-Object {
                        Set-UiaProgress 'status_text_type' $sequence $nodeIndex
                        if ($_.Current.ControlType.ProgrammaticName -eq 'ControlType.Text') {
                            Set-UiaProgress 'status_filter_name' $sequence $nodeIndex
                            $_.Current.Name
                        } else { $false }
                    } | ForEach-Object {
                        Set-UiaProgress 'status_text_name' $sequence $nodeIndex
                        $_.Current.Name
                    })
                    if ($texts.Count -gt 0) { $text = $texts -join ' ' }
                }
                Set-UiaProgress 'bounding_rectangle' $sequence $nodeIndex
                $rectangle = $current.BoundingRectangle
                Set-UiaProgress 'record_id' $sequence $nodeIndex
                $id = $current.AutomationId
                Set-UiaProgress 'record_name' $sequence $nodeIndex
                $name = $current.Name
                Set-UiaProgress 'record_type' $sequence $nodeIndex
                $role = $current.ControlType.ProgrammaticName
                Set-UiaProgress 'record_enabled' $sequence $nodeIndex
                $enabled = $current.IsEnabled
                Set-UiaProgress 'record_focused' $sequence $nodeIndex
                $focused = $current.HasKeyboardFocus
                Set-UiaProgress 'record_offscreen' $sequence $nodeIndex
                $offscreen = $current.IsOffscreen
                [pscustomobject]@{
                    id = $id
                    name = $name
                    role = $role
                    enabled = $enabled
                    focused = $focused
                    offscreen = $offscreen
                    value = $value
                    text = $text
                    rect = @($rectangle.Left, $rectangle.Top, $rectangle.Width, $rectangle.Height)
                }
            })
            Set-UiaProgress 'response_serialize' $sequence
            $response = [pscustomobject]@{ schema = 1; nodes = $records } | ConvertTo-Json -Depth 6 -Compress
            Set-UiaProgress 'response_output' $sequence
            $response
            Set-UiaProgress 'response_pipeline_returned' $sequence
        } catch {
            $actual = $_
            $diagnosticError = $null
            try { Set-UiaProgress 'actor_error' $sequence } catch { $diagnosticError = $_.Exception.ToString() }
            [pscustomobject]@{ schema = 1; error = $actual.Exception.Message; progress_error = $diagnosticError } | ConvertTo-Json -Compress
        }
        Set-UiaProgress 'stdin_read' $sequence
    }
} catch {
    $primary = $_
    try { [Console]::Error.WriteLine($primary.Exception.ToString()) }
    catch { $primary.Exception.Data['uia_stderr_write'] = $_.Exception }
    throw
} finally {
    Close-UiaProgress $primary
}

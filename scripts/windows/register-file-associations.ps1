[CmdletBinding(SupportsShouldProcess = $true, DefaultParameterSetName = "Register")]
param(
    [Parameter(Mandatory = $true, ParameterSetName = "Register")]
    [ValidateNotNullOrEmpty()]
    [string]$ExecutablePath,

    [Parameter(Mandatory = $true, ParameterSetName = "Unregister")]
    [switch]$Unregister
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$progId = "Mnemosyne.HeapDump"
$extensions = @(".hprof", ".bin")
$classesPath = "Software\Classes"
$classes = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($classesPath)

try {
    if ($Unregister) {
        if (-not $PSCmdlet.ShouldProcess("current user", "Remove Mnemosyne Open With registration")) {
            return
        }

        foreach ($extension in $extensions) {
            $openWith = $classes.OpenSubKey("$extension\OpenWithProgids", $true)
            if ($null -ne $openWith) {
                try {
                    $openWith.DeleteValue($progId, $false)
                }
                finally {
                    $openWith.Close()
                }
            }
        }
        $classes.DeleteSubKeyTree($progId, $false)
        Write-Host "Removed Mnemosyne from Open With for .hprof and .bin."
        return
    }

    $resolvedExecutable = (Resolve-Path -LiteralPath $ExecutablePath).ProviderPath
    if (-not (Test-Path -LiteralPath $resolvedExecutable -PathType Leaf)) {
        throw "Mnemosyne executable was not found."
    }
    if ([System.IO.Path]::GetExtension($resolvedExecutable) -ne ".exe") {
        throw "ExecutablePath must point to Mnemosyne.exe."
    }
    if (-not $PSCmdlet.ShouldProcess("current user", "Register Mnemosyne for .hprof and .bin")) {
        return
    }

    $progIdKey = $classes.CreateSubKey($progId)
    try {
        $progIdKey.SetValue("", "Mnemosyne JVM Heap Dump")

        $iconKey = $progIdKey.CreateSubKey("DefaultIcon")
        try {
            $iconKey.SetValue("", "`"$resolvedExecutable`",0")
        }
        finally {
            $iconKey.Close()
        }

        $commandKey = $progIdKey.CreateSubKey("shell\open\command")
        try {
            $commandKey.SetValue("", "`"$resolvedExecutable`" --open `"%1`"")
        }
        finally {
            $commandKey.Close()
        }
    }
    finally {
        $progIdKey.Close()
    }

    foreach ($extension in $extensions) {
        $openWith = $classes.CreateSubKey("$extension\OpenWithProgids")
        try {
            $openWith.SetValue(
                $progId,
                [byte[]]@(),
                [Microsoft.Win32.RegistryValueKind]::None
            )
        }
        finally {
            $openWith.Close()
        }
    }

    Write-Host "Registered Mnemosyne in Open With for .hprof and .bin."
    Write-Host "Use Windows 'Choose another app' once if Mnemosyne is not shown immediately."
}
finally {
    $classes.Close()
}

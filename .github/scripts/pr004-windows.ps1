$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or -not [Environment]::Is64BitProcess) {
    throw 'Run this probe in 64-bit Windows PowerShell on x64 Windows.'
}

$failedProbes = [Collections.Generic.List[string]]::new()
function Invoke-Probe([string]$Name, [scriptblock]$Measure) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        $result = & $Measure
        $report = [ordered]@{ probe = $Name; state = 'passed'; observations = $result }
    } catch {
        $failedProbes.Add($Name)
        $cause = $_.Exception.GetBaseException()
        $report = [ordered]@{ probe = $Name; state = 'failed'; errorType = $cause.GetType().FullName; hresult = $cause.HResult }
        if ($cause -is [ComponentModel.Win32Exception]) {
            $report['win32Error'] = $cause.NativeErrorCode
        }
        if ($cause.Data.Contains('probeStep')) {
            $report['step'] = $cause.Data['probeStep']
        }
    }
    $timer.Stop()
    $report['elapsedMs'] = $timer.ElapsedMilliseconds
    Write-Output ($report | ConvertTo-Json -Depth 4 -Compress)
}

Invoke-Probe 'cim-os' {
    $os = Get-CimInstance -ClassName Win32_OperatingSystem -Property Caption, BuildNumber, ProductType -OperationTimeoutSec 10
    if (-not $os.BuildNumber -or -not $os.Caption -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        throw 'Missing OS metadata or architecture differs from x64.'
    }
    $user = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        $principal = [Security.Principal.WindowsPrincipal]::new($user)
        [ordered]@{
            edition = $os.Caption
            build = $os.BuildNumber
            productType = $os.ProductType
            arch = $env:PROCESSOR_ARCHITECTURE
            administratorToken = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        }
    } finally {
        $user.Dispose()
    }
}

Invoke-Probe 'event-log' {
    $query = [Diagnostics.Eventing.Reader.EventLogQuery]::new('System', [Diagnostics.Eventing.Reader.PathType]::LogName)
    $query.ReverseDirection = $true
    $reader = [Diagnostics.Eventing.Reader.EventLogReader]::new($query)
    $count = 0
    try {
        while ($count -lt 8) {
            $record = $reader.ReadEvent([TimeSpan]::FromSeconds(2))
            if ($null -eq $record) { break }
            try {
                if ($null -eq $record.RecordId -or $null -eq $record.TimeCreated) {
                    throw 'Event lacks a record identity or timestamp.'
                }
                $count++
            } finally {
                $record.Dispose()
            }
        }
    } finally {
        $reader.Dispose()
    }
    if ($count -eq 0) { throw 'No System log records were available for verification.' }
    [ordered]@{ channel = 'System'; limit = 8; recordsWithIdentityAndTimestamp = $count }
}

Invoke-Probe 'job-object' {
    if (-not ('WinStrataJobProbe' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

public static class WinStrataJobProbe {
    [StructLayout(LayoutKind.Sequential)]
    private struct BasicLimits {
        public long ProcessTime, JobTime;
        public uint Flags;
        public UIntPtr MinWorkingSet, MaxWorkingSet;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass, SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ExtendedLimits {
        public BasicLimits Basic;
        public ulong ReadOperations, WriteOperations, OtherOperations;
        public ulong ReadBytes, WriteBytes, OtherBytes;
        public UIntPtr ProcessMemory, JobMemory, PeakProcessMemory, PeakJobMemory;
    }

    private const uint ActiveProcessLimit = 0x00000008;
    private const uint KillOnJobClose = 0x00002000;
    private const int JobObjectExtendedLimitInformation = 9;
    private const int ExitTimeoutMs = 5000;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateJobObject(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(SafeFileHandle job, int kind, ref ExtendedLimits limits, uint size);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(SafeFileHandle job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool IsProcessInJob(IntPtr process, SafeFileHandle job, out bool member);

    private static Exception NativeError(string step) {
        var error = new Win32Exception(Marshal.GetLastWin32Error());
        error.Data["probeStep"] = step;
        return error;
    }

    private static void Check(bool passed, string step) {
        if (passed) return;
        var error = new InvalidOperationException(step);
        error.Data["probeStep"] = step;
        throw error;
    }

    private static void SetCapacity(SafeFileHandle job, uint capacity) {
        var limits = new ExtendedLimits();
        limits.Basic.Flags = ActiveProcessLimit | KillOnJobClose;
        limits.Basic.ActiveProcessLimit = capacity;
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, ref limits, (uint)Marshal.SizeOf(limits)))
            throw NativeError("set-job-limits");
    }

    private static Process StartIdleChild() {
        var start = new ProcessStartInfo(Path.Combine(Environment.SystemDirectory, "cmd.exe"), "/d /q");
        start.UseShellExecute = false;
        start.CreateNoWindow = true;
        start.RedirectStandardInput = true;
        start.RedirectStandardOutput = true;
        start.RedirectStandardError = true;
        return Process.Start(start);
    }

    private static void Admit(SafeFileHandle job, Process child, string step) {
        if (!AssignProcessToJobObject(job, child.Handle)) throw NativeError(step);
        bool member;
        if (!IsProcessInJob(child.Handle, job, out member)) throw NativeError("query-membership");
        Check(member && !child.HasExited, "verify-live-membership");
    }

    private static void Cleanup(Process child) {
        if (child == null) return;
        using (child) {
            if (!child.HasExited) {
                child.StandardInput.Close();
                Check(child.WaitForExit(ExitTimeoutMs), "idle-child-cleanup");
            }
        }
    }

    public static int CheckLimits() {
        Process first = null, second = null, control = null;
        try {
            int denialCode;
            using (var job = CreateJobObject(IntPtr.Zero, null)) {
                if (job.IsInvalid) throw NativeError("create-job");
                SetCapacity(job, 1);
                first = StartIdleChild();
                Admit(job, first, "admit-first");
                second = StartIdleChild();
                bool admitted = AssignProcessToJobObject(job, second.Handle);
                denialCode = Marshal.GetLastWin32Error();
                Check(!admitted && denialCode != 0, "reject-second-at-capacity-one");

                SetCapacity(job, 2);
                control = StartIdleChild();
                Admit(job, control, "admit-second-at-capacity-two");
                Check(!first.HasExited && !control.HasExited, "verify-both-live-before-close");
            }

            Check(first.WaitForExit(ExitTimeoutMs) && control.WaitForExit(ExitTimeoutMs), "verify-both-exited-on-close");
            return denialCode;
        } finally {
            try { Cleanup(first); }
            finally {
                try { Cleanup(second); }
                finally { Cleanup(control); }
            }
        }
    }
}
'@
    }
    $measurement = [Diagnostics.Stopwatch]::StartNew()
    $denial = [WinStrataJobProbe]::CheckLimits()
    $measurement.Stop()
    [ordered]@{
        rejectedAtLimit = 1; rejectedAssignmentWin32Error = $denial; admittedAtLimit = 2
        liveMembersBeforeClose = 2; membersExitedAfterClose = 2; exerciseMs = $measurement.ElapsedMilliseconds
    }
}

if ($failedProbes.Count -gt 0) {
    throw ('Windows probes failed: ' + ($failedProbes -join ', '))
}

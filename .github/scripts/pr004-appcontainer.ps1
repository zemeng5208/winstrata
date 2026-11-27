$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or $PSVersionTable.PSEdition -ne 'Desktop') {
    throw 'Use 64-bit Windows PowerShell on x64 Windows.'
}

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;

public static class WinStrataContainerProbe {
    [StructLayout(LayoutKind.Sequential)]
    private struct Capabilities {
        public IntPtr Sid, Groups;
        public uint Count, Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Startup {
        public int Size;
        public IntPtr Reserved, Desktop, Title;
        public int X, Y, XSize, YSize, XChars, YChars, Fill, Flags;
        public ushort Show, ReservedSize;
        public IntPtr ReservedData, Input, Output, Error, Attributes;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Child {
        public IntPtr Process, Thread;
        public uint ProcessId, ThreadId;
    }

    [DllImport("userenv.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int CreateAppContainerProfile(string name, string display, string description,
        IntPtr capabilities, uint count, out IntPtr sid);
    [DllImport("userenv.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int DeleteAppContainerProfile(string name);
    [DllImport("userenv.dll", SetLastError = true)]
    private static extern bool CreateEnvironmentBlock(out IntPtr environment, IntPtr token, bool inherit);
    [DllImport("userenv.dll", SetLastError = true)]
    private static extern bool DestroyEnvironmentBlock(IntPtr environment);
    [DllImport("kernel32.dll")]
    private static extern IntPtr GetCurrentProcess();
    [DllImport("advapi32.dll")]
    private static extern IntPtr FreeSid(IntPtr sid);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool InitializeProcThreadAttributeList(IntPtr list, int count, uint flags, ref IntPtr size);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool UpdateProcThreadAttribute(IntPtr list, uint flags, IntPtr attribute, IntPtr value,
        IntPtr size, IntPtr oldValue, IntPtr returnedSize);
    [DllImport("kernel32.dll")]
    private static extern void DeleteProcThreadAttributeList(IntPtr list);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    private static extern bool CreateProcessW(string file, StringBuilder command, IntPtr processSecurity,
        IntPtr threadSecurity, bool inherit, uint flags, IntPtr environment, string directory, ref Startup startup, out Child child);
    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool GetTokenInformation(IntPtr token, int kind, IntPtr buffer, int length, out int needed);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr process, out uint code);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr process, uint code);
    [DllImport("kernel32.dll")]
    private static extern bool CloseHandle(IntPtr handle);

    private static void Require(bool success, string step) {
        if (success) return;
        int code = Marshal.GetLastWin32Error();
        throw new Win32Exception(code, step + " (Win32 " + code + "): " + new Win32Exception(code).Message);
    }

    public static IntPtr Create(string name) {
        IntPtr sid;
        Marshal.ThrowExceptionForHR(CreateAppContainerProfile(name, name, "WinStrata fixture", IntPtr.Zero, 0, out sid));
        return sid;
    }

    public static void Remove(string name, IntPtr sid) {
        try { Marshal.ThrowExceptionForHR(DeleteAppContainerProfile(name)); }
        finally { FreeSid(sid); }
    }

    private static void CheckToken(IntPtr process, IntPtr sid) {
        const int TokenAppContainerSid = 31;
        IntPtr token;
        Require(OpenProcessToken(process, 0x0008, out token), "OpenProcessToken");
        try {
            int size;
            GetTokenInformation(token, TokenAppContainerSid, IntPtr.Zero, 0, out size);
            if (size <= 0) throw new InvalidOperationException("Missing AppContainer SID size.");
            IntPtr info = Marshal.AllocHGlobal(size);
            try {
                Require(GetTokenInformation(token, TokenAppContainerSid, info, size, out size), "Read AppContainer SID");
                IntPtr observed = Marshal.ReadIntPtr(info);
                if (observed == IntPtr.Zero || !new SecurityIdentifier(sid).Equals(new SecurityIdentifier(observed)))
                    throw new InvalidOperationException("Child token does not carry the requested AppContainer SID.");
            } finally { Marshal.FreeHGlobal(info); }
        } finally { CloseHandle(token); }
    }

    public static uint ReadFixture(IntPtr sid, string executable, string fixture) {
        const int SecurityCapabilitiesAttribute = 0x00020009;
        const uint LaunchFlags = 0x00080000 | 0x00000004 | 0x08000000 | 0x00000400;
        string system = Environment.SystemDirectory;
        var cmd = new StringBuilder(String.Format("\"{0}\" \"{1}\"", executable, fixture));
        IntPtr listSize = IntPtr.Zero;
        InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref listSize);
        if (listSize.ToInt64() <= 0) throw new InvalidOperationException("Missing process attribute list size.");
        IntPtr list = Marshal.AllocHGlobal(listSize);
        IntPtr caps = IntPtr.Zero, environment = IntPtr.Zero;
        bool initialized = false;
        try {
            Require(InitializeProcThreadAttributeList(list, 1, 0, ref listSize), "Initialize process attributes");
            initialized = true;
            var attributes = new Capabilities { Sid = sid };
            int size = Marshal.SizeOf(attributes);
            caps = Marshal.AllocHGlobal(size);
            Marshal.StructureToPtr(attributes, caps, false);
            Require(UpdateProcThreadAttribute(list, 0, (IntPtr)SecurityCapabilitiesAttribute, caps, (IntPtr)size,
                IntPtr.Zero, IntPtr.Zero), "Set security capabilities");

            IntPtr parentToken;
            Require(OpenProcessToken(GetCurrentProcess(), 0x0008 | 0x0002, out parentToken), "Open current user token");
            try {
                Require(CreateEnvironmentBlock(out environment, parentToken, false), "Create clean user environment");
            } finally { CloseHandle(parentToken); }
            var startup = new Startup { Attributes = list };
            startup.Size = Marshal.SizeOf(startup);
            Child child;
            Require(CreateProcessW(executable, cmd, IntPtr.Zero, IntPtr.Zero, false, LaunchFlags, environment, system,
                ref startup, out child), "Create fixture child");
            try {
                CheckToken(child.Process, sid);
                Require(ResumeThread(child.Thread) != UInt32.MaxValue, "Resume fixture child");
                uint wait = WaitForSingleObject(child.Process, 10000);
                if (wait == 258) throw new TimeoutException("Fixture child exceeded 10 seconds.");
                Require(wait == 0, "Wait for fixture child");
                uint code;
                Require(GetExitCodeProcess(child.Process, out code), "Read fixture child exit code");
                return code;
            } finally {
                try {
                    if (WaitForSingleObject(child.Process, 0) == 258) {
                        bool stopped = TerminateProcess(child.Process, 1);
                        if (!stopped && WaitForSingleObject(child.Process, 0) != 0)
                            throw new InvalidOperationException("Fixture child cleanup failed.");
                        if (WaitForSingleObject(child.Process, 5000) != 0)
                            throw new TimeoutException("Fixture child did not exit.");
                    }
                } finally {
                    CloseHandle(child.Thread);
                    CloseHandle(child.Process);
                }
            }
        } finally {
            if (initialized) DeleteProcThreadAttributeList(list);
            Marshal.FreeHGlobal(list);
            Marshal.FreeHGlobal(caps);
            if (environment != IntPtr.Zero) DestroyEnvironmentBlock(environment);
        }
    }
}
'@

function Set-FixtureAccess([string]$Path, [Security.Principal.SecurityIdentifier]$OwnerSid,
    [Security.Principal.SecurityIdentifier]$PackageSid, [Security.AccessControl.FileSystemRights]$Grant,
    [switch]$Directory) {
    if ($Directory) { $acl = [Security.AccessControl.DirectorySecurity]::new() }
    else { $acl = [Security.AccessControl.FileSecurity]::new() }
    $acl.SetAccessRuleProtection($true, $false)
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($OwnerSid, 'FullControl', 'Allow'))
    if ([int]$Grant -ne 0) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($PackageSid, $Grant, 'Allow'))
    }
    if ($Directory) { [IO.Directory]::SetAccessControl($Path, $acl) }
    else { [IO.File]::SetAccessControl($Path, $acl) }
}

function Build-FixtureReader([string]$Folder) {
    $locator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $install = & $locator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $install) { throw 'No installed x64 MSVC compiler was found.' }
    $vc = Join-Path $install 'VC\Auxiliary\Build\vcvars64.bat'
    $source = @'
#include <windows.h>
#include <string.h>
int wmain(int argc, wchar_t **argv) {
    static const char marker[] = "WinStrata synthetic fixture";
    char content[sizeof(marker)];
    DWORD count, error;
    HANDLE file;
    if (argc != 2) return ERROR_INVALID_PARAMETER;
    file = CreateFileW(argv[1], GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return (int)GetLastError();
    if (!ReadFile(file, content, (DWORD)sizeof(content), &count, NULL)) error = GetLastError();
    else if (count != sizeof(marker) - 1 || memcmp(content, marker, count) != 0) error = ERROR_INVALID_DATA;
    else error = ERROR_SUCCESS;
    CloseHandle(file);
    return (int)error;
}
'@
    [IO.File]::WriteAllText([IO.Path]::Combine($Folder, 'reader.c'), $source, [Text.Encoding]::ASCII)
    Push-Location -LiteralPath $Folder
    try {
        $command = 'call "' + $vc + '" >nul && cl.exe /nologo /W4 /WX /O1 /MT reader.c /link /OUT:reader.exe'
        & $env:ComSpec /d /v:off /c $command | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Native fixture reader compilation failed.' }
    } finally { Pop-Location }
}

$timer = [Diagnostics.Stopwatch]::StartNew()
$name = 'WinStrata.Probe.' + [Guid]::NewGuid().ToString('N')
$sid = [WinStrataContainerProbe]::Create($name)
$folder = $null
try {
    $packageSid = [Security.Principal.SecurityIdentifier]::new($sid)
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    try { $ownerSid = $identity.User } finally { $identity.Dispose() }
    $base = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
    $folder = (New-Item -Path ([IO.Path]::Combine($base, $name)) -ItemType Directory).FullName
    Build-FixtureReader $folder
    $executable = [IO.Path]::Combine($folder, 'reader.exe')
    $reference = [IO.Path]::Combine($folder, 'control.txt')
    $fixture = [IO.Path]::Combine($folder, 'fixture.txt')
    foreach ($path in @($reference, $fixture)) {
        [IO.File]::WriteAllText($path, 'WinStrata synthetic fixture', [Text.Encoding]::ASCII)
    }

    Set-FixtureAccess $folder $ownerSid $packageSid ReadAndExecute -Directory
    Set-FixtureAccess $executable $ownerSid $packageSid ReadAndExecute
    Set-FixtureAccess $reference $ownerSid $packageSid Read
    $control = [WinStrataContainerProbe]::ReadFixture($sid, $executable, $reference)
    if ($control -ne 0) { throw ('Always-granted control read failed with code ' + $control) }
    $codes = foreach ($grant in @(0, [Security.AccessControl.FileSystemRights]::Read, 0)) {
        Set-FixtureAccess $fixture $ownerSid $packageSid $grant
        [WinStrataContainerProbe]::ReadFixture($sid, $executable, $fixture)
    }
    if (($codes -join ',') -ne '5,0,5') {
        throw ('Unexpected native read results for denied/granted/revoked phases: ' + ($codes -join ','))
    }
} finally {
    try {
        if ($null -ne $folder) { [IO.Directory]::Delete($folder, $true) }
    } finally {
        [WinStrataContainerProbe]::Remove($name, $sid)
    }
}
$timer.Stop()
[ordered]@{
    probe = 'appcontainer-file'; state = 'passed'; requestedExtraCapabilities = 0; controlRead = $control
    readExitCodes = $codes; phases = @('package-grant-absent', 'package-grant-present', 'package-grant-revoked')
    elapsedMs = $timer.ElapsedMilliseconds
} | ConvertTo-Json -Depth 3 -Compress

param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot),
    [string]$OutputDirectory = (Join-Path $RepositoryRoot 'target\word365-auto'),
    [string]$Fixture = '',
    [int]$Limit = 0,
    [bool]$ShowWord = $true,
    [bool]$SkipExisting = $true,
    [switch]$ValidateOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$apartmentState = [Threading.Thread]::CurrentThread.ApartmentState
if ($apartmentState -ne [Threading.ApartmentState]::STA) {
    throw "Microsoft Word automation requires STA; current state is $apartmentState. Run with powershell.exe -STA -ExecutionPolicy Bypass -File .\tools\export-word365-baselines.ps1"
}

Add-Type -AssemblyName Microsoft.Office.Interop.Word
Add-Type -AssemblyName office

if (-not ('WordBaselineEarlyBoundExporter' -as [type])) {
    $interopAssembly = [Microsoft.Office.Interop.Word.ApplicationClass].Assembly.Location
    $officeAssembly = [Microsoft.Office.Core.MsoAutomationSecurity].Assembly.Location
    $source = @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using Word = Microsoft.Office.Interop.Word;
using Office = Microsoft.Office.Core;

public static class WordBaselineEarlyBoundExporter
{
    [STAThread]
    public static void Export(string inputPath, string outputPath, bool visible)
    {
        Word.Application application = null;
        Word.Document document = null;
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(outputPath));
            application = new Word.Application();
            application.Visible = visible;
            application.DisplayAlerts = Word.WdAlertLevel.wdAlertsNone;
            application.AutomationSecurity = Office.MsoAutomationSecurity.msoAutomationSecurityForceDisable;
            application.Options.ConfirmConversions = false;
            application.Options.UpdateLinksAtOpen = false;

            document = application.Documents.Open(
                FileName: inputPath,
                ConfirmConversions: false,
                ReadOnly: true,
                AddToRecentFiles: false,
                Visible: visible,
                OpenAndRepair: false,
                NoEncodingDialog: true
            );

            document.ExportAsFixedFormat(
                OutputFileName: outputPath,
                ExportFormat: Word.WdExportFormat.wdExportFormatPDF,
                OpenAfterExport: false,
                OptimizeFor: Word.WdExportOptimizeFor.wdExportOptimizeForPrint,
                Range: Word.WdExportRange.wdExportAllDocument,
                From: 1,
                To: 1,
                Item: Word.WdExportItem.wdExportDocumentContent,
                IncludeDocProps: true,
                KeepIRM: true,
                CreateBookmarks: Word.WdExportCreateBookmarks.wdExportCreateNoBookmarks,
                DocStructureTags: true,
                BitmapMissingFonts: true,
                UseISO19005_1: false
            );

            if (!File.Exists(outputPath) || new FileInfo(outputPath).Length == 0)
            {
                throw new IOException("Word returned without creating a non-empty PDF.");
            }
        }
        finally
        {
            if (document != null)
            {
                ((Word._Document)document).Close(
                    SaveChanges: Word.WdSaveOptions.wdDoNotSaveChanges
                );
                Marshal.FinalReleaseComObject(document);
            }
            if (application != null)
            {
                ((Word._Application)application).Quit(
                    SaveChanges: Word.WdSaveOptions.wdDoNotSaveChanges
                );
                Marshal.FinalReleaseComObject(application);
            }
        }
    }
}
'@
    Add-Type -TypeDefinition $source -Language CSharp -ReferencedAssemblies $interopAssembly, $officeAssembly
}

Write-Host 'Early-bound Word interop compiled successfully.'
if ($ValidateOnly) { return }

$fixturesDirectory = Join-Path $RepositoryRoot 'fixtures'
if ($Fixture) {
    $files = @(Get-ChildItem $fixturesDirectory -Filter $Fixture -File)
    if ($files.Count -eq 0) { throw "No fixture matched '$Fixture'." }
}
else {
    $files = @(Get-ChildItem $fixturesDirectory -Filter '*.rtf' -File | Sort-Object Name)
}
if ($Limit -gt 0) { $files = @($files | Select-Object -First $Limit) }

$completed = 0
$failed = @()
$position = 0
foreach ($file in $files) {
    $position++
    $pdfPath = Join-Path $OutputDirectory ($file.BaseName + '.pdf')
    if ($SkipExisting -and (Test-Path $pdfPath) -and (Get-Item $pdfPath).Length -gt 0) {
        $completed++
        Write-Host ("[{0}/{1}] Already exists: {2}" -f $position, $files.Count, $file.Name)
        continue
    }
    Write-Host ("[{0}/{1}] Exporting {2}..." -f $position, $files.Count, $file.Name)
    try {
        [WordBaselineEarlyBoundExporter]::Export($file.FullName, $pdfPath, $ShowWord)
        $completed++
        Write-Host ("Created {0}" -f $pdfPath)
    }
    catch {
        $failed += [pscustomobject]@{ File = $file.Name; Error = $_.Exception.Message }
        Write-Warning ("Failed: {0}: {1}" -f $file.Name, $_.Exception.Message)
    }
}

if ($failed.Count -gt 0) {
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $failed | Export-Csv (Join-Path $OutputDirectory 'failures.csv') -NoTypeInformation
}

Write-Host ("Exported {0} of {1} fixtures to {2}" -f $completed, $files.Count, $OutputDirectory)
if ($failed.Count -gt 0) { exit 2 }

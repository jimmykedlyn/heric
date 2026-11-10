# Windows twin of scripts/check-docs-structure.sh.
# CI runs the bash script; this one exists so the same check can be run locally
# without a POSIX shell.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$failed = $false

function Fail([string]$message) {
    Write-Host "ERROR: $message" -ForegroundColor Red
    $script:failed = $true
}

function Get-MarkdownFiles([string]$dir) {
    Get-ChildItem $dir -Recurse -File -Filter '*.md' |
        ForEach-Object { $_.FullName.Substring($dir.Length + 1).Replace('\', '/') } |
        Sort-Object
}

$languages = @('en', 'pt-BR', 'es')
$reference = Get-MarkdownFiles (Join-Path $root 'docs/book/en/src')

foreach ($lang in $languages[1..2]) {
    $other = Get-MarkdownFiles (Join-Path $root "docs/book/$lang/src")
    $diff = Compare-Object $reference $other
    if ($diff) {
        $diff | ForEach-Object { Write-Host ("  {0} {1}" -f $_.SideIndicator, $_.InputObject) }
        Fail "docs/book/$lang/src has a different file set from docs/book/en/src"
    }
}

function Get-SummaryShape([string]$file) {
    Select-String -Path $file -Pattern '^(\s*)[-*] \[[^\]]*\]\(([^)]*)\)' |
        ForEach-Object { $_.Matches[0].Groups[1].Value + '|' + $_.Matches[0].Groups[2].Value }
}

$referenceSummary = Get-SummaryShape (Join-Path $root 'docs/book/en/src/SUMMARY.md')
foreach ($lang in $languages[1..2]) {
    $otherSummary = Get-SummaryShape (Join-Path $root "docs/book/$lang/src/SUMMARY.md")
    if (Compare-Object $referenceSummary $otherSummary) {
        Fail "docs/book/$lang/src/SUMMARY.md differs in structure from the English one"
    }
}

function Get-HeadingShape([string]$file) {
    Select-String -Path $file -Pattern '^(#{1,6}) ' | ForEach-Object { $_.Matches[0].Groups[1].Value }
}

foreach ($base in @('README', 'CONTRIBUTING')) {
    $refShape = Get-HeadingShape (Join-Path $root "$base.md")
    foreach ($lang in $languages[1..2]) {
        $other = Join-Path $root "$base.$lang.md"
        if (-not (Test-Path $other)) {
            Fail "missing $base.$lang.md"
            continue
        }
        if (Compare-Object $refShape (Get-HeadingShape $other)) {
            Fail "$base.$lang.md heading structure differs from $base.md"
        }
    }
}

function Test-Selector([string]$file, [string[]]$expected) {
    $head = Get-Content $file -TotalCount 2
    if (-not ($head -match 'English')) {
        Fail "$file is missing the language selector"
    }
    $content = Get-Content $file -Raw
    foreach ($target in $expected) {
        if (-not $content.Contains($target)) {
            Fail "$file selector misses $target"
        }
    }
}

Test-Selector (Join-Path $root 'README.md') @('README.pt-BR.md', 'README.es.md')
Test-Selector (Join-Path $root 'README.pt-BR.md') @('README.md', 'README.es.md')
Test-Selector (Join-Path $root 'README.es.md') @('README.md', 'README.pt-BR.md')
Test-Selector (Join-Path $root 'CONTRIBUTING.md') @('CONTRIBUTING.pt-BR.md', 'CONTRIBUTING.es.md')
Test-Selector (Join-Path $root 'CONTRIBUTING.pt-BR.md') @('CONTRIBUTING.md', 'CONTRIBUTING.es.md')
Test-Selector (Join-Path $root 'CONTRIBUTING.es.md') @('CONTRIBUTING.md', 'CONTRIBUTING.pt-BR.md')

if ($failed) {
    Write-Host 'Documentation structure check failed.' -ForegroundColor Red
    exit 1
}

Write-Host 'Documentation structure check passed.' -ForegroundColor Green

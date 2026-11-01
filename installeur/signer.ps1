# Signe des fichiers au nom de Lopjoy (Authenticode).
# - Si un certificat de signature de code « Lopjoy » est installé (acheté auprès d'une autorité), il est utilisé.
# - Sinon, un certificat auto-signé « CN=Lopjoy » est créé dans le magasin de l'utilisateur.
#   Il marque bien les fichiers comme signés par Lopjoy, mais Windows SmartScreen continuera
#   d'avertir les autres utilisateurs tant que le certificat ne vient pas d'une autorité reconnue.
param([Parameter(Mandatory = $true)][string[]]$Fichiers)
$ErrorActionPreference = "Stop"

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -like "*CN=Lopjoy*" -and $_.NotAfter -gt (Get-Date) } |
    Sort-Object NotAfter | Select-Object -Last 1
if (-not $cert) {
    Write-Host "   Aucun certificat Lopjoy trouvé : création d'un certificat auto-signé (valable 5 ans)." -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Lopjoy, O=Lopjoy, C=FR" `
        -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddYears(5) -KeyUsage DigitalSignature `
        -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256
}

foreach ($f in $Fichiers) {
    # Horodatage : la signature reste valide après l'expiration du certificat (nécessite Internet)
    try {
        Set-AuthenticodeSignature -FilePath $f -Certificate $cert -HashAlgorithm SHA256 -TimestampServer "http://timestamp.digicert.com" | Out-Null
    } catch {
        # Sans Internet, signature sans horodatage
        Set-AuthenticodeSignature -FilePath $f -Certificate $cert -HashAlgorithm SHA256 | Out-Null
    }
    # Avec un certificat auto-signé, Windows signale une chaîne « non approuvée » : la signature est pourtant posée.
    $sig = Get-AuthenticodeSignature -FilePath $f
    if (-not $sig.SignerCertificate) { throw "Signature impossible pour $f : $($sig.StatusMessage)" }
    Write-Host "   Signé : $f ($($sig.SignerCertificate.Subject))"
}

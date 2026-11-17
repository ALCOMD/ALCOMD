/** Single-extension HTTPS source previews; authorization remains in Core. */
export interface ExtensionInstallationSource { url: string; version: string }
export interface ExtensionSourcePreview {
    url: string;
    extensionId: string;
    displayName: string;
    publisherFingerprint: string;
    description?: string;
    versions: { version: string; apiMajor: number }[];
    sourceFingerprint: string;
}

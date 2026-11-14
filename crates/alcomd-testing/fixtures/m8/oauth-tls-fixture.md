# Public OAuth TLS test fixture

`oauth-localhost.test-certificate.der` and `oauth-localhost.test-key.pkcs8` are a deliberately
public disposable RSA-2048 TLS pair for the M8 isolated tests, not production signing material.
The certificate is a self-signed **end entity, not a CA**, with DNS SAN localhost, digital
signature/server-auth use, and validity 2020-01-01 through 2045-01-01. Never use this known key
for real service authentication or install it into the OS certificate store.

Generated in memory with .NET CertificateRequest/CreateSelfSigned and exported directly as DER
and PKCS8. No Windows certificate store or global trust settings were changed. Integration tests
pass this certificate explicitly to their own reqwest client; an ordinary client must reject it.
Only test targets include these bytes. Runtime source, product packages and default configuration
must not refer to or trust this fixture.

Matching server certificate SAN and private key is checked at startup. Chain trust and validity
remain each connecting client's responsibility; a server cannot infer every client's trust store.
Neither tests nor production use an invalid-certificate bypass or cleartext fallback.

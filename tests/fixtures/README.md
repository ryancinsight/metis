# QUIC TLS fixtures

The public test-only certificate chain and key come from Rustls test CA revision
`bcf1c74727b9575208cc019fdc323ea834b19413`, as used by the Moirai TLS tests:
<https://github.com/rustls/rustls/tree/bcf1c74727b9575208cc019fdc323ea834b19413/test-ca/rsa-2048>.
The standalone Metis test package includes this immutable material so no sibling
checkout is required. These keys are public test fixtures, never production
credentials. Tests trust only the committed root and use real loopback UDP/TLS.

#!/bin/bash
# Generate self-signed test certificates for TLS adversarial tests.
# These are test-only certs — never use in production.
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Generating CA key and cert..."
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "$DIR/ca.key" -out "$DIR/ca.crt" \
  -days 3650 -subj "/CN=m2c-test-ca"

echo "Generating server key and CSR..."
openssl req -newkey rsa:2048 -nodes \
  -keyout "$DIR/server.key" -out "$DIR/server.csr" \
  -subj "/CN=localhost"

echo "Signing server cert with CA..."
openssl x509 -req -in "$DIR/server.csr" \
  -CA "$DIR/ca.crt" -CAkey "$DIR/ca.key" -CAcreateserial \
  -out "$DIR/server.crt" -days 3650

rm -f "$DIR/server.csr" "$DIR/ca.srl"

echo "Certs generated in $DIR/"
echo "  ca.crt     — CA certificate (trust this in client)"
echo "  server.crt — Server certificate"
echo "  server.key — Server private key"

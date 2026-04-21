#!/bin/bash
set -e

# Version and architecture setup
VERSION="1.0.33"
ARCH=$(uname -m)
OS="linux"

# SHA-256 digests (Retrieved from GitHub API)
HASH_x86_64="48802662bc745aed60207c6ed6465d9fed428b1e53532045689d89bcad19bdd9"
HASH_aarch64="8b0d13d11f607c1da18be921096de3f85af67520b305aad425c74dd4f6775697"

# Select architecture suffix and hash
case "$ARCH" in
    x86_64)
        TT_ARCH="x86_64"
        TT_HASH=$HASH_x86_64
        ;;
    aarch64|arm64)
        TT_ARCH="aarch64"
        TT_HASH=$HASH_aarch64
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

RELEASENAME="trusttunnel-v${VERSION}-${OS}-${TT_ARCH}"
FILENAME="${RELEASENAME}.tar.gz"
URL="https://github.com/TrustTunnel/TrustTunnel/releases/download/v${VERSION}/${FILENAME}"

echo "Downloading TrustTunnel v${VERSION} for ${TT_ARCH}..."
curl -L -o "$FILENAME" "$URL"

echo "Verifying SHA-256 hash..."
echo "${TT_HASH}  ${FILENAME}" | sha256sum -c -

echo "Extracting binary..."
tar -xzf "$FILENAME"
cp "${RELEASENAME}/trusttunnel_endpoint" trusttunnel_endpoint
chmod +x trusttunnel_endpoint

# Cleanup
rm "$FILENAME"

echo "TrustTunnel v${VERSION} (${TT_ARCH}) downloaded and verified successfully."

#!/bin/bash
set -e

# Version and architecture setup
VERSION="1.0.49"
ARCH=$(uname -m)
OS="linux"

case "$ARCH" in
    x86_64)
        TT_CLIENT_ARCH="x86_64"
        ;;
    aarch64|arm64)
        TT_CLIENT_ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

RELEASENAME="trusttunnel_client-v${VERSION}-${OS}-${TT_CLIENT_ARCH}"
FILENAME="${RELEASENAME}.tar.gz"
URL="https://github.com/TrustTunnel/TrustTunnelClient/releases/download/v${VERSION}/${FILENAME}"

echo "Downloading TrustTunnelClient v${VERSION} for ${TT_CLIENT_ARCH}..."
curl -L -o "$FILENAME" "$URL"

echo "Extracting TrustTunnelClient binary..."
tar -xzf "$FILENAME"
cp "${RELEASENAME}/trusttunnel_client" trusttunnel_client
chmod +x trusttunnel_client

# Cleanup
rm "$FILENAME"

echo "TrustTunnelClient v${VERSION} (${TT_CLIENT_ARCH}) downloaded successfully."

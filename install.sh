set -e

echo "Installing..."

NAME=track
INSTALL_DIR="/usr/local/bin"
SYSTEM=$(uname -a)

if [[ $SYSTEM == "Linux"* ]]; then
  curl -sfL https://github.com/maxxgl/track/releases/download/v0.0.1-alpha.1/track-cli-linux > $INSTALL_DIR/$NAME
elif [[ $SYSTEM == "Darwin"* ]]; then
  curl -sfL https://github.com/maxxgl/track/releases/download/v0.0.1-alpha.1/track-cli-mac > $INSTALL_DIR/$NAME
else
  echo "Installation failed, system not supported"
  echo "Supported systems:"
  echo "x86: Mac, Linux"
fi

chmod 755 $INSTALL_DIR/$NAME

echo "Track CLI Tool Successfully Installed"

set -euo pipefail

git describe --tags --abbrev=0
read -p "Enter Tag: " TAG
git tag $TAG
SHA=$(git rev-parse --short HEAD)

# MAC BUILD ------------------------------------------------------------------
MAC_BIN_NAME="track-cli-mac-$TAG"
DIR_NAME="/tmp/target-$TAG"

mkdir $DIR_NAME
cargo build --release --target-dir $DIR_NAME
mv "$DIR_NAME/release/track-cli" "output/$MAC_BIN_NAME"
rm -rf $DIR_NAME

# LINUX BUILD ----------------------------------------------------------------
LIN_BIN_NAME="track-cli-linux-$TAG"

docker build . -t track-cli-release
docker run --rm -v "$PWD/output":/out track-cli-release

echo "Tag $TAG @ $SHA new builds created."

set -euo pipefail

git describe --tags --abbrev=0
read -p "Enter Tag: " TAG
git tag $TAG
git push origin $TAG
SHA=$(git rev-parse --short HEAD)

# MAC BUILD ------------------------------------------------------------------
MAC_BIN_NAME="track-cli-mac"
DIR_NAME="/tmp/target-$TAG"
mkdir "output/$TAG"

mkdir $DIR_NAME
cargo build --release --target-dir $DIR_NAME
mv "$DIR_NAME/release/track-cli" "output/$TAG/$MAC_BIN_NAME"
rm -rf $DIR_NAME

# LINUX BUILD ----------------------------------------------------------------
LIN_BIN_NAME="track-cli-linux"

docker build . -t track-cli-release
docker run --rm -v "$PWD/output/$TAG":/out track-cli-release

echo "Tag $TAG @ $SHA new builds created."

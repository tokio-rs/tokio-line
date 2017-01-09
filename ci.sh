set -e

pushd multiplexed
cargo update
cargo test
popd

pushd simple
cargo update
cargo test
popd

pushd streaming
cargo update
cargo test
popd

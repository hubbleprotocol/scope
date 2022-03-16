#!/usr/bin/env bash
# TODO - use `cargo metadata` here
cargo_version=$(grep -m 1 'version = "' Cargo.toml | cut -d'=' -f2 | cut -d'"' -f2)
build_version=$cargo_version

if [[ "$cargo_version" =~ ^.*-SNAPSHOT$ ]]; then
    build_version="$build_version-$(date +%s).${GITHUB_RUN_ID}"
fi

release_version="${build_version%-SNAPSHOT*}"

echo "Build version: $build_version"
echo "Release version: $release_version"

echo "::set-output name=build_version::$build_version"
echo "::set-output name=release_version::$release_version"

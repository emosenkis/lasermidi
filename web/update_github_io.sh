#!/bin/bash

set -e -o pipefail

cargo web build --target-webasm-emscripten --release
cp -av static/* target/wasm32-unknown-emscripten/release/lasermidi_web.wasm ../docs/
cp -av target/wasm32-unknown-emscripten/release/lasermidi_web.js ../docs/js/app.js

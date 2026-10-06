#!/bin/sh
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
out="$root/target/mgba-shot"
mkdir -p "$out"
if [ ! -d "$out/mgba-src" ]; then
    git clone -q --depth 1 --branch 0.10.5 https://github.com/mgba-emu/mgba.git "$out/mgba-src"
fi
if [ ! -f "$out/mgba-build/libmgba.a" ]; then
    cmake -S "$out/mgba-src" -B "$out/mgba-build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
        -DBUILD_QT=OFF -DBUILD_SDL=OFF -DBUILD_SHARED=OFF -DBUILD_STATIC=ON -DM_CORE_GB=OFF \
        -DUSE_FFMPEG=OFF -DUSE_LUA=OFF -DUSE_EDITLINE=OFF -DUSE_DEBUGGERS=OFF -DUSE_LIBZIP=OFF -DUSE_MINIZIP=OFF \
        -DUSE_LZMA=OFF -DUSE_ELF=OFF -DUSE_PNG=OFF -DUSE_ZLIB=OFF -DUSE_SQLITE3=OFF -DUSE_DISCORD_RPC=OFF \
        -DBUILD_GL=OFF -DBUILD_GLES2=OFF -DBUILD_GLES3=OFF -DUSE_EPOXY=OFF > "$out/cmake.log"
    cmake --build "$out/mgba-build" -j 8 > "$out/build.log"
fi
cc -O2 -I "$out/mgba-src/include" -I "$out/mgba-build/include" "$(dirname "$0")/shot.c" \
    "$out/mgba-build/libmgba.a" -lm -lpthread $(if [ "$(uname)" = Darwin ]; then echo "-framework CoreFoundation -framework Foundation"; fi) -o "$out/mgba-shot"
echo "$out/mgba-shot"

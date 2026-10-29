#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

pot=po/pipewire-effects.pot

for tool in xgettext msgmerge; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "update-po: $tool is missing; install the gettext package" >&2
        exit 2
    fi
done

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

mapfile -t sources < <(grep -vE '^[[:space:]]*(#|$)' po/POTFILES.in)

xgettext \
    --language=C \
    --from-code=UTF-8 \
    --keyword=t:1 \
    --keyword=tr:1 \
    --keyword=tn:1,2 \
    --add-comments=TRANSLATORS \
    --package-name="PipeWire Effects" \
    --package-version="$version" \
    --msgid-bugs-address="https://github.com/D1ZZY4/PipewireEffects/issues" \
    --output="$pot" \
    "${sources[@]}"

for po in po/*.po; do
    [ -e "$po" ] || continue
    msgmerge --update --backup=none "$po" "$pot"
done

echo "update-po: wrote $pot from ${#sources[@]} files"

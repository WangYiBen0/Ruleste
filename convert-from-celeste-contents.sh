#!/usr/bin/env bash
# convert-from-celeste-contents.sh — 把原版 Celeste `Content/` 转成本项目
# 的 pack 布局（`maps/<pack>/` + `resources/<pack>/<namespace>/`）。
# 官方内容不参与分发，只处理你本地的副本。
#
# 用法：
#   convert-from-celeste-contents.sh < Content/ dir > < pack_name > < output/ >
# 例：
#   ./convert-from-celeste-contents.sh references/Celeste/Content Celeste .
# 产出：
#   ./maps/Celeste/*.bin
#   ./resources/Celeste/Celeste/textures/...      (Graphics 直拷贝 + 改名)
#   ./resources/Celeste/Celeste/texts/...         (Dialog)
#   ./resources/Celeste/Celeste/font/...          (Monocle spritefont 等)
#   ./resources/Celeste/Celeste/audio/...         (bank → ogg)
#   ./resources/Celeste/Celeste/effects/...       (Effects)
#   ./resources/Celeste/Celeste/mountain/...      (Overworld)
#   ./resources/Celeste/Celeste/tutorials/...     (Tutorials)
set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <input-Content-dir> <pack_name> [output-dir]" >&2
    exit 2
fi
in_dir=$1
pack=$2
out_dir=${3:-$(pwd)}

if [[ ! -d "$in_dir" ]]; then
    echo "error: input dir '$in_dir' missing" >&2
    exit 1
fi

# 原版是单一命名空间 Celeste。
ns=Celeste
res_root="$out_dir/resources/$pack/$ns"
mkdir -p "$out_dir/maps/$pack" "$res_root"

# 把原版 Content/ 的子目录原样拷到新布局对应位置。
# Maps → maps/<pack>/  （顶层，不在 resources/）
# Graphics → textures/  Dialog → texts/  Monocle → font/  Effects → effects/
# Overworld → mountain/  Tutorials → tutorials/
copy_into() {
    local src="$1" dst="$2"
    if [[ -d "$src" ]]; then
        echo "==> $(basename "$src") -> $dst"
        mkdir -p "$dst"
        cp -rT "$src" "$dst"
    fi
}

# 1) FMOD banks → OGG + manifest（dev 工具，nix shell 外部拉）
if [[ -d "$in_dir/FMOD/Desktop" ]]; then
    echo "==> audio: FMOD banks -> OGG"
    nix shell nixpkgs#vgmstream nixpkgs#ffmpeg -c \
        "$(dirname "$0")/tools/bank-to-ogg.sh" \
        "$in_dir/FMOD/Desktop" "$res_root/audio"
else
    echo "(no $in_dir/FMOD/Desktop, skipping audio)" >&2
fi

copy_into "$in_dir/Maps"        "$out_dir/maps/$pack"
copy_into "$in_dir/Graphics"    "$res_root/textures"
copy_into "$in_dir/Dialog"      "$res_root/texts"
copy_into "$in_dir/Monocle"     "$res_root/font"
copy_into "$in_dir/Effects"     "$res_root/effects"
copy_into "$in_dir/Overworld"   "$res_root/mountain"
copy_into "$in_dir/Tutorials"   "$res_root/tutorials"

# metadata.json：从拷好的 maps/ 目录生成章节清单（name 由作者补全），
# 与 README §resources 的 schema 一致。
if [[ -d "$out_dir/maps/$pack" ]]; then
    chapters=()
    for bin in "$out_dir/maps/$pack"/*.bin; do
        [[ -f "$bin" ]] || continue
        id=$(basename "$bin" .bin)
        chapters+=("    { \"id\": \"$id\", \"name\": \"\", \"map\": \"maps/$pack/$id.bin\", \"song\": \"\" }")
    done
    if [[ ${#chapters[@]} -gt 0 ]]; then
        {
            echo "{"
            echo "  \"pack\": \"$pack\","
            echo "  \"namespace\": \"$ns\","
            echo "  \"title\": \"$pack\","
            echo "  \"version\": \"1.0.0\","
            echo "  \"description\": \"converted from the original Content/\","
            echo "  \"chapters\": ["
            printf '%s\n' "${chapters[@]}" | paste -sd, -
            echo "  ]"
            echo "}"
        } > "$res_root/metadata.json"
        echo "==> wrote $res_root/metadata.json (${#chapters[@]} chapters)"
    fi
fi

echo "Done. pack '$pack' at $out_dir"
echo "TODO: pack.png + metadata.json 的章节 name/song 由作者自行补全。"

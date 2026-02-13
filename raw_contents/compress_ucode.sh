#!/bin/bash
SRC_DIR="./ucode"
DEST_DIR="../contents/ucode"

mkdir -p "$DEST_DIR"

find "$SRC_DIR" -type f 2>/dev/null | while read -r f; do
    rel_path="${f#$SRC_DIR/}"
    out="$DEST_DIR/$rel_path.z"

    mkdir -p "$(dirname "$out")"

    stat -c%s "$f" | perl -nE 'print pack("V", $_)' > "$out"

    python3 <<EOF >> "$out"
import zlib, sys
with open('$f', 'rb') as f_in:
    compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
    raw_deflate = compressor.compress(f_in.read()) + compressor.flush()
    sys.stdout.buffer.write(raw_deflate)
EOF

    echo "Compressed: $f -> $out"
done

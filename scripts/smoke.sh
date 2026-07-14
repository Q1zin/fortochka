#!/usr/bin/env bash
# Сквозной smoke-тест сервера: healthz → register → upload → pair → wallpaper.
# Использование: scripts/smoke.sh [base_url]   (по умолчанию — прод)
set -euo pipefail
BASE="${1:-https://fortochka.fun}"

jsonget() { python3 -c 'import sys,json;print(json.loads(sys.argv[1])[sys.argv[2]])' "$1" "$2"; }

echo "→ healthz: $(curl -fsS "$BASE/healthz")"

REG=$(curl -fsS -X POST "$BASE/api/v1/cameras/register" \
    -H 'content-type: application/json' -d '{"name":"Смоук-тест"}')
CAM=$(jsonget "$REG" camera_id)
TOK=$(jsonget "$REG" upload_token)
CODE=$(jsonget "$REG" pairing_code)
echo "→ камера зарегистрирована: $CAM (код: $CODE)"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
# крошечный валидный JPEG 8×16, зашит прямо в скрипт
base64 -d > "$TMP/frame.jpg" <<'JPEG'
/9j/4AAQSkZJRgABAQAASABIAAD/4QBMRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAA
A6ABAAMAAAABAAEAAKACAAQAAAABAAAACKADAAQAAAABAAAAEAAAAAD/7QA4UGhvdG9zaG9wIDMu
MAA4QklNBAQAAAAAAAA4QklNBCUAAAAAABDUHYzZjwCyBOmACZjs+EJ+/8AAEQgAEAAIAwEiAAIR
AQMRAf/EAB8AAAEFAQEBAQEBAAAAAAAAAAABAgMEBQYHCAkKC//EALUQAAIBAwMCBAMFBQQEAAAB
fQECAwAEEQUSITFBBhNRYQcicRQygZGhCCNCscEVUtHwJDNicoIJChYXGBkaJSYnKCkqNDU2Nzg5
OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6g4SFhoeIiYqSk5SVlpeYmZqio6Slpqeo
qaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2drh4uPk5ebn6Onq8fLz9PX29/j5+v/EAB8BAAMB
AQEBAQEBAQEAAAAAAAABAgMEBQYHCAkKC//EALURAAIBAgQEAwQHBQQEAAECdwABAgMRBAUhMQYS
QVEHYXETIjKBCBRCkaGxwQkjM1LwFWJy0QoWJDThJfEXGBkaJicoKSo1Njc4OTpDREVGR0hJSlNU
VVZXWFlaY2RlZmdoaWpzdHV2d3h5eoKDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5
usLDxMXGx8jJytLT1NXW19jZ2uLj5OXm5+jp6vLz9PX29/j5+v/bAEMAAgICAgICAwICAwUDAwMF
BgUFBQUGCAYGBgYGCAoICAgICAgKCgoKCgoKCgwMDAwMDA4ODg4ODw8PDw8PDw8PD//bAEMBAgIC
BAQEBwQEBxALCQsQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ
EBAQEP/dAAQAAf/aAAwDAQACEQMRAD8A6DRoB4jl1xtRjsLy71TydaWC8dXS2t/JLlomlnINrJZS
DBRGaOSSTMsY+cw/2V4I/wCgT4U/8C4v/k2ug0GeSMX+t+GdSstN1nVdQt0kmv47aXfcx3NrHLaL
FEhWAzfLDIksqsFXyhGdvy+2b/jX/wA+/hj/AMAbb/CvkMbmknU562ZKjzKLSlOrG6cVsoShGyd4
aRveLu2z53M8NUg4RwuGtT5U43o0bpO8rfvaMpK17aNxunZt3b//2Q==
JPEG

curl -fsS -X POST "$BASE/api/v1/cameras/$CAM/frame" \
    -H "authorization: Bearer $TOK" -H 'content-type: image/jpeg' \
    --data-binary @"$TMP/frame.jpg" > /dev/null
echo "→ кадр загружен"

PAIR=$(curl -fsS -X POST "$BASE/api/v1/pair" \
    -H 'content-type: application/json' -d "{\"pairing_code\":\"$CODE\"}")
VIEW=$(jsonget "$PAIR" view_token)
echo "→ пейринг прошёл: камера «$(jsonget "$PAIR" camera_name)»"

curl -fsS "$BASE/cam/$VIEW/wallpaper.jpg?w=108&h=240" -o "$TMP/wall.jpg"
file "$TMP/wall.jpg" | grep -q JPEG || { echo "✗ обои — не JPEG"; exit 1; }
echo "→ обои получены: $(file -b "$TMP/wall.jpg" | cut -d, -f1,8)"

echo
echo "✅ Полный цикл прошёл на $BASE"
echo "   Код камеры для приложения:  $CODE"
echo "   URL обоев (можно в браузер): $BASE/cam/$VIEW/wallpaper.jpg?w=1080&h=2400"

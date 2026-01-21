#!/bin/bash
#
# gaia-recorder RT mode startup script with automatic G drive backup
#
# Usage:
#   ./scripts/start-recorder.sh [options]
#
# Options:
#   --playback-mode    Start in playback mode instead of RT mode
#   --no-backup        Skip automatic backup to G drive on exit
#
# Example:
#   ./scripts/start-recorder.sh
#   ./scripts/start-recorder.sh --playback-mode
#   ./scripts/start-recorder.sh --no-backup
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CURRENT_DIR_NAME="$(basename "$SCRIPT_DIR")"
RECORDINGS_DIR="recordings"
PLAYBACK_MODE=false
ENABLE_BACKUP=true
RECORDER_PID=""
CLEANUP_DONE=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --playback-mode)
            PLAYBACK_MODE=true
            shift
            ;;
        --no-backup)
            ENABLE_BACKUP=false
            shift
            ;;
        *)
            echo "Usage: $0 [--playback-mode] [--no-backup]"
            exit 1
            ;;
    esac
done

# Cleanup function
cleanup() {
    if [ "$CLEANUP_DONE" = true ]; then
        return
    fi
    CLEANUP_DONE=true

    if [ -n "$RECORDER_PID" ]; then
        kill $RECORDER_PID 2>/dev/null || true
        wait $RECORDER_PID 2>/dev/null || true
    fi

    # Backup database to G drive if enabled
    if [ "$ENABLE_BACKUP" = true ] && [ -d "$RECORDINGS_DIR" ]; then
        G_DRIVE_BASE='G:\共有ドライブ\ArkEdge Users\HirokiHarada\zatsu\ログデータ'

        set +e
        cmd.exe /c "if exist G:\\ (exit /b 0) else (exit /b 1)" >/dev/null 2>&1
        if [ $? -eq 0 ]; then
            DB_FILES=$(find "$RECORDINGS_DIR" -name "*.duckdb" -type f 2>/dev/null)
            for db in $DB_FILES; do
                WINDOWS_SRC=$(wslpath -w "$(realpath "$db")")
                WINDOWS_FILE=$(basename "$db")
                powershell.exe -NoProfile -Command "
                    \$base = '$G_DRIVE_BASE'
                    \$dirName = '$CURRENT_DIR_NAME'
                    \$targetDir = Join-Path \$base \$dirName
                    \$src = '$WINDOWS_SRC'
                    \$dst = Join-Path \$targetDir '$WINDOWS_FILE'

                    if (-not (Test-Path \$targetDir)) {
                        New-Item -Path \$targetDir -ItemType Directory -Force | Out-Null
                    }

                    if (-not (Test-Path \$dst)) {
                        Copy-Item -Path \$src -Destination \$targetDir -ErrorAction SilentlyContinue
                        if (\$?) { exit 0 } else { exit 1 }
                    } else {
                        exit 0
                    }
                " >/dev/null 2>&1
                [ $? -eq 0 ] && echo "Backed up: $CURRENT_DIR_NAME/$WINDOWS_FILE"
            done
        fi
        set -e
    fi
}

trap cleanup EXIT INT TERM

# Build command arguments
RECORDER_ARGS="--data-dir $RECORDINGS_DIR --schema-file satconfig.json"
if [ "$PLAYBACK_MODE" = true ]; then
    RECORDER_ARGS="$RECORDER_ARGS --playback-mode"
fi

# Start gaia-recorder
echo "Starting gaia-recorder with args: $RECORDER_ARGS"
"$SCRIPT_DIR/../boom-tools/bin/gaia-recorder" $RECORDER_ARGS > /tmp/gaia-recorder.log 2>&1 &

RECORDER_PID=$!
echo "gaia-recorder started (PID: $RECORDER_PID)"

wait

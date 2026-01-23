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
        echo "Starting backup to G drive..."
        G_DRIVE_BASE='G:\共有ドライブ\ArkEdge Users\HirokiHarada\zatsu\ログデータ'

        set +e
        cmd.exe /c "if exist G:\\ (exit /b 0) else (exit /b 1)" >/dev/null 2>&1
        if [ $? -eq 0 ]; then
            # Use robocopy for faster bulk copy
            WINDOWS_SRC=$(wslpath -w "$(realpath "$RECORDINGS_DIR")")

            # Run backup with timeout (30 seconds max)
            timeout 30 powershell.exe -NoProfile -WindowStyle Hidden -Command "
                \$base = '$G_DRIVE_BASE'
                \$dirName = '$CURRENT_DIR_NAME'
                \$targetDir = Join-Path \$base \$dirName
                \$src = '$WINDOWS_SRC'

                if (-not (Test-Path \$targetDir)) {
                    New-Item -Path \$targetDir -ItemType Directory -Force | Out-Null
                }

                # Use robocopy for efficient copying (only copy new files)
                # /XO = exclude older files, /R:0 = no retry, /W:0 = no wait between retries
                \$null = robocopy \$src \$targetDir *.duckdb /XO /R:0 /W:0 /NP /NDL /NJH /NJS

                # robocopy returns 0-7 for success (0=no files, 1=files copied, etc)
                if (\$LASTEXITCODE -le 7) {
                    exit 0
                } else {
                    exit 1
                }
            " >/dev/null 2>&1

            BACKUP_STATUS=$?
            if [ $BACKUP_STATUS -eq 0 ] || [ $BACKUP_STATUS -eq 124 ]; then
                # 0 = success, 124 = timeout (partial success)
                [ $BACKUP_STATUS -eq 124 ] && echo "Backup timeout - partial backup completed"
                # [ $BACKUP_STATUS -eq 0 ] && echo "Backup completed: $CURRENT_DIR_NAME/"
            else
                echo "Backup failed or G drive not accessible"
            fi
        else
            echo "G drive not available - skipping backup"
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

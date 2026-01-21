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

# Script directory (base for relative paths)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Configuration
RECORDINGS_DIR="recordings"
PLAYBACK_MODE=false
ENABLE_BACKUP=true
RECORDER_PID=""

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
    echo "$(date +"%Y-%m-%dT%H:%M:%S%z") cleanup: start" >> /tmp/gaia-recorder.log
    # Kill gaia-recorder
    if [ -n "$RECORDER_PID" ]; then
        kill $RECORDER_PID 2>/dev/null || true
        wait $RECORDER_PID 2>/dev/null || true
    fi

    # # Backup database to G drive if enabled
    # if [ "$ENABLE_BACKUP" = true ] && [ -d "$RECORDINGS_DIR" ]; then
    #     G_DRIVE_PATH="/mnt/g/共有ドライブ/ArkEdge Users/HirokiHarada/zatsu/ログデータ"

    #     if [ -d "$G_DRIVE_PATH" ]; then
    #         DB_FILES=$(find "$RECORDINGS_DIR" -name "*.db" -type f 2>/dev/null)
    #         if [ -n "$DB_FILES" ]; then
    #             rsync -av --ignore-existing $DB_FILES "$G_DRIVE_PATH/" 2>&1 | grep -q "recording_" && \
    #                 echo "Database backup completed" || \
    #                 echo "All databases already exist on G drive"
    #         fi
    #     fi
    # fi

    # Backup database to G drive if enabled
    # if [ "$ENABLE_BACKUP" = true ] && [ -d "$RECORDINGS_DIR" ]; then
    #     G_DRIVE_DST="G:\\共有ドライブ\\ArkEdge Users\\HirokiHarada\\zatsu\\ログデータ"

    #     set +e
    #     cmd.exe /c "pushd C:\\ >nul 2>&1 && if exist G:\\ (exit /b 0) else exit /b 2" >nul 2>&1
    #     G_DRIVE_RC=$?
    #     set -e
    #     echo "gdrive rc=$G_DRIVE_RC" >> /tmp/gaia-recorder.log
    #     if [ $G_DRIVE_RC -eq 0 ]; then
    #         DB_FILES=$(find "$RECORDINGS_DIR" -name "*.db" -type f 2>/dev/null)
    #         if [ -n "$DB_FILES" ]; then
    #             for db in $DB_FILES; do
    #                 WINDOWS_DIR=$(wslpath -w "$(dirname "$db")")
    #                 WINDOWS_FILE=$(basename "$db")
    #                 set +e
    #                 cmd.exe /c "pushd \"$WINDOWS_DIR\" && robocopy . \"$G_DRIVE_DST\" \"$WINDOWS_FILE\" /XN /NJH /NJS /NDL /NC /NS & set RC=%ERRORLEVEL% & popd & exit /b %RC%" >nul 2>&1
    #                 RC=$?
    #                 set -e
    #                 echo "robocopy rc=$RC file=$(basename "$db")" >> /tmp/gaia-recorder.log
    #                 [ $RC -le 1 ] && echo "Backed up: $(basename "$db")" || true
    #             done
    #         fi
    #     fi
    # fi

    echo "$(date +"%Y-%m-%dT%H:%M:%S%z") cleanup: end" >> /tmp/gaia-recorder.log
}

# Set trap to cleanup on exit
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
# echo "Log: /tmp/gaia-recorder.log"

# Wait for user interrupt
wait

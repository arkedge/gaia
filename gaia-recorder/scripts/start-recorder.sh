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

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RECORDINGS_DIR="$PROJECT_DIR/recordings"
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
            echo "Error: Unknown option $1"
            echo "Usage: $0 [--playback-mode] [--no-backup]"
            exit 1
            ;;
    esac
done

# Validate required files
TLMCMDDB_FILE="$PROJECT_DIR/example/tlmcmddb.json"
if [ ! -f "$TLMCMDDB_FILE" ]; then
    echo "Error: tlmcmddb.json not found at: $TLMCMDDB_FILE"
    echo "Please ensure example/tlmcmddb.json exists"
    exit 1
fi

# Ensure recordings directory exists
mkdir -p "$RECORDINGS_DIR"

# Cleanup function
cleanup() {
    if [ "$CLEANUP_DONE" = true ]; then
        return
    fi
    CLEANUP_DONE=true

    echo ""
    echo "Shutting down gaia-recorder..."

    if [ -n "$RECORDER_PID" ]; then
        kill $RECORDER_PID 2>/dev/null || true
        wait $RECORDER_PID 2>/dev/null || true
    fi

    # Backup database to G drive if enabled
    if [ "$ENABLE_BACKUP" = true ] && [ -d "$RECORDINGS_DIR" ]; then
        echo "Starting backup to G drive..."

        # Get directory name from project dir
        CURRENT_DIR_NAME="$(basename "$PROJECT_DIR")"
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
                if [ $BACKUP_STATUS -eq 124 ]; then
                    echo "Backup timeout - partial backup completed"
                else
                    echo "Backup completed successfully"
                fi
            else
                echo "Warning: Backup failed or G drive not accessible"
            fi
        else
            echo "G drive not available - skipping backup"
        fi
        set -e
    fi

    echo "Shutdown complete"
}

trap cleanup EXIT INT TERM

# Build command arguments
RECORDER_ARGS="--data-dir $RECORDINGS_DIR --tlmcmddb $TLMCMDDB_FILE"
if [ "$PLAYBACK_MODE" = true ]; then
    RECORDER_ARGS="$RECORDER_ARGS --playback-mode"
    echo "Starting gaia-recorder in PLAYBACK mode"
else
    echo "Starting gaia-recorder in RT mode"
fi

# Check if gaia-recorder binary exists
RECORDER_BIN="$PROJECT_DIR/boom-tools/bin/gaia-recorder"
if [ ! -x "$RECORDER_BIN" ]; then
    echo "Error: gaia-recorder binary not found or not executable: $RECORDER_BIN"
    echo "Please run 'pnpm install' to build the binary"
    exit 1
fi

# Start gaia-recorder
LOG_FILE="/tmp/gaia-recorder.log"
echo "Starting gaia-recorder..."
echo "  Arguments: $RECORDER_ARGS"
echo "  Log file: $LOG_FILE"
echo ""

"$RECORDER_BIN" $RECORDER_ARGS > "$LOG_FILE" 2>&1 &

RECORDER_PID=$!
echo "gaia-recorder started (PID: $RECORDER_PID)"
echo "Press Ctrl+C to stop"
echo ""

# Wait for the process to finish
wait

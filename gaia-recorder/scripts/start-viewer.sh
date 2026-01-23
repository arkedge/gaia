#!/bin/bash
#
# gaia-recorder playback mode viewer startup script
#
# Usage:
#   ./scripts/start-viewer.sh [options] <zip_file>
#
# Options:
#   --reuse-db   Reuse existing database if available (skip import)
#
# Example:
#   ./scripts/start-viewer.sh recording.zip
#   ./scripts/start-viewer.sh --reuse-db recording.zip
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

RECORDINGS_DIR="$PROJECT_DIR/recordings"
TEMP_EXTRACT_DIR="$PROJECT_DIR/.tmp-csv-import"
DB_FILE=""
CLEANUP_ON_EXIT=true
REUSE_DB=false
RECORDER_PID=""

# Cleanup function
cleanup() {
    echo ""
    echo "Shutting down services..."

    if [ -n "$RECORDER_PID" ]; then
        kill $RECORDER_PID 2>/dev/null || true
        wait $RECORDER_PID 2>/dev/null || true
    fi

    if [ "$CLEANUP_ON_EXIT" = true ] && [ -n "$DB_FILE" ] && [ -f "$DB_FILE" ]; then
        echo "Removing temporary database: $DB_FILE"
        rm -f "$DB_FILE"
    fi

    if [ -d "$TEMP_EXTRACT_DIR" ]; then
        echo "Removing temporary files: $TEMP_EXTRACT_DIR"
        rm -rf "$TEMP_EXTRACT_DIR"
    fi

    echo "Shutdown complete"
}

trap cleanup EXIT INT TERM

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --reuse-db)
            REUSE_DB=true
            CLEANUP_ON_EXIT=false
            shift
            ;;
        -*)
            echo "Error: Unknown option $1"
            echo "Usage: $0 [--reuse-db] <zip_file>"
            exit 1
            ;;
        *)
            ZIP_FILE="$1"
            shift
            ;;
    esac
done

if [ -z "${ZIP_FILE:-}" ]; then
    echo "Error: No zip file specified"
    echo "Usage: $0 [--reuse-db] <zip_file>"
    exit 1
fi

if [ ! -f "$ZIP_FILE" ]; then
    echo "Error: Zip file not found: $ZIP_FILE"
    exit 1
fi

# Validate required files
TLMCMDDB_FILE="$PROJECT_DIR/example/tlmcmddb.json"
if [ ! -f "$TLMCMDDB_FILE" ]; then
    echo "Error: tlmcmddb.json not found at: $TLMCMDDB_FILE"
    echo "Please ensure example/tlmcmddb.json exists"
    exit 1
fi

# Check binaries
IMPORT_CSV_BIN="$PROJECT_DIR/target/release/import-csv"
RECORDER_BIN="$PROJECT_DIR/boom-tools/bin/gaia-recorder"

if [ ! -f "$IMPORT_CSV_BIN" ]; then
    echo "Building import-csv binary..."
    (cd "$PROJECT_DIR" && cargo build --release --bin import-csv)
fi

if [ ! -x "$RECORDER_BIN" ]; then
    echo "Error: gaia-recorder binary not found: $RECORDER_BIN"
    echo "Please run 'pnpm install' to install the binary"
    exit 1
fi

# Extract base name from ZIP file (without extension)
ZIP_BASENAME=$(basename "$ZIP_FILE" .zip)
DB_FILE="$RECORDINGS_DIR/${ZIP_BASENAME}.duckdb"

# Extract ZIP
echo "Extracting ZIP file..."
mkdir -p "$TEMP_EXTRACT_DIR"
unzip -q -o "$ZIP_FILE" -d "$TEMP_EXTRACT_DIR"

# Find extracted directory
if [ -d "$TEMP_EXTRACT_DIR/TLM" ] || [ -d "$TEMP_EXTRACT_DIR/CMD" ]; then
    EXTRACTED_DIR="$TEMP_EXTRACT_DIR"
else
    EXTRACTED_DIR=$(find "$TEMP_EXTRACT_DIR" -maxdepth 1 -type d ! -path "$TEMP_EXTRACT_DIR" | head -n 1)
    if [ -z "$EXTRACTED_DIR" ]; then
        echo "Error: No directory found in extracted zip"
        exit 1
    fi
fi

# Verify telemetry data exists
FIRST_CSV=$(find "$EXTRACTED_DIR/TLM" -name "*.csv" -type f 2>/dev/null | head -n 1)
if [ -z "$FIRST_CSV" ]; then
    echo "Error: No telemetry CSV files found in $EXTRACTED_DIR/TLM"
    exit 1
fi

# Check if database already exists
SKIP_IMPORT=false
if [ -f "$DB_FILE" ]; then
    if [ "$REUSE_DB" = true ]; then
        echo "Reusing existing database: $DB_FILE"
        SKIP_IMPORT=true
        CLEANUP_ON_EXIT=false
    else
        echo "Database already exists: $DB_FILE"
        read -p "Reuse existing database? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            CLEANUP_ON_EXIT=false
            SKIP_IMPORT=true
            echo "Reusing existing database"
        else
            echo "Removing existing database"
            rm -f "$DB_FILE"
        fi
    fi
fi

# Import CSV to database
if [ "$SKIP_IMPORT" != true ]; then
    echo "Importing CSV to database..."
    echo "  Input: $EXTRACTED_DIR"
    echo "  Output: $DB_FILE"
    echo ""

    mkdir -p "$RECORDINGS_DIR"
    "$IMPORT_CSV_BIN" \
        --input-dir "$EXTRACTED_DIR" \
        --output-db "$DB_FILE"

    echo ""
    echo "Import completed"
fi

# Start gaia-recorder in playback mode
LOG_FILE="/tmp/gaia-recorder.log"
echo ""
echo "Starting gaia-recorder in playback mode..."
echo "  Database: $DB_FILE"
echo "  Schema: $TLMCMDDB_FILE"
echo "  Log file: $LOG_FILE"
echo ""

"$RECORDER_BIN" \
    --data-dir "$RECORDINGS_DIR" \
    --tlmcmddb "$TLMCMDDB_FILE" \
    --playback-mode \
    > "$LOG_FILE" 2>&1 &

RECORDER_PID=$!

# Wait for gaia-recorder to start
sleep 3
if ! kill -0 $RECORDER_PID 2>/dev/null; then
    echo "Error: gaia-recorder failed to start"
    echo "Log output:"
    cat "$LOG_FILE"
    exit 1
fi

echo "gaia-recorder started successfully (PID: $RECORDER_PID)"
echo ""
echo "Access the viewer at: http://localhost:8920/devtools/"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Wait for the process to finish
wait

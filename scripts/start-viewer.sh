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
#   ./scripts/start-viewer.sh tmtc-c2a/260113-0822-comet-ae-rp-staging.zip
#   ./scripts/start-viewer.sh --reuse-db tmtc-c2a/260113-0822-comet-ae-rp-staging.zip
#

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

RECORDINGS_DIR="$PROJECT_ROOT/gaia-recorder/recordings"
TEMP_EXTRACT_DIR="$PROJECT_ROOT/.tmp-csv-import"
DB_FILE=""
CLEANUP_ON_EXIT=true
REUSE_DB=false
RECORDER_PID=""
TMTC_PID=""

# Cleanup function
cleanup() {
    if [ -n "$RECORDER_PID" ]; then
        kill $RECORDER_PID 2>/dev/null || true
        wait $RECORDER_PID 2>/dev/null || true
    fi
    if [ -n "$TMTC_PID" ]; then
        kill $TMTC_PID 2>/dev/null || true
        wait $TMTC_PID 2>/dev/null || true
    fi
    if [ "$CLEANUP_ON_EXIT" = true ] && [ -n "$DB_FILE" ] && [ -f "$DB_FILE" ]; then
        rm -f "$DB_FILE"
    fi
    if [ -d "$TEMP_EXTRACT_DIR" ]; then
        rm -rf "$TEMP_EXTRACT_DIR"
    fi
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

if [ -z "$ZIP_FILE" ]; then
    echo "Error: No zip file specified"
    echo "Usage: $0 [--reuse-db] <zip_file>"
    exit 1
fi

if [ ! -f "$ZIP_FILE" ]; then
    echo "Error: Zip file not found: $ZIP_FILE"
    exit 1
fi

# Extract ZIP
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

# Extract session ID from CSV timestamp
FIRST_CSV=$(find "$EXTRACTED_DIR/TLM" -name "*.csv" -type f 2>/dev/null | head -n 1)
if [ -z "$FIRST_CSV" ]; then
    echo "Error: No telemetry CSV files found"
    exit 1
fi

FIRST_TIMESTAMP=$(sed -n '2p' "$FIRST_CSV" | cut -d',' -f1)
if [ -z "$FIRST_TIMESTAMP" ]; then
    echo "Error: Could not extract timestamp from CSV"
    exit 1
fi

TIMESTAMP_DATE=$(echo "$FIRST_TIMESTAMP" | cut -d' ' -f1)
TIMESTAMP_TIME=$(echo "$FIRST_TIMESTAMP" | cut -d' ' -f2)
DATE_PART=$(echo "$TIMESTAMP_DATE" | sed 's/-//g')
TIME_PART=$(echo "$TIMESTAMP_TIME" | cut -d':' -f1-2 | sed 's/://g')
SESSION_ID="${DATE_PART}_${TIME_PART}"

DB_FILE="$RECORDINGS_DIR/recording_${SESSION_ID}.db"

# Check if database already exists
SKIP_IMPORT=false
if [ -f "$DB_FILE" ]; then
    if [ "$REUSE_DB" = true ]; then
        echo "Reusing existing database: $DB_FILE"
        SKIP_IMPORT=true
    else
        read -p "Database already exists. Reuse? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            CLEANUP_ON_EXIT=false
            SKIP_IMPORT=true
        else
            rm -f "$DB_FILE"
        fi
    fi
fi

# Import CSV to database
if [ "$SKIP_IMPORT" != true ]; then
    mkdir -p "$RECORDINGS_DIR"
    cargo run --package gaia-recorder --bin import-csv -- \
        --input-dir "$EXTRACTED_DIR" \
        --output-db "$DB_FILE"
fi

# Start gaia-recorder
cargo run --package gaia-recorder --bin gaia-recorder -- \
    --playback-mode \
    --data-dir "$RECORDINGS_DIR" \
    --schema-file tmtc-c2a/satconfig.json \
    > /tmp/gaia-recorder.log 2>&1 &
RECORDER_PID=$!

sleep 3
if ! kill -0 $RECORDER_PID 2>/dev/null; then
    echo "Error: gaia-recorder failed to start"
    cat /tmp/gaia-recorder.log
    exit 1
fi

# Start tmtc-c2a
cargo run --package tmtc-c2a -- \
    --satconfig tmtc-c2a/satconfig.json \
    --tlmcmddb tmtc-c2a/tlmcmddb.json \
    --recorder-endpoint http://127.0.0.1:8920 \
    > /tmp/tmtc-c2a.log 2>&1 &
TMTC_PID=$!

sleep 3
if ! kill -0 $TMTC_PID 2>/dev/null; then
    echo "Error: tmtc-c2a failed to start"
    cat /tmp/tmtc-c2a.log
    exit 1
fi

echo "Services started"
echo "gaia-recorder: http://localhost:8920 (PID: $RECORDER_PID)"
echo "tmtc-c2a: http://localhost:8900 (PID: $TMTC_PID)"
echo "Press Ctrl+C to stop"

wait

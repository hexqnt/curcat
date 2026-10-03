#!/usr/bin/env bash

# Profile an interactive desktop workflow or a reproducible headless UI workload.
# Compilation finishes before measurement; perf modes preserve symbols and frame pointers.

set -Eeuo pipefail

readonly PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly PACKAGE="curcat"

usage() {
    cat <<'EOF'
Usage: ./profiling.sh [record|stat|all] [--] [IMAGE]
       ./profiling.sh [ui|ui-record|ui-stat|ui-all]

Desktop modes: exercise a representative workflow, then close the window. all runs record and stat sequentially; repeat the same workflow in both runs.

ui measures a reproducible headless egui_kittest zoom workload and writes frame timings to JSON. ui-record, ui-stat and ui-all also collect Linux perf data. The workload measures CPU UI work without GPU rendering or a native window.

Environment:
  CURCAT_PROFILE_FRAMES  Measured frames, at least 120 (default: 2400).
  CURCAT_PROFILE_POINTS Picked points (default: 1000).
  CURCAT_PROFILE_OUTPUT JSON destination (default: PROFILE_DIR/PROFILE_NAME.json).
  PROFILE_DIR           Result directory (default: out/profiles).
  PROFILE_NAME          Result basename (default: curcat-<mode>).
  PROFILE_FREQ          perf record sampling frequency (default: 499 Hz).
  PROFILE_EVENT         perf record event (default: cycles:u).
  PROFILE_REPEATS       perf stat repetitions (default: 1).
  CARGO_TARGET_DIR      Cargo build directory (default: target).
EOF
}

MODE="record"
if (($# > 0)); then
    case "$1" in
        record | stat | all | ui | ui-record | ui-stat | ui-all)
            MODE="$1"
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
    esac
fi
readonly MODE

if (($# > 0)) && [[ "$1" == "--" ]]; then
    shift
fi

UI_MODE=false
OPERATION="$MODE"
if [[ "$MODE" == ui* ]]; then
    UI_MODE=true
    OPERATION="${MODE#ui-}"
    if [[ "$MODE" == ui ]]; then
        OPERATION="timing"
    fi
    if (($# > 0)); then
        printf 'error: headless UI profiling does not accept image arguments\n' >&2
        exit 2
    fi
fi
readonly UI_MODE OPERATION

readonly PROFILE_DIR="${PROFILE_DIR:-$PROJECT_DIR/out/profiles}"
readonly PROFILE_NAME="${PROFILE_NAME:-curcat-$MODE}"
readonly PROFILE_FREQ="${PROFILE_FREQ:-499}"
readonly PROFILE_EVENT="${PROFILE_EVENT:-cycles:u}"
readonly PROFILE_REPEATS="${PROFILE_REPEATS:-1}"
TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_DIR/target}"
if [[ "$TARGET_DIR" != /* ]]; then
    TARGET_DIR="$PROJECT_DIR/$TARGET_DIR"
fi
readonly TARGET_DIR

if "$UI_MODE"; then
    BINARY="$TARGET_DIR/profiling/examples/ui"
    BUILD_TARGET=(--package curcat-profiling --example ui)
else
    BINARY="$TARGET_DIR/profiling/$PACKAGE"
    BUILD_TARGET=(--package "$PACKAGE" --bin "$PACKAGE")
fi
readonly BINARY
readonly -a BUILD_TARGET
readonly -a PROFILE_COMMAND=("$BINARY" "$@")

# Software counters show scheduling overhead; hardware counters describe CPU efficiency.
readonly PERF_EVENTS="task-clock,context-switches,cpu-migrations,page-faults,cycles,instructions,branches,branch-misses,cache-references,cache-misses"

if [[ "$OPERATION" == stat || "$OPERATION" == all ]] && [[ ! "$PROFILE_REPEATS" =~ ^[1-9][0-9]*$ ]]; then
    printf 'error: PROFILE_REPEATS must be a positive integer\n' >&2
    exit 2
fi

if [[ "$OPERATION" == record || "$OPERATION" == all ]] && [[ ! "$PROFILE_FREQ" =~ ^[1-9][0-9]*$ ]]; then
    printf 'error: PROFILE_FREQ must be a positive integer\n' >&2
    exit 2
fi

if [[ ! "$PROFILE_NAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
    printf 'error: PROFILE_NAME contains unsupported characters\n' >&2
    exit 2
fi

if [[ "$OPERATION" != timing ]] && ! command -v perf >/dev/null 2>&1; then
    printf 'error: perf is not installed or not available in PATH\n' >&2
    exit 127
fi

rotate_output() {
    local output="$1"

    if [[ -e "$output" ]]; then
        mv -f -- "$output" "$output.old"
    fi
}

record_profile() {
    local data="$PROFILE_DIR/$PROFILE_NAME.perf.data"
    local report="$PROFILE_DIR/$PROFILE_NAME.perf-report.txt"

    rotate_output "$data"
    rotate_output "$report"

    if "$UI_MODE"; then
        printf 'Recording call stacks for the headless UI workload.\n'
    else
        printf 'Recording call stacks; close the Curcat window when the scenario is complete.\n'
    fi
    perf record \
        --freq "$PROFILE_FREQ" \
        --event "$PROFILE_EVENT" \
        --call-graph fp \
        --output "$data" \
        -- "${PROFILE_COMMAND[@]}"

    perf report \
        --stdio \
        --no-children \
        --call-graph none \
        --input "$data" \
        --sort dso,symbol \
        >"$report"

    printf 'perf data:   %s\n' "$data"
    printf 'perf report: %s\n' "$report"
}

collect_stats() {
    local output="$PROFILE_DIR/$PROFILE_NAME.perf-stat.txt"

    rotate_output "$output"
    if "$UI_MODE"; then
        printf 'Collecting counters for the headless UI workload.\n'
    else
        printf 'Collecting counters; close the Curcat window when the scenario is complete.\n'
    fi
    if ! "$UI_MODE" && ((PROFILE_REPEATS > 1)); then
        printf 'The application will be launched %s times; repeat the same scenario each time.\n' \
            "$PROFILE_REPEATS"
    fi

    perf stat \
        --repeat "$PROFILE_REPEATS" \
        --event "$PERF_EVENTS" \
        --output "$output" \
        -- "${PROFILE_COMMAND[@]}"

    printf 'perf stat:   %s\n' "$output"
}

cd -- "$PROJECT_DIR"
mkdir -p -- "$PROFILE_DIR"

# Keep compilation outside measurements and retain release optimizations and frame pointers.
readonly PROFILING_RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C force-frame-pointers=yes"
if "$UI_MODE"; then
    export CURCAT_PROFILE_OUTPUT="${CURCAT_PROFILE_OUTPUT:-$PROFILE_DIR/$PROFILE_NAME.json}"
fi
RUSTFLAGS="$PROFILING_RUSTFLAGS" cargo build \
    --quiet \
    --locked \
    --profile profiling \
    --target-dir "$TARGET_DIR" \
    "${BUILD_TARGET[@]}"

case "$OPERATION" in
    timing)
        "${PROFILE_COMMAND[@]}"
        ;;
    record)
        record_profile
        ;;
    stat)
        collect_stats
        ;;
    all)
        record_profile
        collect_stats
        ;;
esac

if "$UI_MODE"; then
    printf 'frame timings: %s\n' "$CURCAT_PROFILE_OUTPUT"
fi

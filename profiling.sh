#!/usr/bin/env bash

# Профилирование интерактивного запуска Curcat с помощью Linux perf.
#
# Основной режим записывает сэмплы стеков. После запуска нужно воспроизвести
# интересующий пользовательский сценарий и штатно закрыть окно приложения:
#   ./profiling.sh record path/to/image.png
#
# Аппаратные счётчики собираются отдельным запуском:
#   ./profiling.sh stat path/to/image.png
#
# Режим all последовательно запускает оба измерения, поэтому сценарий придётся
# воспроизвести дважды:
#   ./profiling.sh all path/to/image.png
#
# Путь к изображению необязателен. Аргументы после `--` без изменений передаются
# приложению:
#   ./profiling.sh record -- path/to/image.png
#
# Настройки можно переопределить переменными окружения:
#   PROFILE_DIR     — каталог результатов, по умолчанию out/profiles;
#   PROFILE_NAME    — базовое имя файлов, по умолчанию curcat-<режим>;
#   PROFILE_FREQ    — частота сэмплирования perf record, по умолчанию 499 Гц;
#   PROFILE_EVENT   — событие perf record, по умолчанию cycles:u;
#   PROFILE_REPEATS — число интерактивных запусков perf stat, по умолчанию 1.

set -Eeuo pipefail

readonly PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly PACKAGE="curcat"

usage() {
    cat <<'EOF'
Usage: ./profiling.sh [record|stat|all] [--] [IMAGE]

Run Curcat under Linux perf. Exercise one representative workflow in the
opened application, then close its window to finish the measurement.
EOF
}

MODE="record"
if (($# > 0)); then
    case "$1" in
        record | stat | all)
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

readonly PROFILE_DIR="${PROFILE_DIR:-$PROJECT_DIR/out/profiles}"
readonly PROFILE_NAME="${PROFILE_NAME:-curcat-$MODE}"
readonly PROFILE_FREQ="${PROFILE_FREQ:-499}"
readonly PROFILE_EVENT="${PROFILE_EVENT:-cycles:u}"
readonly PROFILE_REPEATS="${PROFILE_REPEATS:-1}"
readonly BINARY="$PROJECT_DIR/target/profiling/$PACKAGE"
readonly -a PROFILE_COMMAND=("$BINARY" "$@")

# task-clock показывает суммарное процессорное время. Программные счётчики
# помогают увидеть накладные расходы планировщика и памяти, аппаратные —
# эффективность инструкций, ветвлений и кэша процессора.
readonly PERF_EVENTS="task-clock,context-switches,cpu-migrations,page-faults,cycles,instructions,branches,branch-misses,cache-references,cache-misses"

if [[ ! "$PROFILE_REPEATS" =~ ^[1-9][0-9]*$ ]]; then
    printf 'error: PROFILE_REPEATS must be a positive integer\n' >&2
    exit 2
fi

if [[ ! "$PROFILE_FREQ" =~ ^[1-9][0-9]*$ ]]; then
    printf 'error: PROFILE_FREQ must be a positive integer\n' >&2
    exit 2
fi

if [[ ! "$PROFILE_NAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
    printf 'error: PROFILE_NAME contains unsupported characters\n' >&2
    exit 2
fi

if ! command -v perf >/dev/null 2>&1; then
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

    printf 'Recording call stacks; close the Curcat window when the scenario is complete.\n'
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
    printf 'Collecting counters; close the Curcat window when the scenario is complete.\n'
    if ((PROFILE_REPEATS > 1)); then
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

# Сборка не входит в измерения. Профиль profiling сохраняет release-оптимизации
# и символы, а frame pointers включаются для всего графа зависимостей.
readonly PROFILING_RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C force-frame-pointers=yes"
RUSTFLAGS="$PROFILING_RUSTFLAGS" cargo build \
    --quiet \
    --locked \
    --profile profiling \
    --package "$PACKAGE"

case "$MODE" in
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

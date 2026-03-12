#!/usr/bin/env bash
# load_test.sh — Simulate concurrent users browsing posts and pages.
#
# Usage:
#   ./scripts/load_test.sh -domain beth.com -port 3000
#   ./scripts/load_test.sh -domain beth.com -port 3000 -users 50 -duration 120
#
# Reads DATABASE_URL from .env in the project root if not already set.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# ── Load .env ──────────────────────────────────────────────────────────────────
if [[ -z "${DATABASE_URL:-}" && -f "$PROJECT_ROOT/.env" ]]; then
    export DATABASE_URL
    DATABASE_URL=$(grep -E '^DATABASE_URL=' "$PROJECT_ROOT/.env" | cut -d= -f2-)
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set and could not be read from .env" >&2
    exit 1
fi

# ── Argument parsing ───────────────────────────────────────────────────────────
DOMAIN=""
PORT=""
USERS=100
DURATION_MS=60000
INTERVAL=5
URL_SAMPLE=100

usage() {
    cat <<EOF
Usage: $(basename "$0") -domain <hostname> -port <port> [options]

Simulate concurrent users randomly browsing posts and pages for a site.
Picks a random sample of URLs from the database, then runs N virtual users
each hitting a random URL every INTERVAL seconds. Monitors memory and prints
a performance report when done.

Required:
  -domain <hostname>    Site hostname (e.g. beth.com)
  -port   <port>        Port the app is listening on (e.g. 3000)

Options:
  -users    <n>         Number of concurrent virtual users (default: 100)
  -time     <ms>        How long to run in milliseconds (default: 60000)
  -interval <s>         Seconds between each user's requests (default: 5)
  -sample   <n>         Number of URLs to pull from the DB (default: 100)

Examples:
  $(basename "$0") -domain beth.com -port 3000
  $(basename "$0") -domain beth.com -port 3000 -time 120000
  $(basename "$0") -domain beth.com -port 3000 -users 50 -time 30000
  $(basename "$0") -domain beth.com -port 3000 -users 100 -time 60000 -interval 3
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help) usage ;;
        -domain)   DOMAIN="$2";       shift 2 ;;
        -port)     PORT="$2";         shift 2 ;;
        -users)    USERS="$2";        shift 2 ;;
        -time)     DURATION_MS="$2";  shift 2 ;;
        -interval) INTERVAL="$2";     shift 2 ;;
        -sample)   URL_SAMPLE="$2";   shift 2 ;;
        *) usage ;;
    esac
done

# Convert ms to seconds (ceiling)
DURATION=$(( (DURATION_MS + 999) / 1000 ))

[[ -z "$DOMAIN" || -z "$PORT" ]] && usage

# ── Resolve site ───────────────────────────────────────────────────────────────
psql_q() { command psql "$DATABASE_URL" --tuples-only --no-align "$@"; }

SITE_ID=$(psql_q -c "SELECT id FROM sites WHERE hostname = '$DOMAIN' LIMIT 1;" | tr -d '[:space:]')
if [[ -z "$SITE_ID" ]]; then
    echo "ERROR: No site found with hostname '$DOMAIN'" >&2
    exit 1
fi

# ── Build URL list from DB ─────────────────────────────────────────────────────
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

URL_FILE="$WORK_DIR/urls.txt"

psql_q -c "
    SELECT 'http://${DOMAIN}:${PORT}/' || slug
    FROM posts
    WHERE site_id = '$SITE_ID'
      AND status   = 'published'
      AND post_type IN ('post', 'page')
    ORDER BY RANDOM()
    LIMIT $URL_SAMPLE;
" > "$URL_FILE"

URL_COUNT=$(wc -l < "$URL_FILE" | tr -d '[:space:]')
if [[ "$URL_COUNT" -eq 0 ]]; then
    echo "ERROR: No published posts or pages found for '$DOMAIN'." >&2
    exit 1
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Synaptic Signals — Load Test"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
printf "  Site     : %s\n"    "$DOMAIN:$PORT"
printf "  URLs     : %s\n"    "$URL_COUNT (sampled from DB)"
printf "  Users    : %s\n"    "$USERS"
printf "  Duration : %sms (%ss)\n" "$DURATION_MS" "$DURATION"
printf "  Interval : %ss between each user request\n" "$INTERVAL"
printf "  Est. RPS : ~%.1f\n" "$(awk -v u="$USERS" -v i="$INTERVAL" 'BEGIN { printf "%.1f", u/i }')"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ── Find the synaptic process ──────────────────────────────────────────────────
SYNAPTIC_PID=$(pgrep -f 'synaptic-signals/target' 2>/dev/null | head -1 || true)
if [[ -z "$SYNAPTIC_PID" ]]; then
    echo "WARNING: synaptic process not found — memory monitoring disabled." >&2
fi

# ── Memory monitor (runs in background) ───────────────────────────────────────
MEM_LOG="$WORK_DIR/memory.log"
mem_monitor() {
    local pid=$1 log=$2
    while true; do
        if [[ -d /proc/$pid ]]; then
            local rss
            rss=$(awk '/^VmRSS/{print $2}' /proc/$pid/status 2>/dev/null || echo "0")
            echo "$(date +%s) $rss" >> "$log"
        fi
        sleep 2
    done
}

if [[ -n "$SYNAPTIC_PID" ]]; then
    mem_monitor "$SYNAPTIC_PID" "$MEM_LOG" &
    MEM_PID=$!
else
    MEM_PID=""
fi

# ── Virtual user (runs N times in background) ─────────────────────────────────
RESULTS_DIR="$WORK_DIR/results"
mkdir -p "$RESULTS_DIR"

virtual_user() {
    local uid=$1 url_file=$2 interval=$3 out=$4
    # Stagger startup so all 100 don't hit at exactly t=0
    sleep "0.$(( RANDOM % 100 ))"
    while true; do
        local url
        url=$(shuf -n 1 "$url_file")
        local result
        result=$(curl -o /dev/null -s -w "%{http_code} %{time_total}" \
                      --max-time 10 \
                      --connect-timeout 5 \
                      -H "Host: ${DOMAIN}" \
                      "$url" 2>/dev/null || echo "000 10.000")
        echo "$result" >> "$out"
        sleep "$interval"
    done
}

echo "Starting $USERS virtual users..."
USER_PIDS=()
for ((u = 1; u <= USERS; u++)); do
    virtual_user "$u" "$URL_FILE" "$INTERVAL" "$RESULTS_DIR/user_${u}.log" &
    USER_PIDS+=($!)
done

# ── Progress bar while waiting ─────────────────────────────────────────────────
echo ""
START_TIME=$(date +%s)
END_TIME=$(( START_TIME + DURATION ))

while true; do
    NOW=$(date +%s)
    ELAPSED=$(( NOW - START_TIME ))
    [[ "$ELAPSED" -ge "$DURATION" ]] && break

    REMAINING=$(( DURATION - ELAPSED ))
    DONE_REQ=$(cat "$RESULTS_DIR"/user_*.log 2>/dev/null | wc -l | tr -d '[:space:]')

    APP_MEM=""
    if [[ -n "$SYNAPTIC_PID" && -d /proc/$SYNAPTIC_PID ]]; then
        APP_MEM=$(awk '/^VmRSS/{printf "| app mem %d MB ", $2/1024}' \
                  /proc/$SYNAPTIC_PID/status 2>/dev/null || true)
    fi

    SYS_MEM=$(awk '
        /^MemTotal/    { total=$2 }
        /^MemAvailable/{ avail=$2 }
        END { printf "| system %d MB ", (total-avail)/1024 }
    ' /proc/meminfo 2>/dev/null || true)

    # CPU% of the synaptic process via /proc/<pid>/stat
    CPU_PCT=""
    if [[ -n "$SYNAPTIC_PID" && -d /proc/$SYNAPTIC_PID ]]; then
        # Read utime+stime now, sleep briefly, read again — delta / elapsed = %
        STAT1=$(awk '{print $14+$15}' /proc/$SYNAPTIC_PID/stat 2>/dev/null || echo 0)
        sleep 0.5
        STAT2=$(awk '{print $14+$15}' /proc/$SYNAPTIC_PID/stat 2>/dev/null || echo 0)
        HZ=$(getconf CLK_TCK 2>/dev/null || echo 100)
        CPU_PCT=$(awk -v s1="$STAT1" -v s2="$STAT2" -v hz="$HZ" \
                  'BEGIN { printf "| cpu %.1f%% ", (s2-s1)/hz/0.5*100 }')
    fi

    PCT=$(( ELAPSED * 40 / DURATION ))
    BAR=$(printf '%0.s█' $(seq 1 $PCT))$(printf '%0.s░' $(seq 1 $(( 40 - PCT ))))
    printf "\r  [%s] %ds left | %s reqs %s%s%s " \
           "$BAR" "$REMAINING" "$DONE_REQ" "$APP_MEM" "$SYS_MEM" "$CPU_PCT"
    # 0.5s already spent inside the CPU sample above — add 0.5 to hit ~1s cadence
    sleep 0.5
done

printf "\r%-70s\r" " "   # clear the progress line

# ── Stop everything ────────────────────────────────────────────────────────────
echo "Stopping virtual users..."
for PID in "${USER_PIDS[@]}"; do
    kill "$PID" 2>/dev/null || true
done
wait "${USER_PIDS[@]}" 2>/dev/null || true

if [[ -n "$MEM_PID" ]]; then
    kill "$MEM_PID" 2>/dev/null || true
    wait "$MEM_PID" 2>/dev/null || true
fi

# ── Crunch the numbers ─────────────────────────────────────────────────────────
COMBINED="$WORK_DIR/combined.log"
cat "$RESULTS_DIR"/user_*.log 2>/dev/null > "$COMBINED" || true

TOTAL_REQS=$(wc -l < "$COMBINED" | tr -d '[:space:]')

if [[ "$TOTAL_REQS" -eq 0 ]]; then
    echo "No requests completed — check that the app is running on $DOMAIN:$PORT."
    exit 1
fi

# awk does all the stats in one pass
awk '
BEGIN {
    ok=0; err=0
    min=9999; max=0; sum=0
    n=0
}
{
    code=$1; t=$2
    if (code >= 200 && code < 400) ok++
    else err++

    if (t < min) min=t
    if (t > max) max=t
    sum += t
    times[n++] = t
}
END {
    avg = (n > 0) ? sum/n : 0

    # sort for percentiles (bubble — good enough for <=10k rows)
    for (i=0; i<n-1; i++)
        for (j=i+1; j<n; j++)
            if (times[i] > times[j]) { tmp=times[i]; times[i]=times[j]; times[j]=tmp }

    p50 = times[int(n * 0.50)]
    p95 = times[int(n * 0.95)]
    p99 = times[int(n * 0.99)]

    printf "TOTAL=%d OK=%d ERR=%d MIN=%.3f MAX=%.3f AVG=%.3f P50=%.3f P95=%.3f P99=%.3f\n",
           n, ok, err, min, max, avg, p50, p95, p99
}
' "$COMBINED" > "$WORK_DIR/stats.txt"

read -r STATS < "$WORK_DIR/stats.txt"
eval "$STATS"   # sets TOTAL OK ERR MIN MAX AVG P50 P95 P99

# Memory stats
MEM_START="" MEM_END="" MEM_PEAK=""
if [[ -f "$MEM_LOG" && -s "$MEM_LOG" ]]; then
    read MEM_START _ < "$MEM_LOG"
    MEM_START=$(awk 'NR==1{print $2}' "$MEM_LOG")
    MEM_END=$(awk 'END{print $2}' "$MEM_LOG")
    MEM_PEAK=$(awk 'BEGIN{m=0} {if($2>m) m=$2} END{print m}' "$MEM_LOG")
    MEM_DELTA=$(( MEM_END - MEM_START ))
    MEM_SIGN="+"
    [[ "$MEM_DELTA" -lt 0 ]] && MEM_SIGN=""
fi

ACTUAL_RPS=$(awk -v t="$TOTAL" -v d="$DURATION" 'BEGIN { printf "%.2f", (d > 0 ? t/d : 0) }')

# ── Report ─────────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  RESULTS"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
printf "  Duration        : %sms (%ss)\n" "$DURATION_MS" "$DURATION"
printf "  Virtual users   : %s\n"     "$USERS"
printf "  URL pool        : %s\n"     "$URL_COUNT"
echo   "  ─────────────────────────────────────────────────"
printf "  Total requests  : %s\n"     "$TOTAL"
printf "  Successful 2xx  : %s\n"     "$OK"
printf "  Errors          : %s\n"     "$ERR"
printf "  Actual RPS      : %s\n"     "$ACTUAL_RPS"
echo   "  ─────────────────────────────────────────────────"
echo   "  Response times (seconds)"
printf "    Min           : %s\n"     "$MIN"
printf "    Avg           : %s\n"     "$AVG"
printf "    p50 (median)  : %s\n"     "$P50"
printf "    p95           : %s\n"     "$P95"
printf "    p99           : %s\n"     "$P99"
printf "    Max           : %s\n"     "$MAX"
if [[ -n "$MEM_START" ]]; then
    echo   "  ─────────────────────────────────────────────────"
    echo   "  Memory — synaptic process (RSS)"
    printf "    Start         : %s MB\n" "$(( MEM_START / 1024 ))"
    printf "    End           : %s MB\n" "$(( MEM_END   / 1024 ))"
    printf "    Peak          : %s MB\n" "$(( MEM_PEAK  / 1024 ))"
    printf "    Delta         : %s%s MB\n" "$MEM_SIGN" "$(( MEM_DELTA / 1024 ))"
fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [[ "$ERR" -gt 0 ]]; then
    echo ""
    echo "  Error breakdown (status codes):"
    awk '$1 < 200 || $1 >= 400 {print $1}' "$COMBINED" | sort | uniq -c | sort -rn | \
        awk '{printf "    %s × %s\n", $1, $2}'
fi

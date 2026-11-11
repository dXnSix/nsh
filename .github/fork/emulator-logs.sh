#!/usr/bin/env bash
# What the `android` job's emulator leaves behind when its tests fail, kept
# as the run's artifact. An emulator that goes away during a test ends
# without a line in the job's log, and nothing can be asked of it after.
# Usage: emulator-logs.sh start <dir> before the tests (a line of the
# emulator step's script), emulator-logs.sh collect <dir> after they failed.
#
# - `logcat.txt`: the device's log as it comes, to its last line (the app's
#   frame times, what the system killed or waited for).
# - `host.txt`: every 5 s the runner's load and memory and the emulator's
#   process (its memory, CPU time and threads), which dates a stall and the
#   emulator's end, and says whether memory ran out before it.
# - `dmesg.txt`: the runner's kernel log (a process killed for memory, a
#   crash).
# - `crashes/`: the emulator's crash reports (`emu-crash-*.db`), with what
#   else it left in its temporary directory listed in `tmp.txt`.
set -uo pipefail
mode="$1"
dir="$2"
mkdir -p "$dir"
adb="${ANDROID_HOME:+$ANDROID_HOME/platform-tools/}adb"
case "$mode" in
  start)
    # Both in the background, on none of the step's streams: the script's
    # line ends at once, and the tests' output stays the step's.
    nohup "$adb" logcat -v threadtime > "$dir/logcat.txt" 2>&1 < /dev/null &
    nohup bash -c '
      while sleep 5; do
        echo "$(date -u +%H:%M:%S) load $(cut -d" " -f1-3 /proc/loadavg)"
        free -m | sed -n 2,3p
        pid=$(pgrep qemu-system | head -1)
        if [[ -n "$pid" ]]; then
          ps -o pid=,rss=,cputime=,nlwp=,etime=,comm= -p "$pid"
        else
          echo "no emulator process"
        fi
      done' > "$dir/host.txt" 2>&1 < /dev/null &
    ;;
  collect)
    sudo -n dmesg -T > "$dir/dmesg.txt" 2>&1 || true
    df -h > "$dir/disk.txt" 2>&1 || true
    ls -laR /tmp/android-* > "$dir/tmp.txt" 2>&1 || true
    mkdir -p "$dir/crashes"
    cp -R /tmp/android-*/emu-crash-*.db "$dir/crashes/" 2> /dev/null || true
    ;;
  *)
    echo "emulator-logs.sh: start or collect, not $mode" >&2
    exit 2
    ;;
esac

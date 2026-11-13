#!/usr/bin/env bash
# Runs a test command, and once more when it fails, before the step fails.
#
#   bash .github/fork/retry.sh <command> [arguments...]
#
# Some unit tests fail now and then on a loaded runner (a wait that runs
# out, a frame that is not drawn yet): the TUI's dock previews, GPUI's test
# windows, the Windows sound decoder. A second run costs seconds; without
# it the job fails and is rerun by hand, minutes later. A command that
# passes the second time is reported as flaky, a warning on the run, so it
# stays visible; one that fails twice fails the step. `examples.sh` does the
# same for the examples' tests.
"$@" && exit 0
echo "::group::Failed: running it once more"
"$@"
status=$?
echo "::endgroup::"
if ((status == 0)); then
  echo "::warning title=Flaky test step::\`$*\` failed, then passed when run again"
fi
exit "$status"

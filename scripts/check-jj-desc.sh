#!/usr/bin/env bash
# Prefix checker script for Jujutsu change descriptions
# Usage: scripts/check-jj-desc.sh --slug <slug> --milestone <milestone> --task <task>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SLUG=""
MILESTONE=""
TASK=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --slug)
      if [[ $# -lt 2 ]]; then
        echo "Error: --slug requires an argument." >&2
        exit 1
      fi
      SLUG="$2"
      shift 2
      ;;
    --milestone)
      if [[ $# -lt 2 ]]; then
        echo "Error: --milestone requires an argument." >&2
        exit 1
      fi
      MILESTONE="$2"
      shift 2
      ;;
    --task)
      if [[ $# -lt 2 ]]; then
        echo "Error: --task requires an argument." >&2
        exit 1
      fi
      TASK="$2"
      shift 2
      ;;
    *)
      echo "Error: Unknown argument: $1" >&2
      echo "Usage: $0 --slug <slug> --milestone <milestone> --task <task>" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$SLUG" || -z "$MILESTONE" || -z "$TASK" ]]; then
  echo "Error: Missing required arguments." >&2
  echo "Usage: $0 --slug <slug> --milestone <milestone> --task <task>" >&2
  exit 1
fi

if [[ ! "$MILESTONE" =~ ^[0-9]+$ ]]; then
  echo "Error: --milestone must be an integer." >&2
  exit 1
fi

if [[ ! "$TASK" =~ ^[0-9]+$ ]]; then
  echo "Error: --task must be an integer." >&2
  exit 1
fi

# Fetch active change description using Jujutsu
# Handle case where jj command might fail (e.g. not in jj repo)
if ! DESC=$(jj --no-pager log -r @ -T "description" --no-graph 2>/dev/null); then
  echo "Error: Failed to run jj command. Are you in a Jujutsu repository? (Script resolved directory: $SCRIPT_DIR)" >&2
  exit 1
fi

PREFIX="${SLUG}-M${MILESTONE}-T${TASK}:"

if [[ "$DESC" != "$PREFIX"* ]]; then
  echo "Error: Jujutsu description does not start with expected prefix: '$PREFIX'" >&2
  echo "Current description is:" >&2
  echo "--------------------------------------------------" >&2
  echo "$DESC" >&2
  echo "--------------------------------------------------" >&2
  exit 1
fi

echo "Success: Description matches prefix '$PREFIX'."
exit 0

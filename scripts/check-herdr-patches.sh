#!/usr/bin/env bash
# Run nix/herdr-patches.nix's postPatch body against a checkout of herdr,
# without building anything.
#
# The patch set's whole safety story is that every anchor is a --replace-fail
# on a line that exists exactly once: a herdr bump that moves one FAILS the
# build. That guarantee is only worth what it costs to find out, and finding
# out through `nixos-rebuild` means waiting for a rust compile to reach the
# postPatch it would have failed in seconds. This does the postPatch alone, on
# a throwaway copy, in about a second.
#
# It does not typecheck the result -- a passing run says every anchor matched
# and the replacements landed, not that the Rust compiles. Build the patched
# herdr for that.
#
#   scripts/check-herdr-patches.sh [path-to-herdr-checkout]   (default ~/Code/herdr)
#
# Prints the patched hunks for eyeballing; exits non-zero on the first anchor
# that does not match, naming it.
set -euo pipefail

drip_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
herdr_src=${1:-$HOME/Code/herdr}
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT

if [[ ! -f $herdr_src/src/app/state.rs ]]; then
  echo "not a herdr checkout: $herdr_src" >&2
  exit 2
fi

echo "herdr:  $herdr_src ($(git -C "$herdr_src" describe --always --dirty 2>/dev/null || echo 'no git'))"
echo "drip:   $drip_root"
echo

cp -a -- "$herdr_src/src" "$scratch/src"
cp -a -- "$herdr_src/Cargo.toml" "$scratch/" 2>/dev/null || true

# The nixpkgs builder function the patch set actually calls, near enough for
# this purpose: literal replacement of every occurrence, non-zero when the
# pattern is not there. Only the --replace-fail form is implemented, because
# that is the only form the rules of the set allow.
#
# Bash rather than an interpreter on purpose: `${body//"$needle"/...}` with the
# needle quoted is a literal substitution, no pattern metacharacters and no
# escaping to get wrong, and it keeps this runnable on a host that has nothing
# but a shell -- which is the same reason the drip's own runtime pieces avoid
# depending on what is on the server's PATH. `$(<file)` drops trailing
# newlines and `printf '%s\n'` puts back exactly one, which is what a rust
# source file has.
substituteInPlace() {
  local file=$1
  shift
  local body replaced rest count
  while [[ $# -gt 0 ]]; do
    case $1 in
      --replace-fail)
        body=$(<"$file")
        replaced=${body//"$2"/"$3"}
        if [[ $replaced == "$body" ]]; then
          printf 'anchor not found in %s:\n  %s\n' "$file" "$2" >&2
          exit 1
        fi
        rest=$body
        count=0
        while [[ $rest == *"$2"* ]]; do
          rest=${rest#*"$2"}
          count=$((count + 1))
        done
        if [[ $count -gt 1 ]]; then
          printf '  note: %d occurrences in %s of:\n    %s\n' "$count" "$file" "$2" >&2
        fi
        printf '%s\n' "$replaced" > "$file"
        shift 3
        ;;
      *)
        echo "check-herdr-patches: unsupported substituteInPlace arg: $1" >&2
        exit 2
        ;;
    esac
  done
}
export -f substituteInPlace

# The postPatch body, lifted out of the nix file rather than copied into this
# one, so the thing under test is the thing that ships. Two pieces of nix
# survive extraction: the ''-string's escapes and the path interpolations,
# which are the .rs files appended alongside the substitutions.
awk '/^  postPatch = /{flag=1; next} /^  ..;$/{flag=0} flag' \
  "$drip_root/nix/herdr-patches.nix" \
  | sed -e "s#\${\./#$drip_root/nix/#g" -e "s#\.rs}#.rs#g" \
  | sed -e "s/''\\\${/\${/g" -e "s/'''/''/g" \
  > "$scratch/postPatch.sh"

if ! grep -q substituteInPlace "$scratch/postPatch.sh"; then
  echo "extracted no postPatch body -- has nix/herdr-patches.nix changed shape?" >&2
  exit 2
fi

# A nix interpolation inside a REPLACEMENT -- sidebar-version's ${dripRev} --
# lands in the patched file verbatim here: it sits in single quotes, so the
# shell never expands it and nix is not in the loop to. That is cosmetic in the
# diff below and cannot affect what is being checked, because the anchors are
# the PATTERNS and no pattern interpolates.
( cd "$scratch" && source "$scratch/postPatch.sh" )

echo "all anchors matched"
echo
diff -ru --label "herdr/src" --label "patched/src" "$herdr_src/src" "$scratch/src" || true

#!/usr/bin/env bash
set -euo pipefail

# Keeps the triage table and the issue blocks of a deep-review report. Commands:
#   load   <report>            Add the triage table if the report has none. Create the returns
#                              directory. Print each row that is not fixed, rejected, or skipped.
#   ingest <report>            Record each return file in the returns directory, then delete it.
#   wave   <report> [<size>]   Set the next wave of queued units to in-progress, and print them.
#                              <size> is the most units in one wave, 4 by default.
#   set    <report> <ID> [status=<status>] [note=<text>] [reason=<text>]
#                              <status> is queued, in-progress, skipped, or blocked. skipped and
#                              blocked need a reason, which the resolution block records.
# The returns directory is the report path without `.md`, plus `.returns`. Each subagent writes
# its return block to `<returns directory>/<ID>.txt`.

usage() {
  sed -n '4,14p' "$0" | sed 's/^# //' >&2
  exit 2
}

die() {
  echo "ERROR: $*" >&2
  exit 1
}

IFS= read -r -d '' PROG <<'AWK' || true
function trim(s) { sub(/^[ \t\r]+/, "", s); sub(/[ \t\r]+$/, "", s); return s }
function fatal(msg) { print "ERROR: " msg > "/dev/stderr"; exit 1 }
function reject(msg) { print "ERROR: " RET ": " msg > "/dev/stderr"; BAD = 1 }
function is_fence(line) { return line ~ /^[ \t]*(```|~~~)/ }
function flat(s) { gsub(/[\r\n]+/, " ", s); return trim(s) }

function esc(s,   out, i, c) {
  out = ""
  for (i = 1; i <= length(s); i++) {
    c = substr(s, i, 1)
    out = out (c == "|" ? "\\|" : c)
  }
  return out
}

function split_row(line, cells,   n, i, c, cur) {
  line = trim(line)
  sub(/^\|/, "", line)
  if (line ~ /[^\\]\|$/) line = substr(line, 1, length(line) - 1)
  n = 1
  cur = ""
  for (i = 1; i <= length(line); i++) {
    c = substr(line, i, 1)
    if (c == "\\" && substr(line, i + 1, 1) == "|") { cur = cur "\\|"; i++ }
    else if (c == "|") { cells[n++] = trim(cur); cur = "" }
    else cur = cur c
  }
  cells[n] = trim(cur)
  return n
}

function row_text(r,   k, s) {
  s = "|"
  for (k = 1; k <= 8; k++) s = s " " R[r, k] " |"
  return s
}

function ticks(list,   n, i, a, f, out) {
  if (tolower(trim(list)) == "none") return ""
  n = split(list, a, ",")
  out = ""
  for (i = 1; i <= n; i++) {
    f = a[i]
    gsub(/`/, "", f)
    f = trim(f)
    if (f != "") out = out (out == "" ? "" : ", ") "`" f "`"
  }
  return out
}

function tokens(cell, arr,   n) {
  split("", arr)
  n = 0
  while (match(cell, /`[^`]+`/)) {
    arr[++n] = substr(cell, RSTART + 1, RLENGTH - 2)
    cell = substr(cell, RSTART + RLENGTH)
  }
  return n
}

function union_files(cell,   n, t, i, seen, out) {
  n = tokens(cell, t)
  out = ""
  for (i = 1; i <= n; i++) {
    if (t[i] in seen) continue
    seen[t[i]] = 1
    out = out (out == "" ? "" : ", ") "`" t[i] "`"
  }
  return out
}

function ids_in(s, own,   n, a, i, skip, id, out) {
  n = split(own, a, ",")
  for (i = 1; i <= n; i++) skip[trim(a[i])] = 1
  out = ""
  while (match(s, /[A-Z]+-[0-9]+/)) {
    id = substr(s, RSTART, RLENGTH)
    s = substr(s, RSTART + RLENGTH)
    if (id in skip) continue
    skip[id] = 1
    out = out (out == "" ? "" : ", ") id
  }
  return out
}

function first_id(cell,   a) { split(cell, a, ","); return trim(a[1]) }

function read_return(   line, key) {
  line = $0
  if (is_fence(line)) return
  if (match(line, /^[A-Z]+:/)) {
    key = substr(line, 1, RLENGTH - 1)
    if (key ~ /^(ID|VERDICT|JUDGMENT|RISK|FOOTPRINT|GROUP|REASON|FIX|OPTIONS|RECOMMEND|STATUS|FILES|TESTS|NOTE|NEEDS)$/) {
      if (key == "ID" || NB == 0) NB++
      F[NB, key] = trim(substr(line, RLENGTH + 1))
      CUR = key
      return
    }
  }
  if (CUR != "" && trim(line) != "")
    F[NB, CUR] = F[NB, CUR] (F[NB, CUR] == "" ? "" : (CUR == "OPTIONS" ? "\n" : " ")) trim(line)
}

function word(b, key,   v) {
  v = tolower(F[b, key])
  gsub(/[`*]/, "", v)
  return trim(v)
}

function add_row(i,   c, n, k, a, m) {
  n = split_row(L[i], c)
  if (n != 8) fatal("the triage row at line " i " has " n " cells, but the table has 8 columns. Fix the row, then run the command again.")
  NROW++
  ROW_LINE[NROW] = i
  for (k = 1; k <= 8; k++) R[NROW, k] = c[k]
  m = split(c[1], a, ",")
  for (k = 1; k <= m; k++) ROW_OF[trim(a[k])] = NROW
}

function index_report(   i, fence, lvl, open, rest, id) {
  fence = 0
  open = 0
  for (i = 1; i <= N; i++) {
    if (is_fence(L[i])) { fence = !fence; continue }
    if (fence) continue
    if (!match(L[i], /^#+ /)) {
      if (TRI && !TRI_END && L[i] ~ /^\|[ \t]*[A-Z]+-[0-9]+/) add_row(i)
      continue
    }
    lvl = RLENGTH - 1
    if (lvl > 3) continue
    if (open) { SEC_END[open] = i - 1; open = 0 }
    if (lvl <= 2) {
      if (SUM && !SUM_END) SUM_END = i - 1
      if (TRI && !TRI_END) TRI_END = i - 1
      if (L[i] ~ /^## Summary[ \t]*$/) SUM = i
      if (L[i] ~ /^## Triage[ \t]*$/) TRI = i
    }
    if (lvl == 3 && L[i] ~ /^### \[[A-Z]+-[0-9]+\]/) {
      open = ++NSEC
      SEC_START[NSEC] = i
      rest = substr(L[i], 5)
      while (match(rest, /^\[[A-Z]+-[0-9]+\][ \t]*/)) {
        id = substr(rest, 2, index(rest, "]") - 2)
        SEC_IDS[NSEC] = SEC_IDS[NSEC] (SEC_IDS[NSEC] == "" ? "" : ", ") id
        SEC_OF[id] = NSEC
        rest = substr(rest, RLENGTH + 1)
      }
      SEC_TITLE[NSEC] = trim(rest)
    }
  }
  if (open) SEC_END[open] = N
  if (SUM && !SUM_END) SUM_END = N
  if (TRI && !TRI_END) TRI_END = N
}

function do_load(   k, r, s) {
  if (!TRI) {
    if (!SUM) fatal("the report has no \"## Summary\" section, so the triage table has no place to go. Check that the file is a deep-review report.")
    if (!NSEC) fatal("the report has no issues. Each issue starts with a \"### [ID]\" heading.")
    for (k = 1; k <= NSEC; k++) {
      R[k, 1] = SEC_IDS[k]
      R[k, 2] = esc(SEC_TITLE[k])
      R[k, 3] = R[k, 4] = R[k, 5] = R[k, 7] = R[k, 8] = "—"
      R[k, 6] = "open"
    }
    NROW = NSEC
    INSERT_AFTER = SUM_END
  }
  for (r = 1; r <= NROW; r++) {
    s = R[r, 6]
    if (s == "fixed" || s == "rejected" || s == "skipped") { DONE[s]++; continue }
    print row_text(r)
  }
  print "Done: " (DONE["fixed"] + 0) " fixed, " (DONE["rejected"] + 0) " rejected, " (DONE["skipped"] + 0) " skipped."
}

function check_validation(b, id,   v, j, k) {
  v = word(b, "VERDICT")
  j = word(b, "JUDGMENT")
  k = word(b, "RISK")
  if (v !~ /^(confirmed|confirmed-alt|rejected|unclear)$/) { reject(id ": VERDICT must be confirmed, confirmed-alt, rejected, or unclear."); return }
  if (F[b, "REASON"] == "") reject(id ": REASON is empty.")
  if (v !~ /^confirmed/) return
  if (j !~ /^(mechanical|choice|design)$/) reject(id ": JUDGMENT must be mechanical, choice, or design.")
  if (k !~ /^(low|medium|high)$/) reject(id ": RISK must be low, medium, or high.")
  if (ticks(F[b, "FOOTPRINT"]) == "") reject(id ": FOOTPRINT is empty.")
  if (v == "confirmed-alt" && F[b, "FIX"] == "") reject(id ": FIX is empty, but the verdict is confirmed-alt.")
  if (j == "choice" && (F[b, "OPTIONS"] == "" || F[b, "RECOMMEND"] == "")) reject(id ": OPTIONS and RECOMMEND are required, because the judgment is choice.")
}

function check_fix(b, id,   s) {
  s = word(b, "STATUS")
  if (s !~ /^(fixed|blocked)$/) { reject(id ": STATUS must be fixed or blocked."); return }
  if (F[b, "NOTE"] == "") reject(id ": NOTE is empty.")
  if (s == "fixed" && ticks(F[b, "FILES"]) == "") reject(id ": FILES is empty, but the status is fixed.")
}

function options(s,   n, a, i, t, out) {
  n = split(s, a, "\n")
  out = ""
  for (i = 1; i <= n; i++) {
    t = trim(a[i])
    sub(/^[-*][ \t]+/, "", t)
    if (t == "") continue
    if (t ~ /^[A-Z][.)]/) t = "(" substr(t, 1, 1) ") " trim(substr(t, 3))
    out = out (out == "" ? "" : " ") t
  }
  return out
}

function apply_validation(b, id,   r, k, v, j, rk, st, blk, g) {
  r = ROW_OF[id]
  k = SEC_OF[id]
  if (R[r, 6] != "open") { print id ": ignored, because its status is " R[r, 6] ", not open."; return }
  v = word(b, "VERDICT")
  j = word(b, "JUDGMENT")
  rk = word(b, "RISK")
  R[r, 3] = v
  blk = "- **Validation:** " v "\n  - Evidence: " flat(F[b, "REASON"])
  if (v ~ /^confirmed/) {
    R[r, 4] = j
    R[r, 5] = rk
    R[r, 7] = esc(ticks(F[b, "FOOTPRINT"]))
    st = (j == "mechanical" && rk != "high") ? "queued" : "awaiting-decision"
    if (v == "confirmed-alt") {
      blk = blk "\n  - Fix: " flat(F[b, "FIX"])
      R[r, 8] = "Apply the validator's fix."
    }
    if (j == "choice") blk = blk "\n  - Options: " options(F[b, "OPTIONS"]) "\n  - Recommend: " flat(F[b, "RECOMMEND"])
    g = ids_in(F[b, "GROUP"], R[r, 1])
    if (g != "") blk = blk "\n  - Group: " g
  } else {
    st = v
    NEWR[k] = "- **Resolution:** " v
  }
  R[r, 6] = st
  ROW_DIRTY[r] = 1
  NEWV[k] = blk
  SEC_DIRTY[k] = 1
  print id ": " v (v ~ /^confirmed/ ? ", " j ", " rk : "") " -> " st
}

function apply_fix(b, id,   r, k, s, tests, needs, blk) {
  r = ROW_OF[id]
  k = SEC_OF[id]
  if (r in APPLIED) return
  APPLIED[r] = 1
  if (R[r, 6] != "in-progress") { print id ": ignored, because its status is " R[r, 6] ", not in-progress."; return }
  s = word(b, "STATUS")
  tests = flat(F[b, "TESTS"])
  if (tests == "") tests = "not run"
  needs = ticks(F[b, "NEEDS"])
  if (s == "fixed") {
    R[r, 6] = "fixed"
    R[r, 7] = esc(ticks(F[b, "FILES"]))
    NEWR[k] = "- **Resolution:** fixed\n  - Changed: " ticks(F[b, "FILES"]) ".\n  - Tests: " tests "\n  - Note: " flat(F[b, "NOTE"])
    print id ": fixed"
  } else if (needs != "") {
    R[r, 6] = "queued"
    R[r, 7] = esc(union_files(R[r, 7] ", " needs))
    print id ": queued, with " needs " added to the footprint"
  } else {
    R[r, 6] = "blocked"
    blk = "- **Resolution:** blocked\n  - Reason: " flat(F[b, "NOTE"])
    if (tests !~ /^not run/) blk = blk "\n  - Tests: " tests
    NEWR[k] = blk
    print id ": blocked. " flat(F[b, "NOTE"])
  }
  ROW_DIRTY[r] = 1
  SEC_DIRTY[k] = 1
}

function do_ingest(   b, id) {
  if (!TRI) fatal("the report has no triage table. Run load first.")
  if (!NB) { reject("the file holds no return block. A block starts with an ID: line."); return }
  for (b = 1; b <= NB; b++) {
    id = ""
    if (match(F[b, "ID"], /[A-Z]+-[0-9]+/)) id = substr(F[b, "ID"], RSTART, RLENGTH)
    BID[b] = id
    if (id == "") reject("block " b " has no ID.")
    else if (!(id in ROW_OF) || !(id in SEC_OF)) reject(id " has no row in the triage table, or no issue heading.")
    else if (F[b, "VERDICT"] != "") check_validation(b, id)
    else if (F[b, "STATUS"] != "") check_fix(b, id)
    else reject(id ": the block has neither a VERDICT nor a STATUS line.")
  }
  if (BAD) return
  for (b = 1; b <= NB; b++) {
    if (F[b, "VERDICT"] != "") apply_validation(b, BID[b])
    else apply_fix(b, BID[b])
  }
}

function find(x) { while (P[x] != x) x = P[x]; return x }
function join(x, y) { x = find(x); y = find(y); if (x < y) P[y] = x; else if (y < x) P[x] = y }

function group_of(r,   k, i) {
  k = SEC_OF[first_id(R[r, 1])]
  if (!k) return ""
  for (i = SEC_START[k]; i <= SEC_END[k]; i++) if (L[i] ~ /^[ \t]+- Group: /) return L[i]
  return ""
}

function pick(u,   n, a, i, r) {
  n = split(trim(UROWS[u]), a, " ")
  for (i = 1; i <= n; i++) {
    r = a[i] + 0
    R[r, 6] = "in-progress"
    ROW_DIRTY[r] = 1
    PICKED[r] = 1
  }
  print "UNIT " UIDS[u]
  print "  footprint: " (UUNK[u] ? "unknown, so this unit runs alone" : UFILES[u])
  if (UNOTE[u] != "") { gsub(/\\\|/, "|", UNOTE[u]); print "  note: " UNOTE[u] }
  print "  return: " ENVIRON["TRIAGE_RETURNS"] "/" first_id(R[UFIRST[u], 1]) ".txt"
}

function do_wave(   size, r, i, j, m, a, nq, nu, u, root, t, c, chosen, clash, left, busy) {
  if (!TRI) fatal("the report has no triage table. Run load first.")
  size = ENVIRON["TRIAGE_SIZE"] + 0
  nq = 0
  for (r = 1; r <= NROW; r++) {
    if (R[r, 6] == "queued") { Q[++nq] = r; P[r] = r }
    if (R[r, 6] == "in-progress") { busy = 1; c = tokens(R[r, 7], t); for (j = 1; j <= c; j++) USED[t[j]] = 1 }
  }
  if (!nq) { print "No queued issues."; return }
  for (i = 1; i <= nq; i++) {
    m = split(ids_in(group_of(Q[i]), ""), a, ", ")
    for (j = 1; j <= m; j++)
      if ((a[j] in ROW_OF) && R[ROW_OF[a[j]], 6] == "queued") join(Q[i], ROW_OF[a[j]])
  }
  nu = 0
  for (i = 1; i <= nq; i++) {
    r = Q[i]
    root = find(r)
    if (!(root in UNIT)) { UNIT[root] = ++nu; UFIRST[nu] = r }
    u = UNIT[root]
    UIDS[u] = UIDS[u] (UIDS[u] == "" ? "" : ", ") R[r, 1]
    UROWS[u] = UROWS[u] " " r
    if (R[r, 8] != "—" && R[r, 8] != "") UNOTE[u] = UNOTE[u] (UNOTE[u] == "" ? "" : " ") first_id(R[r, 1]) ": " R[r, 8]
    if (!tokens(R[r, 7], t)) UUNK[u] = 1
    UFILES[u] = union_files(UFILES[u] ", " R[r, 7])
  }
  chosen = 0
  for (u = 1; u <= nu && chosen < size; u++) {
    if (UUNK[u]) {
      if (chosen || busy) continue
      pick(u)
      break
    }
    c = tokens(UFILES[u], t)
    clash = 0
    for (j = 1; j <= c; j++) if (t[j] in USED) clash = 1
    if (clash) continue
    for (j = 1; j <= c; j++) USED[t[j]] = 1
    pick(u)
    chosen++
  }
  left = 0
  for (i = 1; i <= nq; i++) if (!(Q[i] in PICKED)) left++
  if (left == nq) print "No queued unit can start until the in-progress fixers return."
  print "Queued after this wave: " left
}

function do_set(   id, r, k, st) {
  if (!TRI) fatal("the report has no triage table. Run load first.")
  id = ENVIRON["TRIAGE_ID"]
  if (!(id in ROW_OF)) fatal(id " has no row in the triage table. Run load to list the rows.")
  r = ROW_OF[id]
  st = ENVIRON["TRIAGE_STATUS"]
  if (st != "") {
    if (!(id in SEC_OF)) fatal(id " has no issue heading, so its resolution block has no place to go.")
    k = SEC_OF[id]
    R[r, 6] = st
    NEWR[k] = (st == "skipped" || st == "blocked") ? ("- **Resolution:** " st "\n  - Reason: " flat(ENVIRON["TRIAGE_REASON"])) : ""
    SEC_DIRTY[k] = 1
  }
  if (ENVIRON["TRIAGE_NOTE_SET"] == "1") R[r, 8] = (flat(ENVIRON["TRIAGE_NOTE"]) == "" ? "—" : esc(flat(ENVIRON["TRIAGE_NOTE"])))
  ROW_DIRTY[r] = 1
  print R[r, 1] ": " R[r, 6] " | " R[r, 8]
}

function emit_section(k,   i, line, b, nb, inblk, oldv, oldr, fence, v, res) {
  nb = 0
  inblk = ""
  oldv = ""
  oldr = ""
  fence = 0
  for (i = SEC_START[k]; i <= SEC_END[k]; i++) {
    line = L[i]
    if (!fence && line ~ /^- \*\*Validation:\*\*/) { inblk = "v"; oldv = line; continue }
    if (!fence && line ~ /^- \*\*Resolution:\*\*/) { inblk = "r"; oldr = line; continue }
    if (inblk != "" && line ~ /^[ \t]+[^ \t]/) {
      if (inblk == "v") oldv = oldv "\n" line
      else oldr = oldr "\n" line
      continue
    }
    inblk = ""
    if (is_fence(line)) fence = !fence
    b[++nb] = line
  }
  while (nb > 0 && trim(b[nb]) == "") nb--
  v = (k in NEWV) ? NEWV[k] : oldv
  res = (k in NEWR) ? NEWR[k] : oldr
  for (i = 1; i <= nb; i++) print b[i] > OUT
  if ((v != "" || res != "") && b[nb] !~ /^- /) print "" > OUT
  if (v != "") print v > OUT
  if (res != "") print res > OUT
  if (SEC_END[k] < N) print "" > OUT
}

function emit_table(   r) {
  if (trim(L[INSERT_AFTER]) != "") print "" > OUT
  print "## Triage" > OUT
  print "" > OUT
  print "| ID | Title | Verdict | Judgment | Risk | Status | Files Touched | Note |" > OUT
  print "| --- | --- | --- | --- | --- | --- | --- | --- |" > OUT
  for (r = 1; r <= NROW; r++) print row_text(r) > OUT
  print "" > OUT
}

function render(   i, r, k) {
  for (r = 1; r <= NROW; r++) if (ROW_DIRTY[r]) ROWAT[ROW_LINE[r]] = r
  for (k = 1; k <= NSEC; k++) if (SEC_DIRTY[k]) SECAT[SEC_START[k]] = k
  for (i = 1; i <= N; i++) {
    if (i in ROWAT) print row_text(ROWAT[i]) > OUT
    else if (i in SECAT) { k = SECAT[i]; emit_section(k); i = SEC_END[k] }
    else print L[i] > OUT
    if (i == INSERT_AFTER) emit_table()
  }
  close(OUT)
}

BEGIN {
  MODE = ENVIRON["TRIAGE_MODE"]
  RET = ENVIRON["TRIAGE_RET"]
  OUT = ENVIRON["TRIAGE_OUT"]
}
RET != "" && FILENAME == RET { read_return(); next }
{ L[++N] = $0 }
END {
  index_report()
  if (MODE == "load") do_load()
  else if (MODE == "ingest") do_ingest()
  else if (MODE == "wave") do_wave()
  else if (MODE == "set") do_set()
  if (BAD) exit 3
  render()
}
AWK

# awk exits 3 when it rejects a return file, and the report is then left unchanged.
rewrite() {
  local tmp code=0
  tmp="$(mktemp "$REPORT.XXXXXX")" ||
    die "cannot create a temporary file next to '$REPORT'. Check that its folder is writable."
  cp -p "$REPORT" "$tmp"
  export TRIAGE_MODE="$1" TRIAGE_RET="${2:-}" TRIAGE_OUT="$tmp"
  awk "$PROG" ${2:+"$2"} "$REPORT" || code=$?
  if [ "$code" -eq 0 ]; then mv "$tmp" "$REPORT"; else rm -f "$tmp"; fi
  return "$code"
}

CMD="${1:-}"
REPORT="${2:-}"
[ -n "$CMD" ] && [ -n "$REPORT" ] || usage
shift 2
[ -f "$REPORT" ] || die "no report at '$REPORT'. Pass the path of a deep-review report."
export TRIAGE_RETURNS="${REPORT%.md}.returns"

case "$CMD" in
  load)
    [ "$#" -eq 0 ] || usage
    mkdir -p "$TRIAGE_RETURNS" ||
      die "cannot create '$TRIAGE_RETURNS'. Check that the report's folder is writable."
    echo "RETURNS=$TRIAGE_RETURNS"
    rewrite load
    ;;
  ingest)
    [ "$#" -eq 0 ] || usage
    found=0
    failed=0
    for f in "$TRIAGE_RETURNS"/*.txt; do
      [ -f "$f" ] || continue
      found=1
      code=0
      rewrite ingest "$f" || code=$?
      case "$code" in
        0) rm -f "$f" ;;
        3) echo "ERROR: kept '$f'. Run its subagent again, then run ingest again." >&2; failed=1 ;;
        *) exit "$code" ;;
      esac
    done
    [ "$found" -eq 1 ] || echo "No return files in '$TRIAGE_RETURNS'."
    exit "$failed"
    ;;
  wave)
    [ "$#" -le 1 ] || usage
    size="${1:-4}"
    case "$size" in
      '' | *[!0-9]* | 0) die "wave size '$size' is not a positive whole number." ;;
    esac
    export TRIAGE_SIZE="$size"
    rewrite wave
    ;;
  set)
    ID="${1:-}"
    [ -n "$ID" ] || usage
    shift
    [ "$#" -gt 0 ] || usage
    status=""
    note=""
    reason=""
    note_set=0
    for a in "$@"; do
      case "$a" in
        status=*) status="${a#status=}" ;;
        note=*) note="${a#note=}"; note_set=1 ;;
        reason=*) reason="${a#reason=}" ;;
        *) die "unknown setting '$a'. Use status=, note=, or reason=." ;;
      esac
    done
    case "$status" in
      '' | queued | in-progress) [ -z "$reason" ] || die "reason= applies only to status=skipped or status=blocked." ;;
      skipped | blocked) [ -n "$reason" ] || die "status=$status needs reason=<text> for the resolution block." ;;
      *) die "status '$status' is not queued, in-progress, skipped, or blocked." ;;
    esac
    export TRIAGE_ID="$ID" TRIAGE_STATUS="$status" TRIAGE_NOTE="$note" TRIAGE_NOTE_SET="$note_set"
    export TRIAGE_REASON="$reason"
    rewrite set
    ;;
  *) usage ;;
esac

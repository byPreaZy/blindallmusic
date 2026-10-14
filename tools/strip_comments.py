import os, re, sys

KEEP = re.compile(r'//\s*(TODO|FIXME|HACK|XXX|SAFETY|NOTE|NB|WARN)', re.I)
DIVIDER = re.compile(r'^\s*//\s*[-=]{2,}.*[-=]{2,}\s*$')

def strip_rs(src: str) -> str:
    out = []
    i, n = 0, len(src)
    line_start = True          # at start of a line (only ws seen)
    in_str = False             # "..."
    in_char = False            # '.'
    raw_close = None           # e.g. '"#' for r#""# ; None
    block_depth = 0
    line_buf = []              # chars of current line (kept until decided)
    line_is_comment = None     # None / 'line' comment being captured
    skip_line = False

    def flush_comment_line(buf):
        text = ''.join(buf)
        return text

    pending = ''   # accumulating current output line
    lbuf = ''      # current comment text buffer

    while i < n:
        c = src[i]
        nxt = src[i+1] if i+1 < n else ''

        if raw_close is not None:
            pending += c
            if src.startswith(raw_close, i):
                # already added first char; add rest
                pending += raw_close[1:]
                i += len(raw_close)
                raw_close = None
                continue
            if c == '\n':
                line_start = True
            i += 1
            continue

        if in_str:
            pending += c
            if c == '\\':
                if i+1 < n:
                    pending += src[i+1]; i += 2; continue
            elif c == '"':
                in_str = False
            if c == '\n':
                line_start = True
            i += 1
            continue

        if in_char:
            pending += c
            if c == '\\':
                if i+1 < n:
                    pending += src[i+1]; i += 2; continue
            elif c == "'":
                in_char = False
            i += 1
            continue

        if block_depth > 0:
            if c == '/' and nxt == '*':
                block_depth += 1; i += 2; continue
            if c == '*' and nxt == '/':
                block_depth -= 1; i += 2; continue
            if c == '\n':
                # end of line inside block comment: drop line buffer
                pending = pending.rstrip(' \t')
                if pending.strip() == '':
                    pending = ''
                else:
                    out.append(pending)
                    pending = ''
                line_start = True
            i += 1
            continue

        if line_is_comment is not None:
            lbuf += c
            if c == '\n':
                txt = lbuf.rstrip('\n')
                keep = bool(KEEP.search(txt)) or bool(DIVIDER.match(txt))
                # drop commented-out code too (starts with // then code-ish)
                if keep:
                    pending += txt
                    out.append(pending)
                pending = ''
                lbuf = ''
                line_is_comment = None
                line_start = True
            i += 1
            continue

        # NORMAL state
        if c == '\n':
            out.append(pending); pending = ''
            line_start = True
            i += 1
            continue
        if c == 'r' and nxt == '"':
            pending += c; in_str = False; raw_close = '"'; pending += '"'; i += 2; continue
        if c == 'r' and nxt == '#':
            m = re.match(r'#+', src[i+1:])
            hs = m.group(0)
            if src[i+1+len(hs)] == '"':
                pending += 'r' + hs + '"'
                i += 1 + len(hs) + 1
                raw_close = '"' + hs
                continue
            pending += c; i += 1; continue
        if c == '"':
            pending += c; in_str = True; i += 1; line_start = False; continue
        if c == "'":
            # lifetime 'a vs char 'x' / '\n'
            m = re.match(r"'(\\.|[^\\'])'", src[i:])
            if m:
                pending += m.group(0); i += len(m.group(0)); line_start = False; continue
            pending += c; i += 1; continue   # lifetime, not a string
        if c == '/' and nxt == '/':
            if pending.strip() == '':
                line_is_comment = True
                lbuf = '//'
                i += 2
                continue
            else:
                pending += '//'; i += 2
                # trailing comment: copy rest of line verbatim
                j = src.find('\n', i)
                if j == -1:
                    pending += src[i:]; i = n; break
                pending += src[i:j]; i = j; continue
        if c == '/' and nxt == '*':
            block_depth = 1; i += 2
            pending = pending.rstrip(' \t')
            continue
        if c in ' \t':
            pending += c; i += 1; continue
        pending += c
        line_start = False
        i += 1

    if lbuf:
        txt = lbuf
        if KEEP.search(txt) or DIVIDER.match(txt):
            pending += txt
    if pending:
        out.append(pending)
    return '\n'.join(out) + ('\n' if src.endswith('\n') else '')

def main(root):
    changed = []
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in
                   ('target','target-e2e','node_modules','.git','dist','.trunk',
                    'volumes','.libs','.tools','public')]
        for f in files:
            if not f.endswith('.rs'):
                continue
            p = os.path.join(dirpath, f)
            s = open(p, encoding='utf-8').read()
            t = strip_rs(s)
            if t != s:
                open(p, 'w', encoding='utf-8', newline='\n').write(t)
                changed.append(p)
    print('\n'.join(changed))
    print(f'{len(changed)} files changed')

if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else '.')

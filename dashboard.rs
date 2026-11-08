// Glance Dashboard: a G2-style heads-up dashboard for MIRA glasses.
// Glasses side (Rust -> WASM). The phone companion owns all data and networking;
// it publishes one compact snapshot, and this module only lays it out and
// reports task check-offs back.
//
// Snapshot JSON from the phone: {"t":"d","p":"<packed>"}
// <packed> is newline-separated records, tab-separated fields:
//   W  <weather text>
//   S  <12|24>                      clock format
//   E  <start min from today 00:00>  <end min>  <title>  <tag>  <1 = all day>
//   M  <1 unread>  <from>  <subject>  <when>      inbox thread
//   K  <task id digits>  <0|1 done>  <text>
//   N  <note title>  <note body, \r = line break>
// Messages to the phone: {"type":"sync"} and {"type":"set","id":"..","done":true}
#![no_std]
#![allow(static_mut_refs)]
#[path = "../Shared/companion.rs"] mod companion;
#[path = "../Shared/content.rs"] mod content;

app_icon!(content::app_icon::Icon::new()
    .rect(3, 5, 26, 22).line(3, 11, 29, 11).line(8, 16, 22, 16).line(8, 21, 16, 21).0);

#[link(wasm_import_module = "mira")]
extern "C" {
    fn begin();
    fn present();
    fn text(x: i32, y: i32, w: i32, h: i32, c: i32, font: i32, a: i32, s: i32, p: *const u8, n: i32);
    fn fill_round_rect(x: i32, y: i32, w: i32, h: i32, r: i32, c: i32);
    fn stroke_round_rect(x: i32, y: i32, w: i32, h: i32, r: i32, s: i32, c: i32);
    fn line(x0: i32, y0: i32, x1: i32, y1: i32, w: i32, c: i32);
    fn fill_circle(cx: i32, cy: i32, r: i32, c: i32);
    fn stroke_circle(cx: i32, cy: i32, r: i32, s: i32, c: i32);
    fn device_status(out: *mut u8, cap: i32) -> i32;
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

const NAME: &[u8] = b"Glance Dashboard";
// Font IDs: 0=24px 1=18px 2=32px 3=40px. Alignment 0 left, 1 center, 2 right.
const F24: i32 = 0;
const F18: i32 = 1;
const F32: i32 = 2;
const F40: i32 = 3;
const MAX_EV: usize = 10;
const MAX_MAIL: usize = 10;
const MAX_TASK: usize = 12;
const BUF_CAP: usize = 3500;
const TASK_ROWS: usize = 6;
const PAGE_NAMES: [&[u8]; 5] = [b"OVERVIEW", b"AGENDA", b"TO DO", b"INBOX", b"NOTE"];

#[derive(Clone, Copy)]
struct Span { s: u16, n: u16 }
const NONE: Span = Span { s: 0, n: 0 };
#[derive(Clone, Copy)]
struct Ev { start: i32, end: i32, allday: bool, title: Span, tag: Span }
#[derive(Clone, Copy)]
struct Task { id: Span, done: bool, text: Span }
#[derive(Clone, Copy)]
struct Mail { unread: bool, from: Span, subj: Span, when: Span }

static mut BRIDGE: companion::Companion<3600, 96> = companion::Companion::new(NAME);
static mut BUF: [u8; BUF_CAP] = [0; BUF_CAP];
static mut EVS: [Ev; MAX_EV] = [Ev { start: 0, end: 0, allday: false, title: NONE, tag: NONE }; MAX_EV];
static mut MAILS: [Mail; MAX_MAIL] = [Mail { unread: false, from: NONE, subj: NONE, when: NONE }; MAX_MAIL];
static mut MAIL_N: usize = 0;
static mut MSCROLL: usize = 0;
static mut EV_N: usize = 0;
static mut TASKS: [Task; MAX_TASK] = [Task { id: NONE, done: false, text: NONE }; MAX_TASK];
static mut TASK_N: usize = 0;
static mut WEATHER: Span = NONE;
static mut NOTE_T: Span = NONE;
static mut NOTE_B: Span = NONE;
static mut H12: bool = false;
static mut HAVE_DATA: bool = false;
static mut FOCUS: usize = 0;
static mut SCROLL: usize = 0;
static mut STATUS: [u8; 10] = [0; 10];
static mut STATUS_OK: bool = false;
static mut CLOCK_MS: u32 = 0;
static mut DIRTY: bool = true;
static mut WANT_SYNC: bool = true;
static mut SCRATCH: [u8; 160] = [0; 160];
// Small outgoing queue so quick successive check-offs are not lost.
static mut QUEUE: [[u8; 64]; 6] = [[0; 64]; 6];
static mut QUEUE_LEN: [usize; 6] = [0; 6];
static mut QUEUE_N: usize = 0;

// ---------- text measuring (firmware glyph advances, ASCII) ----------
const ADV18: [u8; 95] = [
    6, 7, 8, 15, 11, 17, 13, 4, 7, 7, 9, 15, 6, 7, 6, 6, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 6,
    6, 15, 15, 15, 10, 18, 12, 12, 13, 14, 11, 10, 14, 14, 6, 6, 12, 10, 16, 14, 14, 11, 14, 13,
    11, 12, 14, 12, 19, 13, 12, 13, 7, 6, 7, 15, 9, 9, 10, 11, 9, 11, 11, 6, 11, 11, 5, 5, 10, 5,
    17, 11, 11, 11, 11, 8, 8, 7, 11, 11, 16, 11, 11, 9, 11, 6, 11, 15,
];
const ADV24: [u8; 95] = [
    6, 7, 8, 15, 13, 19, 17, 4, 8, 8, 12, 14, 5, 7, 5, 8, 15, 10, 13, 12, 15, 14, 14, 13, 14, 14,
    7, 7, 14, 14, 14, 12, 24, 16, 15, 15, 17, 14, 13, 17, 18, 6, 11, 15, 13, 21, 18, 18, 15, 18,
    16, 14, 14, 18, 16, 24, 15, 14, 15, 8, 8, 8, 14, 8, 6, 13, 15, 12, 15, 13, 9, 14, 13, 5, 6, 12,
    4, 22, 14, 14, 15, 14, 9, 11, 10, 14, 13, 20, 12, 13, 12, 8, 6, 8, 14,
];
const ADV32: [u8; 95] = [
    8, 8, 10, 19, 18, 25, 22, 6, 10, 10, 17, 18, 7, 11, 7, 10, 19, 12, 17, 17, 19, 18, 19, 18, 20,
    18, 7, 7, 18, 18, 18, 15, 31, 22, 20, 20, 22, 17, 17, 22, 23, 7, 15, 20, 16, 27, 23, 23, 19,
    23, 20, 18, 18, 23, 21, 32, 20, 20, 20, 11, 10, 10, 18, 10, 8, 18, 19, 16, 19, 18, 11, 19, 19,
    7, 7, 16, 7, 29, 19, 18, 19, 19, 11, 14, 12, 19, 17, 27, 16, 17, 16, 10, 8, 11, 18,
];
const ADV40: [u8; 95] = [
    10, 10, 13, 23, 23, 31, 28, 7, 13, 13, 21, 23, 8, 13, 8, 13, 25, 16, 21, 21, 23, 23, 23, 23,
    24, 22, 10, 10, 23, 23, 23, 20, 38, 27, 25, 26, 28, 22, 21, 28, 29, 9, 18, 25, 21, 35, 29, 30,
    23, 30, 25, 23, 22, 29, 27, 40, 25, 25, 24, 13, 13, 13, 23, 13, 10, 22, 25, 21, 25, 22, 14, 25,
    23, 8, 8, 20, 8, 36, 24, 24, 25, 25, 14, 19, 15, 23, 22, 33, 20, 22, 20, 14, 11, 13, 23,
];
fn px(font: i32) -> i32 { match font { F18 => 18, F32 => 32, F40 => 40, _ => 24 } }
fn adv(b: u8, font: i32) -> i32 {
    if (32..=126).contains(&b) {
        let i = (b - 32) as usize;
        (match font { F18 => ADV18[i], F32 => ADV32[i], F40 => ADV40[i], _ => ADV24[i] }) as i32
    } else if b < 32 || b & 0xc0 == 0x80 { 0 } else { px(font) }
}
fn width(s: &[u8], font: i32) -> i32 { s.iter().map(|b| adv(*b, font)).sum() }

/// Returns `src`, or a copy in SCRATCH cut to fit `w` with "...".
unsafe fn fit(src: &[u8], font: i32, w: i32, force: bool) -> &'static [u8] {
    if !force && width(src, font) <= w {
        let n = src.len().min(SCRATCH.len());
        SCRATCH[..n].copy_from_slice(&src[..n]);
        return &SCRATCH[..n];
    }
    let dots = width(b"...", font);
    let (mut n, mut used, mut i) = (0usize, 0i32, 0usize);
    while i < src.len() {
        let b = src[i];
        let len = if b < 128 { 1 } else if b & 0xe0 == 0xc0 { 2 } else if b & 0xf0 == 0xe0 { 3 } else { 4 };
        let a = adv(b, font);
        if used + a + dots > w || i + len > src.len() || n + len + 3 > SCRATCH.len() { break; }
        SCRATCH[n..n + len].copy_from_slice(&src[i..i + len]);
        n += len; used += a; i += len;
    }
    while n > 0 && SCRATCH[n - 1] == b' ' { n -= 1; }
    SCRATCH[n..n + 3].copy_from_slice(b"...");
    &SCRATCH[..n + 3]
}

unsafe fn label(x: i32, y: i32, w: i32, h: i32, gray: i32, font: i32, align: i32, src: &[u8]) {
    if src.is_empty() { return; }
    let s = fit(src, font, w, false);
    text(x, y, w, h, gray, font, align, px(font) + 6, s.as_ptr(), s.len() as i32);
}

/// Word-wraps `src` into at most `rows` lines; `\r` forces a break.
unsafe fn wrap(x: i32, y: i32, w: i32, lh: i32, rows: usize, gray: i32, font: i32, src: &[u8]) {
    let (mut start, mut row) = (0usize, 0usize);
    while start < src.len() && row < rows {
        while start < src.len() && src[start] == b' ' { start += 1; }
        if start >= src.len() { break; }
        let yy = y + row as i32 * lh;
        if row == rows - 1 {
            let mut e = start;
            while e < src.len() && src[e] != b'\r' { e += 1; }
            let more = e < src.len();
            let s = fit(&src[start..e], font, w, more);
            text(x, yy, w, lh, gray, font, 0, lh, s.as_ptr(), s.len() as i32);
            return;
        }
        let (mut i, mut used, mut space) = (start, 0i32, 0usize);
        let (mut end, mut next) = (src.len(), src.len());
        while i < src.len() {
            let b = src[i];
            if b == b'\r' { end = i; next = i + 1; break; }
            let a = adv(b, font);
            if used + a > w {
                if space > start { end = space; next = space + 1; } else {
                    let mut j = i;
                    while j > start + 1 && src[j] & 0xc0 == 0x80 { j -= 1; }
                    end = j.max(start + 1); next = end;
                }
                break;
            }
            if b == b' ' { space = i; }
            used += a; i += 1;
        }
        if end > start {
            text(x, yy, w, lh, gray, font, 0, lh, src[start..end].as_ptr(), (end - start) as i32);
        }
        start = next; row += 1;
    }
}

// ---------- snapshot parsing ----------
unsafe fn sl(sp: Span) -> &'static [u8] { &BUF[sp.s as usize..(sp.s + sp.n) as usize] }
fn num(b: &[u8]) -> i32 {
    b.iter().take_while(|c| c.is_ascii_digit()).fold(0i32, |n, c| n.saturating_mul(10).saturating_add((c - b'0') as i32))
}
fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    if n.len() > h.len() { return None; }
    (0..=h.len() - n.len()).find(|i| &h[*i..*i + n.len()] == n)
}
fn hex4(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |v, c| v * 16 + match c { b'0'..=b'9' => (c - b'0') as u32,
        b'a'..=b'f' => (c - b'a' + 10) as u32, b'A'..=b'F' => (c - b'A' + 10) as u32, _ => 0 })
}
fn utf8(c: u32, out: &mut [u8]) -> usize {
    if c < 0x80 { out[0] = c as u8; 1 }
    else if c < 0x800 { out[0] = 0xc0 | (c >> 6) as u8; out[1] = 0x80 | (c & 63) as u8; 2 }
    else { out[0] = 0xe0 | (c >> 12) as u8; out[1] = 0x80 | ((c >> 6) & 63) as u8; out[2] = 0x80 | (c & 63) as u8; 3 }
}
/// Decodes a JSON string body (up to its closing quote) into `out`.
fn unescape(src: &[u8], out: &mut [u8]) -> usize {
    let (mut i, mut n) = (0usize, 0usize);
    while i < src.len() && n + 4 <= out.len() {
        let b = src[i];
        if b == b'"' { break; }
        if b != b'\\' { out[n] = b; n += 1; i += 1; continue; }
        if i + 1 >= src.len() { break; }
        let e = src[i + 1]; i += 2;
        let c: u32 = match e {
            b'n' => 10, b't' => 9, b'r' => 13, b'b' | b'f' => 32,
            b'u' => {
                if i + 4 > src.len() { break; }
                let v = hex4(&src[i..i + 4]); i += 4;
                if (0xd800..0xdc00).contains(&v) && i + 6 <= src.len() && src[i] == b'\\' && src[i + 1] == b'u' { i += 6; }
                if (0xd800..0xe000).contains(&v) { b'?' as u32 } else { v }
            }
            other => other as u32,
        };
        n += utf8(c, &mut out[n..]);
    }
    n
}

unsafe fn parse_line(a: usize, b: usize) {
    let mut f = [NONE; 6];
    let (mut k, mut s, mut i) = (0usize, a, a);
    while i <= b && k < 6 {
        if i == b || BUF[i] == b'\t' { f[k] = Span { s: s as u16, n: (i - s) as u16 }; k += 1; s = i + 1; }
        i += 1;
    }
    if k == 0 || f[0].n != 1 { return; }
    match BUF[f[0].s as usize] {
        b'W' => WEATHER = f[1],
        b'S' => H12 = sl(f[1]) == b"12",
        b'E' if EV_N < MAX_EV => {
            let start = num(sl(f[1]));
            let end = if f[2].n > 0 { num(sl(f[2])) } else { start + 60 };
            EVS[EV_N] = Ev { start, end, allday: sl(f[5]) == b"1", title: f[3], tag: f[4] };
            EV_N += 1;
        }
        b'M' if MAIL_N < MAX_MAIL => {
            MAILS[MAIL_N] = Mail { unread: sl(f[1]) == b"1", from: f[2], subj: f[3], when: f[4] };
            MAIL_N += 1;
        }
        b'K' if TASK_N < MAX_TASK => { TASKS[TASK_N] = Task { id: f[1], done: sl(f[2]) == b"1", text: f[3] }; TASK_N += 1; }
        b'N' => { NOTE_T = f[1]; NOTE_B = f[2]; }
        _ => {}
    }
}

unsafe fn apply(msg: &[u8]) {
    if find(msg, b"\"t\":\"d\"").is_none() { return; }
    let key = b"\"p\":\"";
    let Some(pos) = find(msg, key) else { return };
    let n = unescape(&msg[pos + key.len()..], &mut BUF);
    EV_N = 0; TASK_N = 0; MAIL_N = 0; WEATHER = NONE; NOTE_T = NONE; NOTE_B = NONE; H12 = false;
    let mut start = 0usize;
    while start < n {
        let mut end = start;
        while end < n && BUF[end] != b'\n' { end += 1; }
        parse_line(start, end);
        start = end + 1;
    }
    HAVE_DATA = true;
    if FOCUS >= stops() { FOCUS = stops() - 1; }
    DIRTY = true;
}

// ---------- outgoing messages ----------
unsafe fn enqueue(msg: &[u8]) {
    if QUEUE_N >= QUEUE.len() || msg.len() > 64 { return; }
    QUEUE[QUEUE_N][..msg.len()].copy_from_slice(msg);
    QUEUE_LEN[QUEUE_N] = msg.len();
    QUEUE_N += 1;
}
unsafe fn send_set(i: usize) {
    let mut m = [0u8; 64];
    let head = b"{\"type\":\"set\",\"id\":\"";
    m[..head.len()].copy_from_slice(head);
    let mut n = head.len();
    for c in sl(TASKS[i].id) { if c.is_ascii_digit() && n < 40 { m[n] = *c; n += 1; } }
    let tail: &[u8] = if TASKS[i].done { b"\",\"done\":true}" } else { b"\",\"done\":false}" };
    m[n..n + tail.len()].copy_from_slice(tail);
    enqueue(&m[..n + tail.len()]);
}
unsafe fn flush() {
    if WANT_SYNC && QUEUE_N == 0 { enqueue(b"{\"type\":\"sync\"}"); WANT_SYNC = false; }
    if QUEUE_N == 0 { return; }
    if BRIDGE.send(&QUEUE[0][..QUEUE_LEN[0]]) {
        for k in 1..QUEUE_N { QUEUE[k - 1] = QUEUE[k]; QUEUE_LEN[k - 1] = QUEUE_LEN[k]; }
        QUEUE_N -= 1;
    }
}

// ---------- navigation ----------
// Focus stops: 0 overview, 1 agenda, then one per task (min 1),
// one per inbox thread (min 1), last = note.
unsafe fn mail_start() -> usize { 2 + TASK_N.max(1) }
unsafe fn stops() -> usize { mail_start() + MAIL_N.max(1) + 1 }
unsafe fn page() -> usize {
    if FOCUS == 0 { 0 } else if FOCUS == 1 { 1 } else if FOCUS + 1 == stops() { 4 }
    else if FOCUS >= mail_start() { 3 } else { 2 }
}
unsafe fn now_min() -> i32 {
    if STATUS_OK && STATUS[4] != 0 { STATUS[5] as i32 * 60 + STATUS[6] as i32 } else { -1 }
}

// ---------- formatting ----------
fn push(out: &mut [u8], n: &mut usize, s: &[u8]) {
    for c in s { if *n < out.len() { out[*n] = *c; *n += 1; } }
}
fn push_num(out: &mut [u8], n: &mut usize, v: i32, pad2: bool) {
    let v = v.max(0);
    if pad2 && v < 10 { push(out, n, b"0"); }
    let mut d = [0u8; 10]; let mut k = 0; let mut x = v;
    loop { d[k] = b'0' + (x % 10) as u8; k += 1; x /= 10; if x == 0 { break; } }
    while k > 0 { k -= 1; push(out, n, &d[k..k + 1]); }
}
unsafe fn clock(minute_of_day: i32, out: &mut [u8]) -> usize {
    let m = minute_of_day.rem_euclid(1440);
    let (h, mm) = (m / 60, m % 60);
    let mut n = 0;
    if H12 {
        push_num(out, &mut n, if h % 12 == 0 { 12 } else { h % 12 }, false);
        push(out, &mut n, b":"); push_num(out, &mut n, mm, true);
        push(out, &mut n, if h < 12 { b" AM" } else { b" PM" });
    } else {
        push_num(out, &mut n, h, true); push(out, &mut n, b":"); push_num(out, &mut n, mm, true);
    }
    n
}
fn countdown(delta: i32, out: &mut [u8]) -> usize {
    let mut n = 0;
    if delta < 0 { push(out, &mut n, b"now"); }
    else if delta >= 1440 { push(out, &mut n, if delta < 2880 { b"tomorrow" } else { b"later" }); }
    else {
        push(out, &mut n, b"in ");
        if delta >= 60 { push_num(out, &mut n, delta / 60, false); push(out, &mut n, b"h "); }
        push_num(out, &mut n, delta % 60, false); push(out, &mut n, b"m");
    }
    n
}
/// First timed event that has not ended yet.
unsafe fn first_upcoming() -> usize {
    let now = now_min();
    (0..EV_N).find(|i| !EVS[*i].allday && (now < 0 || EVS[*i].end > now)).unwrap_or(EV_N)
}
unsafe fn unread_count() -> usize { (0..MAIL_N).filter(|i| MAILS[*i].unread).count() }

// ---------- drawing ----------
unsafe fn header() {
    let mut c = [0u8; 16];
    let n = if now_min() >= 0 { clock(now_min(), &mut c) } else { c[..5].copy_from_slice(b"--:--"); 5 };
    text(32, 88, 300, 52, 15, F40, 0, 46, c.as_ptr(), n as i32);
    if STATUS_OK && STATUS[4] != 0 {
        const DAYS: [&[u8]; 7] = [b"SUN", b"MON", b"TUE", b"WED", b"THU", b"FRI", b"SAT"];
        const MONTHS: [&[u8]; 12] = [b"JAN", b"FEB", b"MAR", b"APR", b"MAY", b"JUN", b"JUL", b"AUG", b"SEP", b"OCT", b"NOV", b"DEC"];
        let mut d = [0u8; 24]; let mut k = 0;
        push(&mut d, &mut k, DAYS[(STATUS[9] as usize).min(6)]); push(&mut d, &mut k, b" ");
        push(&mut d, &mut k, MONTHS[(STATUS[7] as usize).clamp(1, 12) - 1]); push(&mut d, &mut k, b" ");
        push_num(&mut d, &mut k, STATUS[8] as i32, false);
        label(330, 92, 278, 28, 12, F24, 2, &d[..k]);
    }
    label(330, 122, 278, 24, 9, F18, 2, sl(WEATHER));
    line(32, 152, 608, 152, 2, 5);
}

unsafe fn footer() {
    let p = page();
    label(32, 446, 300, 24, 8, F18, 0, PAGE_NAMES[p]);
    for i in 0..5 {
        let cx = 514 + i as i32 * 22;
        if i == p { fill_circle(cx, 458, 5, 15); } else { stroke_circle(cx, 458, 5, 2, 6); }
    }
}

unsafe fn checkbox(x: i32, y: i32, done: bool) {
    if done {
        fill_round_rect(x, y, 20, 20, 4, 11);
        line(x + 4, y + 10, x + 9, y + 15, 3, 0);
        line(x + 9, y + 15, x + 17, y + 5, 3, 0);
    } else {
        stroke_round_rect(x, y, 20, 20, 4, 2, 11);
    }
}

unsafe fn pending_count() -> usize { (0..TASK_N).filter(|i| !TASKS[*i].done).count() }

unsafe fn todo_header(y: i32) {
    let mut h = [0u8; 24]; let mut k = 0;
    push(&mut h, &mut k, b"TO DO  ");
    push_num(&mut h, &mut k, pending_count() as i32, false);
    push(&mut h, &mut k, b" left");
    label(32, y, 400, 24, 9, F18, 0, &h[..k]);
}

unsafe fn overview() {
    label(32, 162, 300, 24, 9, F18, 0, b"NEXT UP");
    let i = first_upcoming();
    if i < EV_N {
        let e = EVS[i];
        label(32, 186, 576, 42, 15, F32, 0, sl(e.title));
        let mut m = [0u8; 96]; let mut k = 0;
        let mut c = [0u8; 16]; let cn = clock(e.start, &mut c);
        push(&mut m, &mut k, &c[..cn]);
        if now_min() >= 0 {
            push(&mut m, &mut k, b"  |  ");
            let mut d = [0u8; 16];
            let dn = if e.start <= now_min() { push(&mut d, &mut 0, b"now"); 3 } else { countdown(e.start - now_min(), &mut d) };
            push(&mut m, &mut k, &d[..dn]);
        }
        if e.tag.n > 0 { push(&mut m, &mut k, b"  |  "); push(&mut m, &mut k, sl(e.tag)); }
        label(32, 230, 576, 30, 12, F24, 0, &m[..k]);
    } else {
        label(32, 190, 576, 34, 10, F24, 0, b"Nothing else scheduled");
    }
    line(32, 268, 608, 268, 1, 4);
    todo_header(276);
    if MAIL_N > 0 {
        let mut h = [0u8; 24]; let mut k = 0;
        push(&mut h, &mut k, b"INBOX  ");
        push_num(&mut h, &mut k, unread_count() as i32, false);
        push(&mut h, &mut k, b" unread");
        label(330, 276, 278, 24, 9, F18, 2, &h[..k]);
    }
    let mut shown = 0;
    for t in 0..TASK_N {
        if TASKS[t].done || shown >= 3 { continue; }
        let y = 304 + shown as i32 * 36;
        checkbox(32, y + 5, false);
        label(64, y, 544, 30, 14, F24, 0, sl(TASKS[t].text));
        shown += 1;
    }
    if shown == 0 { label(32, 306, 576, 30, 9, F24, 0, b"All clear"); }
}

unsafe fn agenda() {
    label(32, 162, 300, 24, 9, F18, 0, b"AGENDA");
    let now = now_min();
    let mut row = 0;
    for i in 0..EV_N {
        let e = EVS[i];
        if row >= 6 || (!e.allday && now >= 0 && e.end <= now) || (e.allday && e.start >= 1440) { continue; }
        let y = 192 + row as i32 * 40;
        if e.allday {
            label(32, y + 4, 112, 26, 12, F18, 0, b"ALL DAY");
        } else {
            let mut c = [0u8; 16]; let cn = clock(e.start, &mut c);
            label(32, y, 112, 30, 12, F24, 0, &c[..cn]);
        }
        label(150, y, 330, 30, 15, F24, 0, sl(e.title));
        if now >= 0 && !e.allday {
            let mut d = [0u8; 16];
            let dn = if e.start <= now { push(&mut d, &mut 0, b"now"); 3 } else { countdown(e.start - now, &mut d) };
            label(484, y + 4, 124, 26, 10, F18, 2, &d[..dn]);
        }
        row += 1;
    }
    if row == 0 { label(32, 196, 576, 30, 10, F24, 0, b"Nothing else scheduled"); }
}

unsafe fn tasks() {
    todo_header(162);
    if TASK_N == 0 {
        label(32, 196, 576, 30, 10, F24, 0, b"No tasks. Add them on your phone.");
        content::cursor(0, 0, false);
        return;
    }
    let fi = FOCUS - 2;
    if fi < SCROLL { SCROLL = fi; }
    if fi >= SCROLL + TASK_ROWS { SCROLL = fi + 1 - TASK_ROWS; }
    if TASK_N > TASK_ROWS {
        let mut h = [0u8; 12]; let mut k = 0;
        push_num(&mut h, &mut k, fi as i32 + 1, false); push(&mut h, &mut k, b"/"); push_num(&mut h, &mut k, TASK_N as i32, false);
        label(450, 162, 158, 24, 8, F18, 2, &h[..k]);
    }
    for (row, i) in (SCROLL..TASK_N).take(TASK_ROWS).enumerate() {
        let t = TASKS[i];
        let y = 190 + row as i32 * 40;
        if i == fi { fill_round_rect(22, y, 596, 38, 10, 3); }
        checkbox(36, y + 9, t.done);
        label(70, y + 4, 520, 30, if t.done { 6 } else { 15 }, F24, 0, sl(t.text));
        if t.done {
            let w = width(sl(t.text), F24).min(520);
            line(70, y + 20, 70 + w, y + 20, 2, 6);
        }
        if i == fi { content::cursor(604, y + 19, true); }
    }
}

unsafe fn inbox() {
    let mut h = [0u8; 24]; let mut k = 0;
    push(&mut h, &mut k, b"INBOX  ");
    push_num(&mut h, &mut k, unread_count() as i32, false);
    push(&mut h, &mut k, b" unread");
    label(32, 162, 400, 24, 9, F18, 0, &h[..k]);
    if MAIL_N == 0 {
        label(32, 196, 576, 30, 10, F24, 0, b"No threads. Set up the feed on your phone.");
        content::cursor(0, 0, false);
        return;
    }
    let fi = FOCUS - mail_start();
    if fi < MSCROLL { MSCROLL = fi; }
    if fi >= MSCROLL + TASK_ROWS { MSCROLL = fi + 1 - TASK_ROWS; }
    if MAIL_N > TASK_ROWS {
        let mut c = [0u8; 12]; let mut k = 0;
        push_num(&mut c, &mut k, fi as i32 + 1, false); push(&mut c, &mut k, b"/"); push_num(&mut c, &mut k, MAIL_N as i32, false);
        label(450, 162, 158, 24, 8, F18, 2, &c[..k]);
    }
    for (row, i) in (MSCROLL..MAIL_N).take(TASK_ROWS).enumerate() {
        let m = MAILS[i];
        let y = 190 + row as i32 * 40;
        if i == fi { fill_round_rect(22, y, 596, 38, 10, 3); }
        if m.unread { fill_circle(40, y + 19, 5, 15); }
        let g = if m.unread { 15 } else { 9 };
        label(54, y + 8, 140, 24, g, F18, 0, sl(m.from));
        label(200, y + 4, 300, 30, g, F24, 0, sl(m.subj));
        label(506, y + 8, 82, 24, 8, F18, 2, sl(m.when));
        if i == fi { content::cursor(604, y + 19, true); }
    }
}

unsafe fn note() {
    let title = if NOTE_T.n > 0 { sl(NOTE_T) } else { b"NOTE" as &[u8] };
    label(32, 162, 576, 24, 9, F18, 0, title);
    if NOTE_B.n == 0 { label(32, 196, 576, 30, 10, F24, 0, b"No note pinned"); return; }
    wrap(32, 192, 576, 34, 7, 15, F24, sl(NOTE_B));
}

unsafe fn render() {
    DIRTY = false;
    begin();
    header();
    if !HAVE_DATA {
        label(32, 180, 576, 34, 15, F24, 0, b"Waiting for your phone");
        wrap(32, 222, 576, 28, 3, 10, F18,
            b"Open Glance Dashboard in the MIRA app to sync your agenda, tasks and weather.");
        content::cursor(0, 0, false);
    } else {
        let p = page();
        if p != 2 && p != 3 { content::cursor(0, 0, false); }
        match p { 0 => overview(), 1 => agenda(), 2 => tasks(), 3 => inbox(), _ => note() }
    }
    footer();
    present();
}

unsafe fn sample_status() {
    let mut s = [0u8; 10];
    if device_status(s.as_mut_ptr(), 10) == 10 {
        let changed = !STATUS_OK || s[4] != STATUS[4] || s[5] != STATUS[5] || s[6] != STATUS[6] || s[8] != STATUS[8];
        STATUS = s; STATUS_OK = true;
        if changed { DIRTY = true; }
    }
}

// ---------- lifecycle ----------
#[no_mangle]
pub unsafe extern "C" fn app_init() {
    BRIDGE = companion::Companion::new(NAME);
    EV_N = 0; TASK_N = 0; MAIL_N = 0; WEATHER = NONE; NOTE_T = NONE; NOTE_B = NONE; H12 = false;
    HAVE_DATA = false; FOCUS = 0; SCROLL = 0; MSCROLL = 0; QUEUE_N = 0; WANT_SYNC = true;
    STATUS_OK = false; CLOCK_MS = 0;
    sample_status();
    render();
}

#[no_mangle]
pub unsafe extern "C" fn app_render(ms: i32) {
    let ms = ms.max(0) as u32;
    if content::context_changed() { DIRTY = true; }
    if let Some(message) = BRIDGE.tick(ms) { apply(message); }
    flush();
    CLOCK_MS = CLOCK_MS.saturating_add(ms);
    if CLOCK_MS >= 1000 { CLOCK_MS = 0; sample_status(); }
    if DIRTY { render(); }
}

#[no_mangle]
pub unsafe extern "C" fn app_event(e: i32) {
    if !HAVE_DATA { if e == 0 { WANT_SYNC = true; } return; }
    let n = stops();
    match e {
        1 => FOCUS = if FOCUS == 0 { n - 1 } else { FOCUS - 1 },
        2 => FOCUS = (FOCUS + 1) % n,
        0 => {
            if page() == 2 && TASK_N > 0 {
                let i = FOCUS - 2;
                TASKS[i].done = !TASKS[i].done;
                send_set(i);
            } else {
                WANT_SYNC = true;
            }
        }
        _ => return,
    }
    render();
}

// The host releases this instance and its owned resources after this callback.
#[no_mangle]
pub extern "C" fn app_stop() {}

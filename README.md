# Glance Dashboard for MIRA

A G2-style heads-up dashboard for MIRA glasses. The clock, date and weather sit at the top,
and you swipe through five panels below:

- **Overview:** your next event with a countdown, your top tasks and your unread count
- **Agenda:** today and tomorrow, with all-day events first and "now" for events in progress
- **To Do:** tap a task on the glasses to check it off; it syncs back to your phone
- **Inbox:** your 10 newest Gmail threads, with unread ones marked by a dot (optional)
- **Note:** a short note you pin from your phone

![Lens preview](preview.png)

*The preview shows simulated frames with sample data. Real text rendering on the glasses may differ slightly.*

Community project. This is not official MIRA software.

## Install

1. Download `GlanceDashboard.html` from the release.
2. In the MIRA iPhone app, go to **Apps > Build a Mira App > Create**. Upload the file, then
   tap **Check code > Add > Install > Open**.
3. Open the app's phone screen to add tasks and events, set your weather city, and choose a
   12- or 24-hour clock.

To update, long-press the app card and tap **Edit App**. Upload the new file, then tap
**Save > Update > Open**.

## Controls

| On the glasses        | What it does                                         |
|-----------------------|------------------------------------------------------|
| Swipe                 | Next or previous panel (on To Do and Inbox, moves one item) |
| Tap on a task         | Check or uncheck it (syncs to the phone)             |
| Tap anywhere else     | Refresh from the phone                               |
| Double tap / look up  | MIRA's sidebar and dashboard (unchanged)             |

## Optional: live Google Calendar + Gmail

Without this, Agenda shows only the events you type in on the phone, and Inbox stays empty.
With it, a small Google Apps Script running in **your own** Google account serves your
calendar and newest inbox threads. Your data goes straight from Google to your phone,
and no third-party server is involved.

1. Go to https://script.google.com, create a **New project**, and paste in `GlanceFeed.gs`.
2. Set `KEY` to a long random string (20+ characters). You can also change `MAIL_QUERY`
   (for example `in:inbox is:important`) or `CALENDARS`.
3. Choose **testFeed** and click **Run**. Approve the permissions, then check the log.
4. Go to **Deploy > New deployment > Web app**. Set *Execute as: Me* and *Who has access: Anyone*,
   then copy the URL. Requests without your key get nothing back.
5. Put `https://script.google.com/macros/s/.../exec?key=YOUR_KEY` on one line in
   `GlanceDashboard/feed-url.txt`, then rebuild from source (below).

**Keep the build you make with your feed private.** That HTML file contains your URL and key,
which give read access to your calendar and email subjects. Never share `feed-url.txt`.

## Build from source

You need Rust, Python 3 and Node.js. Get the MIRA SDK from https://github.com/big-halo/mira-sdk,
then put this `GlanceDashboard` folder at the root of the SDK, next to `Shared/` and
`CompanionExample/`:

    rustup target add wasm32-unknown-unknown
    python3 GlanceDashboard/package.py GlanceDashboard.html
    python3 CompanionExample/validate.py GlanceDashboard.html

The build prints `Live feed: on` or `Live feed: off`. Install the file only if validation prints PASS.

| File             | What it is                                            |
|------------------|-------------------------------------------------------|
| `dashboard.rs`   | Glasses side (Rust compiled to WASM)                  |
| `companion.html` | Phone screen: agenda, tasks, note, weather and settings |
| `package.py`     | Build script using MIRA's documented compiler flags   |
| `GlanceFeed.gs`  | Optional Google Apps Script for live calendar and inbox |

## Privacy

- Tasks, events, notes and settings stay in the app's MIRA companion storage on your phone.
- Weather comes from Open-Meteo (no key needed). Only your city's coordinates are sent.
- Calendar and email data are fetched only if you set up your own feed, and are kept in
  memory rather than saved.
- The glasses receive a compact snapshot over Bluetooth through the MIRA app.

## Known limits

- MIRA doesn't let apps replace its built-in look-up dashboard, so this runs as an app you open.
- Inbox is read-only.
- Up to 10 events, 12 tasks and 10 threads are shown. The phone trims the note if needed to
  fit MIRA's message size limit.

## License

The files in this repository are released under the MIT License (see LICENSE).
The MIRA SDK files you build with (`Shared/`, `CompanionExample/`, `SDK-ABI.json`)
are not part of this repository and remain under MIRA's own terms.

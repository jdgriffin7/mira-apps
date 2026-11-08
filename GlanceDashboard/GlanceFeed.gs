/**
 * Glance Dashboard feed: serves today's calendar and your top inbox threads
 * as JSON for the MIRA phone screen. Runs inside your own Google account.
 *
 * Setup: script.google.com > New project > paste this file > set KEY below >
 * Deploy > New deployment > Web app > Execute as: Me, Who has access: Anyone >
 * Deploy, approve access, copy the Web app URL.
 *
 * "Anyone" only means Google doesn't ask the caller to sign in; requests
 * without the right ?key= get nothing. Treat the URL plus key like a password.
 */
const KEY = 'CHANGE-ME-to-a-long-random-string';

// Which threads count as "top": any Gmail search works, e.g.
// 'in:inbox is:important', 'in:inbox is:unread', 'in:inbox category:primary'.
const MAIL_QUERY = 'in:inbox category:primary';
const MAIL_COUNT = 10;

// 'selected' = every calendar checked in Google Calendar's sidebar,
// or a list such as ['primary', 'team@group.calendar.google.com'].
const CALENDARS = 'selected';

function doGet(e) {
  if (KEY.indexOf('CHANGE-ME') === 0 || KEY.length < 20) return json({ error: 'set a KEY of 20+ characters first' });
  if (!e || !e.parameter || e.parameter.key !== KEY) return json({ error: 'unauthorized' });
  const out = { generated: new Date().toISOString(), events: [], threads: [], errors: [] };
  try { out.events = events(); } catch (err) { out.errors.push('calendar: ' + err.message); }
  try { out.threads = threads(); } catch (err) { out.errors.push('gmail: ' + err.message); }
  return json(out);
}

function json(obj) {
  return ContentService.createTextOutput(JSON.stringify(obj)).setMimeType(ContentService.MimeType.JSON);
}

function calendars() {
  if (Array.isArray(CALENDARS)) {
    return CALENDARS.map(function (id) {
      return id === 'primary' ? CalendarApp.getDefaultCalendar() : CalendarApp.getCalendarById(id);
    }).filter(Boolean);
  }
  return CalendarApp.getAllCalendars().filter(function (c) { return c.isSelected() && !c.isHidden(); });
}

// Today and tomorrow, skipping invitations you declined.
function events() {
  const tz = Session.getScriptTimeZone();
  const start = new Date(); start.setHours(0, 0, 0, 0);
  const end = new Date(start.getTime() + 2 * 864e5);
  const out = [];
  calendars().forEach(function (cal) {
    cal.getEvents(start, end).forEach(function (ev) {
      if (ev.getMyStatus() === CalendarApp.GuestStatus.NO) return;
      const allDay = ev.isAllDayEvent();
      out.push({
        title: ev.getTitle() || '(no title)',
        calendar: cal.getName(),
        allDay: allDay,
        start: allDay ? Utilities.formatDate(ev.getAllDayStartDate(), tz, 'yyyy-MM-dd') : ev.getStartTime().toISOString(),
        end: allDay ? Utilities.formatDate(ev.getAllDayEndDate(), tz, 'yyyy-MM-dd') : ev.getEndTime().toISOString()
      });
    });
  });
  return out;
}

function threads() {
  return GmailApp.search(MAIL_QUERY, 0, MAIL_COUNT).map(function (t) {
    const msgs = t.getMessages();
    const last = msgs[msgs.length - 1];
    return {
      from: senderName(last.getFrom()),
      subject: t.getFirstMessageSubject() || '(no subject)',
      unread: t.isUnread(),
      count: t.getMessageCount(),
      date: t.getLastMessageDate().toISOString()
    };
  });
}

function senderName(from) {
  const m = String(from).match(/^\s*"?([^"<]*?)"?\s*<([^>]+)>\s*$/);
  if (!m) return String(from).trim();
  return (m[1] || m[2].split('@')[0]).trim();
}

// Run this once from the editor to check output and grant permissions.
function testFeed() {
  Logger.log(JSON.stringify({ events: events(), threads: threads() }, null, 2));
}

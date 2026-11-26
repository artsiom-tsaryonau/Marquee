//! Play-history totals. A session counts in the local week that contains its start.
//! Failed and still-running sessions are not credited.

use crate::model::{Session, SessionState};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GameHistory {
    pub sessions: Vec<Session>,
    pub week_seconds: i64,
    #[serde(default)]
    pub stream_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaySummary {
    pub week_seconds: i64,
}

pub fn for_edition(
    sessions: &[Session],
    edition_id: &str,
    now_unix: i64,
    offset_secs: i64,
) -> GameHistory {
    let start = week_start(now_unix, offset_secs);
    let week_seconds = credited(sessions, Some(edition_id), start);
    let stream_seconds = sessions
        .iter()
        .filter(|session| session.edition_id == edition_id)
        .filter(|session| session.mode == "stream")
        .filter(|session| matches!(session.state, SessionState::Ended))
        .map(|session| session.duration_seconds.unwrap_or(0).max(0))
        .sum();
    let sessions = sessions
        .iter()
        .filter(|session| session.edition_id == edition_id)
        .take(50)
        .cloned()
        .collect();
    GameHistory {
        sessions,
        week_seconds,
        stream_seconds,
    }
}

pub fn summary(sessions: &[Session], now_unix: i64, offset_secs: i64) -> PlaySummary {
    PlaySummary {
        week_seconds: credited(sessions, None, week_start(now_unix, offset_secs)),
    }
}

fn credited(sessions: &[Session], edition_id: Option<&str>, week_start: i64) -> i64 {
    sessions
        .iter()
        .filter(|session| edition_id.is_none_or(|id| session.edition_id == id))
        .filter(|session| matches!(session.state, SessionState::Ended))
        .filter(|session| unix_of(&session.started_at).is_some_and(|started| started >= week_start))
        .map(|session| session.duration_seconds.unwrap_or(0).max(0))
        .sum()
}

/// Monday 00:00 in the zone `offset_secs` ahead of UTC, expressed as a UTC unix time.
pub fn week_start(now_unix: i64, offset_secs: i64) -> i64 {
    let local = now_unix + offset_secs;
    let days = local.div_euclid(86_400);
    // 1970-01-01 was a Thursday, three days after Monday.
    let since_monday = (days + 3).rem_euclid(7);
    (days - since_monday) * 86_400 - offset_secs
}

pub fn local_offset_secs(now_unix: i64) -> i64 {
    unsafe {
        let instant = now_unix as libc::time_t;
        let local = libc::localtime(&instant);
        if local.is_null() {
            0
        } else {
            (*local).tm_gmtoff
        }
    }
}

pub(crate) fn counts_this_week(started_at: &str, now_unix: i64, offset_secs: i64) -> bool {
    unix_of(started_at).is_some_and(|started| started >= week_start(now_unix, offset_secs))
}

pub(crate) fn unix_of(stamp: &str) -> Option<i64> {
    let stamp = stamp.strip_suffix('Z')?;
    let (date, time) = stamp.split_once('T')?;
    let mut date = date.split('-');
    let year: i32 = date.next()?.parse().ok()?;
    let month: u32 = date.next()?.parse().ok()?;
    let day: u32 = date.next()?.parse().ok()?;
    let mut time = time.split(':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: i64 = time.next()?.parse().ok()?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = (year - era * 400) as u32;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097 + day_of_era as i32 - 719_468) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(edition: &str, started: &str, seconds: i64, state: SessionState) -> Session {
        Session {
            id: started.into(),
            edition_id: edition.into(),
            runner_id: "runner".into(),
            started_at: started.into(),
            ended_at: None,
            duration_seconds: Some(seconds),
            exit_code: Some(0),
            state,
            error: None,
            port_id: None,
            mode: "local".into(),
        }
    }

    #[test]
    fn a_local_week_starts_on_monday_and_ignores_failed_sessions() {
        // 2026-11-25 12:00 UTC is a Wednesday. With a +2h offset the local week
        // still started on Monday 2026-11-23 00:00 local, which is 22:00 UTC on Sunday.
        let now = unix_of("2026-11-25T12:00:00Z").unwrap();
        let start = week_start(now, 2 * 3600);
        assert_eq!(start, unix_of("2026-11-22T22:00:00Z").unwrap());
        let sessions = vec![
            session("a", "2026-11-22T21:59:59Z", 3600, SessionState::Ended),
            session("a", "2026-11-22T22:30:00Z", 120, SessionState::Ended),
            session("a", "2026-11-24T10:00:00Z", 50, SessionState::Failed),
            session("b", "2026-11-25T01:00:00Z", 30, SessionState::Ended),
            session("a", "2026-11-25T08:00:00Z", 10, SessionState::Running),
        ];
        let history = for_edition(&sessions, "a", now, 2 * 3600);
        assert_eq!(history.week_seconds, 120);
        assert_eq!(history.sessions.len(), 4);
        assert_eq!(history.stream_seconds, 0);
        assert_eq!(summary(&sessions, now, 2 * 3600).week_seconds, 150);
        let mut streamed = session("a", "2026-11-24T12:00:00Z", 40, SessionState::Ended);
        streamed.mode = "stream".into();
        let mut mixed = sessions;
        mixed.push(streamed);
        assert_eq!(for_edition(&mixed, "a", now, 2 * 3600).stream_seconds, 40);
    }
}

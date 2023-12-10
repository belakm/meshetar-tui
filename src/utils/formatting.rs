use chrono::{DateTime, Duration, Local, LocalResult, NaiveDateTime, TimeZone, Utc};

const DATETIME_FORMAT_SHAPE: &str = "%e. %b %H:%M";
const DATETIME_FORMAT_SHAPE_SHORT: &str = "%H:%M:%S";

pub fn current_timestamp() -> String {
    Local::now().format(&DATETIME_FORMAT_SHAPE).to_string()
}

pub fn timestamp_to_string(millis: i64) -> String {
    match Utc.timestamp_millis_opt(millis) {
        LocalResult::Single(dt) => dt.format(&DATETIME_FORMAT_SHAPE).to_string(),
        _ => String::from("Incorrect timestamp millis"),
    }
}

pub fn timestamp_to_dt(timestamp: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp_millis(timestamp).unwrap(),
        Utc,
    )
}

pub fn dt_to_readable(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Utc)
        .format(&DATETIME_FORMAT_SHAPE)
        .to_string()
}

pub fn dt_to_readable_short(dt: DateTime<Utc>) -> String {
    dt.format(&DATETIME_FORMAT_SHAPE_SHORT).to_string()
}

pub fn readable_duration(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let duration = end.signed_duration_since(start);
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;
    format!("{}d {}h {}m", days, hours, minutes)
}

//! The weather ear: conditions now and the week ahead for the reader's place.
//!
//! Straight from Open-Meteo (free, no key, no account) plus the National
//! Weather Service's active alerts for places in the United States (also free,
//! also no key). A forecast is a fact, not an editorial decision, so Claude
//! stays out of the widget. But when the weather is out of the ordinary - an
//! official warning, a heat wave, a big swing, a storm - it becomes a story:
//! the facts below are handed to the editor as wire copy that must run.

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Settings, WireItem};
use crate::wire;

// -------------------------------------------------------------------- types

/// A place the geocoder found, or the one the reader picked.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// "Boise, Idaho, United States"
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// ISO 3166 alpha-2, lower case ("us"). Empty if unknown.
    #[serde(default)]
    pub country: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Current {
    pub temp: i32,
    pub feels_like: i32,
    pub humidity: u32,
    pub wind_mph: u32,
    pub gusts_mph: u32,
    pub code: u32,
    pub text: String,
    pub is_day: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    /// YYYY-MM-DD, local to the place.
    pub date: String,
    /// "Sun"
    pub weekday: String,
    pub high: i32,
    pub low: i32,
    pub code: u32,
    pub text: String,
    /// Chance of precipitation, percent.
    pub rain_chance: u32,
    pub precip_in: f64,
    pub snow_in: f64,
    pub gusts_mph: u32,
    pub uv: f64,
    /// "6:48 AM"
    pub sunrise: String,
    pub sunset: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    /// "Heat Advisory"
    pub event: String,
    pub headline: String,
    /// "Extreme" | "Severe" | "Moderate"
    pub severity: String,
    pub description: String,
    pub expires: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub place: String,
    pub lat: f64,
    pub lon: f64,
    /// RFC 3339, when this was fetched.
    pub updated: String,
    pub current: Current,
    pub today: Day,
    pub yesterday: Option<Day>,
    /// Today and the six days after it.
    pub days: Vec<Day>,
    pub alerts: Vec<Alert>,
    /// Why today is out of the ordinary. Empty on a routine day.
    pub unusual: Vec<String>,
    /// The forecast page for the reader window.
    pub url: String,
    pub source: String,
}

// ------------------------------------------------------------------ helpers

/// WMO weather interpretation codes, as Open-Meteo reports them.
pub fn describe(code: u32, is_day: bool) -> &'static str {
    match code {
        0 => {
            if is_day {
                "Sunny"
            } else {
                "Clear"
            }
        }
        1 => {
            if is_day {
                "Mostly sunny"
            } else {
                "Mostly clear"
            }
        }
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 => "Drizzle",
        55 => "Heavy drizzle",
        56 | 57 => "Freezing drizzle",
        61 => "Light rain",
        63 => "Rain",
        65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71 => "Light snow",
        73 => "Snow",
        75 => "Heavy snow",
        77 => "Snow grains",
        80 => "Light showers",
        81 => "Showers",
        82 => "Heavy showers",
        85 => "Snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorms",
        96 | 99 => "Thunderstorms with hail",
        _ => "Unsettled",
    }
}

fn f64_at(v: &Value, key: &str, i: usize) -> Option<f64> {
    v.get(key)?.as_array()?.get(i)?.as_f64()
}

fn str_at(v: &Value, key: &str, i: usize) -> Option<String> {
    v.get(key)?.as_array()?.get(i)?.as_str().map(|s| s.to_string())
}

fn round(x: Option<f64>) -> i32 {
    x.map(|f| f.round() as i32).unwrap_or(0)
}

fn round_u(x: Option<f64>) -> u32 {
    x.map(|f| f.round().max(0.0) as u32).unwrap_or(0)
}

/// "2026-09-27T06:48" -> "6:48 AM"
fn clock(iso: &str) -> String {
    NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M")
        .map(|t| t.format("%-I:%M %p").to_string())
        .unwrap_or_default()
}

fn weekday(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|d| d.format("%a").to_string()).unwrap_or_default()
}

fn in_us(lat: f64, lon: f64) -> bool {
    (17.0..=72.0).contains(&lat) && (-180.0..=-64.0).contains(&lon)
}

fn forecast_url(place: &str, lat: f64, lon: f64) -> String {
    if in_us(lat, lon) {
        format!("https://forecast.weather.gov/MapClick.php?lat={lat:.4}&lon={lon:.4}")
    } else {
        let _ = place;
        format!("https://www.windy.com/{lat:.3}/{lon:.3}?{lat:.3},{lon:.3},9")
    }
}

// ------------------------------------------------------------------ fetching

/// Places matching what the reader typed, best first.
pub async fn geocode(client: &reqwest::Client, query: &str) -> Result<Vec<Place>, String> {
    let q = query.trim();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=6&language=en&format=json",
        urlencoding(q)
    );
    let resp = client.get(&url).send().await.map_err(|e| format!("weather: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("weather: geocoder HTTP {}", resp.status().as_u16()));
    }
    let body = resp.text().await.map_err(|e| format!("weather: {e}"))?;
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("weather: {e}"))?;
    Ok(places_from_json(&v))
}

pub fn places_from_json(v: &Value) -> Vec<Place> {
    let s = |r: &Value, k: &str| r.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    v.get("results")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let name = s(r, "name")?;
            let lat = r.get("latitude")?.as_f64()?;
            let lon = r.get("longitude")?.as_f64()?;
            let mut parts = vec![name];
            if let Some(a) = s(r, "admin1") {
                if !parts.contains(&a) {
                    parts.push(a);
                }
            }
            if let Some(c) = s(r, "country") {
                parts.push(c);
            }
            Some(Place { name: parts.join(", "), lat, lon, country: s(r, "country_code").unwrap_or_default().to_lowercase() })
        })
        .collect()
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Where the weather is for: the place the reader picked, else the dateline
/// city looked up on the fly, else nowhere.
pub async fn resolve(client: &reqwest::Client, settings: &Settings) -> Option<Place> {
    if settings.weather_lat != 0.0 || settings.weather_lon != 0.0 {
        return Some(Place {
            name: if settings.weather_place.trim().is_empty() { "Home".into() } else { settings.weather_place.trim().to_string() },
            lat: settings.weather_lat,
            lon: settings.weather_lon,
            country: String::new(),
        });
    }
    let city = settings.city.trim();
    if city.is_empty() {
        return None;
    }
    geocode(client, city).await.ok()?.into_iter().next()
}

/// The full report for the reader's place, or None when no place is set.
pub async fn report(client: &reqwest::Client, settings: &Settings) -> Result<Option<Report>, String> {
    let Some(place) = resolve(client, settings).await else {
        return Ok(None);
    };
    fetch(client, &place).await.map(Some)
}

pub async fn fetch(client: &reqwest::Client, place: &Place) -> Result<Report, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &current=temperature_2m,relative_humidity_2m,apparent_temperature,is_day,weather_code,wind_speed_10m,wind_gusts_10m\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,precipitation_sum,snowfall_sum,precipitation_probability_max,wind_gusts_10m_max\
         &temperature_unit=fahrenheit&wind_speed_unit=mph&precipitation_unit=inch&timezone=auto&past_days=1&forecast_days=7",
        lat = place.lat,
        lon = place.lon
    );
    let forecast = async {
        let resp = client.get(&url).send().await.map_err(|e| format!("weather: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("weather: HTTP {}", resp.status().as_u16()));
        }
        let body = resp.text().await.map_err(|e| format!("weather: {e}"))?;
        serde_json::from_str::<Value>(&body).map_err(|e| format!("weather: {e}"))
    };
    let alerts = async {
        if !in_us(place.lat, place.lon) {
            return Vec::new();
        }
        let url = format!("https://api.weather.gov/alerts/active?point={:.4},{:.4}", place.lat, place.lon);
        let Ok(resp) = client.get(&url).header("Accept", "application/geo+json").send().await else { return Vec::new() };
        let Ok(body) = resp.text().await else { return Vec::new() };
        serde_json::from_str::<Value>(&body).map(|v| alerts_from_json(&v)).unwrap_or_default()
    };
    let (forecast, alerts) = tokio::join!(forecast, alerts);
    let mut report = report_from_json(&forecast?, place)?;
    report.alerts = alerts;
    report.unusual = judge(&report);
    report.source = if report.alerts.is_empty() { "Open-Meteo".into() } else { "National Weather Service".into() };
    Ok(report)
}

pub fn alerts_from_json(v: &Value) -> Vec<Alert> {
    let s = |p: &Value, k: &str| p.get(k).and_then(|x| x.as_str()).map(|x| wire::clean_text(x)).unwrap_or_default();
    let mut out: Vec<Alert> = v
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| f.get("properties"))
        .filter(|p| s(p, "status") == "Actual")
        .filter(|p| matches!(s(p, "severity").as_str(), "Extreme" | "Severe" | "Moderate"))
        .filter(|p| !s(p, "event").to_lowercase().contains("test"))
        .map(|p| Alert {
            event: s(p, "event"),
            headline: wire::truncate(&s(p, "headline"), 200),
            severity: s(p, "severity"),
            description: wire::truncate(&s(p, "description"), 900),
            expires: p.get("expires").and_then(|x| x.as_str()).map(|x| x.to_string()),
        })
        .collect();
    // Worst first; one entry per event type.
    let rank = |sev: &str| match sev {
        "Extreme" => 0,
        "Severe" => 1,
        _ => 2,
    };
    out.sort_by_key(|a| rank(&a.severity));
    out.dedup_by(|a, b| a.event == b.event);
    out.truncate(4);
    out
}

pub fn report_from_json(v: &Value, place: &Place) -> Result<Report, String> {
    let daily = v.get("daily").ok_or("weather: no daily block")?;
    let n = daily.get("time").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0);
    if n < 2 {
        return Err("weather: forecast came back empty".into());
    }
    let day = |i: usize| -> Day {
        let date = str_at(daily, "time", i).unwrap_or_default();
        let code = round_u(f64_at(daily, "weather_code", i));
        Day {
            weekday: weekday(&date),
            date,
            high: round(f64_at(daily, "temperature_2m_max", i)),
            low: round(f64_at(daily, "temperature_2m_min", i)),
            code,
            text: describe(code, true).to_string(),
            rain_chance: round_u(f64_at(daily, "precipitation_probability_max", i)),
            precip_in: f64_at(daily, "precipitation_sum", i).unwrap_or(0.0),
            snow_in: f64_at(daily, "snowfall_sum", i).unwrap_or(0.0),
            gusts_mph: round_u(f64_at(daily, "wind_gusts_10m_max", i)),
            uv: f64_at(daily, "uv_index_max", i).unwrap_or(0.0),
            sunrise: clock(&str_at(daily, "sunrise", i).unwrap_or_default()),
            sunset: clock(&str_at(daily, "sunset", i).unwrap_or_default()),
        }
    };
    // past_days=1: index 0 is yesterday, 1 is today.
    let yesterday = day(0);
    let days: Vec<Day> = (1..n).map(day).collect();
    let today = days[0].clone();

    let cur = v.get("current").cloned().unwrap_or(Value::Null);
    let g = |k: &str| cur.get(k).and_then(|x| x.as_f64());
    let is_day = g("is_day").map(|d| d >= 0.5).unwrap_or(true);
    let code = round_u(g("weather_code"));
    let current = Current {
        temp: round(g("temperature_2m")),
        feels_like: round(g("apparent_temperature")),
        humidity: round_u(g("relative_humidity_2m")),
        wind_mph: round_u(g("wind_speed_10m")),
        gusts_mph: round_u(g("wind_gusts_10m")),
        code,
        text: describe(code, is_day).to_string(),
        is_day,
    };

    Ok(Report {
        place: place.name.clone(),
        lat: place.lat,
        lon: place.lon,
        updated: chrono::Utc::now().to_rfc3339(),
        current,
        today,
        yesterday: Some(yesterday),
        days,
        alerts: Vec::new(),
        unusual: Vec::new(),
        url: forecast_url(&place.name, place.lat, place.lon),
        source: "Open-Meteo".into(),
    })
}

// ------------------------------------------------------------- the judgment

/// What, if anything, makes today's weather news. Plain thresholds, no AI:
/// an official warning, real heat or cold, a big swing from yesterday, a lot
/// of rain or snow, thunder, strong wind, extreme sun.
pub fn judge(r: &Report) -> Vec<String> {
    let mut why: Vec<String> = Vec::new();
    for a in &r.alerts {
        why.push(format!("{} in force ({})", a.event, a.severity.to_lowercase()));
    }
    let t = &r.today;
    if t.high >= 100 {
        why.push(format!("a high of {}°F", t.high));
    }
    if t.low <= 10 {
        why.push(format!("a low of {}°F", t.low));
    }
    if let Some(y) = &r.yesterday {
        let swing = t.high - y.high;
        if swing.abs() >= 20 {
            why.push(format!(
                "a {}° {} from yesterday's high of {}°F",
                swing.abs(),
                if swing < 0 { "drop" } else { "jump" },
                y.high
            ));
        }
        if t.low <= 32 && y.low > 40 {
            why.push(format!("a freeze tonight ({}°F) after {}°F last night", t.low, y.low));
        }
    }
    if t.snow_in >= 2.0 {
        why.push(format!("{:.0} in of snow", t.snow_in));
    } else if t.precip_in >= 1.0 {
        why.push(format!("{:.1} in of rain", t.precip_in));
    }
    if matches!(t.code, 95 | 96 | 99) {
        why.push("thunderstorms".into());
    } else if matches!(t.code, 65 | 67 | 75 | 82 | 86) && t.precip_in < 1.0 && t.snow_in < 2.0 {
        why.push(t.text.to_lowercase());
    }
    if t.gusts_mph >= 45 {
        why.push(format!("gusts to {} mph", t.gusts_mph));
    }
    if t.uv >= 11.0 {
        why.push(format!("an extreme UV index of {:.0}", t.uv));
    }
    why
}

/// "Sunny · High 88° Low 63°" - one line for a printed ear.
pub fn ear_line(r: &Report) -> String {
    let mut s = format!("{} · High {}° Low {}°", r.today.text, r.today.high, r.today.low);
    if r.today.rain_chance >= 20 {
        s.push_str(&format!(" · Rain {}%", r.today.rain_chance));
    }
    if let Some(a) = r.alerts.first() {
        s = format!("{} · {s}", a.event);
    }
    s
}

/// The wire item the editor gets on an unusual day.
pub fn wire_item(r: &Report) -> WireItem {
    WireItem {
        kind: "article".into(),
        title: format!("Weather in {}: {}", short_place(&r.place), join_reasons(&r.unusual)),
        url: r.url.clone(),
        source: r.source.clone(),
        author: None,
        published: Some(r.updated.clone()),
        summary: Some(wire::truncate(&facts(r), 1600)),
        image: None,
        beat: "Weather".into(),
    }
}

fn short_place(p: &str) -> String {
    p.split(',').next().unwrap_or(p).trim().to_string()
}

fn join_reasons(why: &[String]) -> String {
    match why {
        [] => String::new(),
        [one] => one.clone(),
        [head @ .., last] => format!("{} and {}", head.join(", "), last),
    }
}

/// Everything the editor may use, as plain text. Numbers come from here and
/// nowhere else.
pub fn facts(r: &Report) -> String {
    let mut s = String::new();
    s.push_str(&format!("Place: {} ({:.3}, {:.3}). Data as of {}.\n", r.place, r.lat, r.lon, r.updated));
    s.push_str(&format!("Why it is news: {}.\n", join_reasons(&r.unusual)));
    for a in &r.alerts {
        s.push_str(&format!("ALERT ({}): {}. {}", a.severity, a.event, a.headline));
        if let Some(e) = &a.expires {
            s.push_str(&format!(" Expires {e}."));
        }
        s.push_str(&format!("\n  {}\n", a.description));
    }
    let c = &r.current;
    s.push_str(&format!(
        "Now: {}°F ({}), feels like {}°F, humidity {}%, wind {} mph gusting {} mph.\n",
        c.temp, c.text, c.feels_like, c.humidity, c.wind_mph, c.gusts_mph
    ));
    let t = &r.today;
    s.push_str(&format!(
        "Today ({} {}): {}, high {}°F, low {}°F, chance of precipitation {}%, rain {:.2} in, snow {:.1} in, gusts to {} mph, UV index {:.0}, sunrise {}, sunset {}.\n",
        t.weekday, t.date, t.text, t.high, t.low, t.rain_chance, t.precip_in, t.snow_in, t.gusts_mph, t.uv, t.sunrise, t.sunset
    ));
    if let Some(y) = &r.yesterday {
        s.push_str(&format!("Yesterday: {}, high {}°F, low {}°F, rain {:.2} in.\n", y.text, y.high, y.low, y.precip_in));
    }
    s.push_str("Next days: ");
    for d in r.days.iter().skip(1).take(4) {
        s.push_str(&format!("{} {} {}/{}°F {}%; ", d.weekday, d.text, d.high, d.low, d.rain_chance));
    }
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(high: i32, low: i32, code: u32) -> Day {
        Day { high, low, code, text: describe(code, true).into(), ..Day::default() }
    }

    fn report(today: Day, yesterday: Option<Day>) -> Report {
        Report { today: today.clone(), yesterday, days: vec![today], ..Report::default() }
    }

    #[test]
    fn a_routine_day_is_not_news() {
        let r = report(day(78, 58, 1), Some(day(80, 59, 0)));
        assert!(judge(&r).is_empty());
    }

    #[test]
    fn heat_swings_storms_and_warnings_are_news() {
        let r = report(day(104, 78, 0), Some(day(101, 77, 0)));
        assert_eq!(judge(&r), vec!["a high of 104°F"]);

        let r = report(day(58, 40, 3), Some(day(82, 55, 0)));
        assert_eq!(judge(&r), vec!["a 24° drop from yesterday's high of 82°F"]);

        let mut stormy = day(70, 55, 95);
        stormy.precip_in = 1.4;
        stormy.gusts_mph = 50;
        let why = judge(&report(stormy, None));
        assert_eq!(why, vec!["1.4 in of rain", "thunderstorms", "gusts to 50 mph"]);

        let mut r = report(day(90, 70, 0), None);
        r.alerts.push(Alert { event: "Heat Advisory".into(), severity: "Moderate".into(), ..Alert::default() });
        assert_eq!(judge(&r), vec!["Heat Advisory in force (moderate)"]);
        assert!(ear_line(&r).starts_with("Heat Advisory · Sunny · High 90° Low 70°"));
    }

    #[test]
    fn parses_open_meteo_and_nws() {
        let v: Value = serde_json::from_str(
            r#"{"current":{"temperature_2m":71.3,"relative_humidity_2m":40,"apparent_temperature":70.1,"is_day":1,"weather_code":2,"wind_speed_10m":6.2,"wind_gusts_10m":11.0},
                "daily":{"time":["2026-09-26","2026-09-27","2026-09-28"],"weather_code":[0,2,61],"temperature_2m_max":[84.0,79.6,70.2],"temperature_2m_min":[60.1,58.9,55.0],
                "sunrise":["2026-09-26T06:47","2026-09-27T06:48","2026-09-28T06:49"],"sunset":["2026-09-26T18:50","2026-09-27T18:49","2026-09-28T18:47"],
                "uv_index_max":[7.1,6.8,4.0],"precipitation_sum":[0,0,0.31],"snowfall_sum":[0,0,0],"precipitation_probability_max":[0,5,60],"wind_gusts_10m_max":[14,18,25]}}"#,
        )
        .unwrap();
        let place = Place { name: "Boise, Idaho, United States".into(), lat: 43.6, lon: -116.2, country: "us".into() };
        let r = report_from_json(&v, &place).unwrap();
        assert_eq!(r.current.temp, 71);
        assert_eq!(r.current.text, "Partly cloudy");
        assert_eq!(r.today.date, "2026-09-27");
        assert_eq!(r.today.weekday, "Sun");
        assert_eq!(r.today.high, 80);
        assert_eq!(r.today.sunrise, "6:48 AM");
        assert_eq!(r.yesterday.as_ref().unwrap().high, 84);
        assert_eq!(r.days.len(), 2);
        assert!(r.url.contains("forecast.weather.gov"));
        assert_eq!(ear_line(&r), "Partly cloudy · High 80° Low 59°");

        let nws: Value = serde_json::from_str(
            r#"{"features":[
                {"properties":{"status":"Actual","severity":"Moderate","event":"Heat Advisory","headline":"Heat Advisory until 8 PM","description":"Hot.","expires":"2026-09-27T20:00:00-07:00"}},
                {"properties":{"status":"Actual","severity":"Minor","event":"Special Weather Statement","headline":"x","description":"y"}},
                {"properties":{"status":"Test","severity":"Extreme","event":"Tornado Warning","headline":"x","description":"y"}},
                {"properties":{"status":"Actual","severity":"Severe","event":"Red Flag Warning","headline":"Red Flag Warning","description":"Wind and low humidity."}}
            ]}"#,
        )
        .unwrap();
        let alerts = alerts_from_json(&nws);
        assert_eq!(alerts.iter().map(|a| a.event.as_str()).collect::<Vec<_>>(), vec!["Red Flag Warning", "Heat Advisory"]);

        let places = places_from_json(
            &serde_json::from_str(r#"{"results":[{"name":"Boise","latitude":43.61,"longitude":-116.2,"country":"United States","country_code":"US","admin1":"Idaho"}]}"#).unwrap(),
        );
        assert_eq!(places[0].name, "Boise, Idaho, United States");
        assert_eq!(places[0].country, "us");
    }

    #[test]
    fn the_wire_item_carries_the_facts() {
        let mut r = report(day(103, 80, 0), Some(day(99, 78, 0)));
        r.place = "Boise, Idaho, United States".into();
        r.url = "https://forecast.weather.gov/MapClick.php?lat=43.6&lon=-116.2".into();
        r.unusual = judge(&r);
        let w = wire_item(&r);
        assert_eq!(w.kind, "article");
        assert_eq!(w.beat, "Weather");
        assert_eq!(w.title, "Weather in Boise: a high of 103°F");
        assert!(w.summary.unwrap().contains("high 103°F"));
        assert_eq!(join_reasons(&["a".into(), "b".into(), "c".into()]), "a, b and c");
    }
}

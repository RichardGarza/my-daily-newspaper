// The weather ear, under the date in the masthead: conditions now, today's
// high and low, the chance of rain, and - on an out-of-the-ordinary day - the
// warning or the reason in red. Hover for the details, click for the full
// forecast in a reader window. Straight from Open-Meteo and the National
// Weather Service; Claude only gets involved when the day is unusual, and
// then it writes a story on the front page.
// Polls every 15 minutes and whenever the window regains focus.

import { useEffect, useState } from "react";
import type { WeatherReport } from "../types";
import { openReader, weather } from "../api";

/** A glyph per WMO code family. Text-presentation selectors keep them ink, not emoji. */
export function glyph(code: number, isDay: boolean): string {
  if (code <= 1) return isDay ? "☀︎" : "☽︎";
  if (code <= 3) return "☁︎";
  if (code === 45 || code === 48) return "≡";
  if (code >= 95) return "⚡︎";
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "❄︎";
  return "☂︎";
}

const cap = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);

/** "H 88° · L 63° · Rain 40%" - the day line under the current conditions. */
export function dayLine(r: WeatherReport): string {
  const t = r.today;
  const rain = t.rainChance >= 20 ? ` · Rain ${t.rainChance}%` : "";
  return `H ${t.high}° · L ${t.low}°${rain}`;
}

export function detailLines(r: WeatherReport): string[] {
  const c = r.current;
  const t = r.today;
  const lines = [
    r.place,
    `Feels like ${c.feelsLike}° · Humidity ${c.humidity}% · Wind ${c.windMph} mph${c.gustsMph > c.windMph + 5 ? `, gusts ${c.gustsMph}` : ""}`,
    `Sunrise ${t.sunrise} · Sunset ${t.sunset}${t.uv >= 6 ? ` · UV ${Math.round(t.uv)}` : ""}`,
  ];
  for (const d of r.days.slice(1, 4)) lines.push(`${d.weekday}: ${d.text}, ${d.high}°/${d.low}°${d.rainChance >= 20 ? `, rain ${d.rainChance}%` : ""}`);
  for (const a of r.alerts) lines.push(`⚠ ${a.headline || a.event}`);
  if (r.alerts.length === 0 && r.unusual.length > 0) lines.push(`Out of the ordinary: ${r.unusual.join(", ")}`);
  lines.push(`${r.source} · click for the full forecast`);
  return lines;
}

interface Props {
  /** Changes when the profile's city or weather place changes, to refetch. */
  placeKey: string;
  /** False when neither a city nor a weather place is set. */
  hasCity: boolean;
}

export default function Weather({ placeKey, hasCity }: Props) {
  const [data, setData] = useState<WeatherReport | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hasCity) return;
    let alive = true;
    const load = () =>
      weather()
        .then((r) => {
          if (!alive) return;
          setData(r);
          setError(null);
        })
        .catch((e) => alive && setError(String(e)));
    void load();
    const timer = window.setInterval(() => void load(), 15 * 60_000);
    const onFocus = () => void load();
    window.addEventListener("focus", onFocus);
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [placeKey, hasCity]);

  if (!hasCity) return <div className="weather muted">Weather: set a city under Edit Interests.</div>;
  if (error && !data) {
    return (
      <div className="weather muted" title={error}>
        Weather unavailable right now.
      </div>
    );
  }
  if (!data) return <div className="weather muted">{data === undefined ? "Checking the sky…" : "Weather: no place found for that city."}</div>;

  const flag = data.alerts[0]?.event ?? (data.unusual.length > 0 ? cap(data.unusual[0]) : null);
  return (
    <div className="weather">
      <button
        type="button"
        className="weather-btn"
        title={detailLines(data).join("\n")}
        onClick={() => void openReader(data.url, `Weather · ${data.place}`)}
      >
        <span className="wx-now">
          <span className="wx-glyph" aria-hidden="true">
            {glyph(data.current.code, data.current.isDay)}
          </span>
          <b>{data.current.temp}°</b> {data.current.text}
        </span>
        <span className="wx-day">{dayLine(data)}</span>
      </button>
      {flag && (
        <div className="wx-alert" title={data.unusual.join(", ")}>
          {"⚠︎"} {flag}
        </div>
      )}
    </div>
  );
}

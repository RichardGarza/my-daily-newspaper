// Name, city, weather place, ball team: shared by the welcome page and Edit Interests.

import { useEffect, useRef, useState } from "react";
import type { WeatherPlace } from "../types";
import { geocode } from "../api";
import { MLB_TEAMS } from "../teams";

export interface ProfileDraft {
  ownerName: string;
  city: string;
  /** Empty place and 0,0 = the dateline city, looked up on the fly. */
  weatherPlace: string;
  weatherLat: number;
  weatherLon: number;
  mlbTeamId: number;
}

interface Props {
  value: ProfileDraft;
  onChange: (next: ProfileDraft) => void;
  autoFocus?: boolean;
}

/** Type a place, pick one of the matches. Clearing the box goes back to the dateline city. */
function PlaceSearch({ value, onChange }: { value: ProfileDraft; onChange: (next: ProfileDraft) => void }) {
  const [query, setQuery] = useState(value.weatherPlace);
  const [results, setResults] = useState<WeatherPlace[]>([]);
  const [open, setOpen] = useState(false);
  const [searching, setSearching] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  // Saved elsewhere (a reset, another form): follow it.
  useEffect(() => setQuery(value.weatherPlace), [value.weatherPlace]);

  useEffect(() => {
    const q = query.trim();
    if (!open || q.length < 2 || q === value.weatherPlace) {
      setResults([]);
      return;
    }
    setSearching(true);
    const t = window.setTimeout(() => {
      geocode(q)
        .then((r) => setResults(r))
        .catch(() => setResults([]))
        .finally(() => setSearching(false));
    }, 350);
    return () => window.clearTimeout(t);
  }, [query, open, value.weatherPlace]);

  useEffect(() => {
    const away = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, []);

  const pick = (p: WeatherPlace) => {
    onChange({ ...value, weatherPlace: p.name, weatherLat: p.lat, weatherLon: p.lon });
    setQuery(p.name);
    setResults([]);
    setOpen(false);
  };
  const clear = () => {
    onChange({ ...value, weatherPlace: "", weatherLat: 0, weatherLon: 0 });
    setQuery("");
    setResults([]);
  };

  const chosen = value.weatherPlace && query === value.weatherPlace;
  return (
    <div className="place-search" ref={box}>
      <input
        value={query}
        placeholder={value.city.trim() ? `Same as the dateline: ${value.city.trim()}` : "e.g. Boise, Idaho"}
        maxLength={80}
        onChange={(e) => {
          setQuery(e.target.value);
          setOpen(true);
          if (e.target.value.trim() === "" && value.weatherPlace) clear();
        }}
        onFocus={() => setOpen(true)}
        aria-autocomplete="list"
      />
      {chosen && (
        <button type="button" className="place-clear" onClick={clear} title="Use the dateline city instead" aria-label="Clear weather location">
          ×
        </button>
      )}
      {open && (results.length > 0 || (searching && query.trim().length >= 2 && !chosen)) && (
        <ul className="place-results" role="listbox">
          {results.map((p) => (
            <li key={`${p.lat},${p.lon}`} role="option" aria-selected={false} onMouseDown={() => pick(p)}>
              {p.name}
            </li>
          ))}
          {results.length === 0 && searching && <li className="place-searching">Looking it up…</li>}
        </ul>
      )}
    </div>
  );
}

export default function ProfileFields({ value, onChange, autoFocus }: Props) {
  return (
    <div className="profile-fields">
      <label className="field grow">
        <span>Your first name</span>
        <input
          className="beat-name"
          value={value.ownerName}
          placeholder="It goes on the masthead"
          autoFocus={autoFocus}
          maxLength={40}
          onChange={(e) => onChange({ ...value, ownerName: e.target.value })}
        />
      </label>
      <label className="field grow">
        <span>City for the dateline (optional)</span>
        <input value={value.city} placeholder="e.g. Boise" maxLength={60} onChange={(e) => onChange({ ...value, city: e.target.value })} />
      </label>
      <label className="field">
        <span>Ball team for the score box</span>
        <select value={value.mlbTeamId} onChange={(e) => onChange({ ...value, mlbTeamId: Number(e.target.value) })}>
          <option value={0}>I’m lame and don’t like baseball</option>
          {MLB_TEAMS.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name}
            </option>
          ))}
        </select>
      </label>
      <div className="field grow">
        <span>
          Weather for <span className="field-hint">(the ear under the date; a story when it’s wild)</span>
        </span>
        <PlaceSearch value={value} onChange={onChange} />
      </div>
    </div>
  );
}

export type CardKind = "article" | "video" | "post";
export type CardSize = "lead" | "feature" | "brief";

export interface Card {
  id: string;
  kind: CardKind;
  size: CardSize;
  section: string;
  headline: string;
  dek: string;
  /** Claude's written piece; paragraphs separated by a blank line. Absent in older editions. */
  story?: string | null;
  source: string;
  author?: string | null;
  url: string;
  /** Article about a specific video: the video itself. */
  videoUrl?: string | null;
  /** Video with a good write-up: that page. */
  articleUrl?: string | null;
  image?: string | null;
  published?: string | null;
  why?: string | null;
}

export interface EditionStats {
  durationSecs: number;
  searches: number;
  pagesRead: number;
  wireItems: number;
  xPostsFromGrok: number;
  costUsd?: number | null;
}

export interface Edition {
  date: string;
  generatedAt: string;
  editionNo: number;
  tagline?: string | null;
  sections: string[];
  cards: Card[];
  notes: string[];
  stats: EditionStats;
}

export type InterestKind = "topic" | "person" | "youtube_channel" | "x_account";

export interface Interest {
  id: string;
  name: string;
  kind: InterestKind;
  value: string;
  notes: string;
  enabled: boolean;
}

export interface InterestsFile {
  version: number;
  interests: Interest[];
}

export type Stage = "wire" | "grok" | "research" | "writing" | "content" | "done" | "error";

export interface Status {
  stage: Stage;
  message: string;
  detail?: string | null;
  at: number;
}

export interface Diagnostics {
  claudePath: string | null;
  grokPath: string | null;
  dataDir: string;
}

export interface TeamLine {
  abbr: string;
  name: string;
  score: number | null;
}

export interface Situation {
  balls: number;
  strikes: number;
  outs: number;
  /** Runner on first, second, third. */
  bases: [boolean, boolean, boolean];
  batter?: string | null;
  pitcher?: string | null;
  /** ABS challenges left: [away, home]. Absent when the league doesn't report them. */
  challenges?: [number, number] | null;
}

export interface GameLine {
  gamePk: number;
  state: "live" | "final" | "preview" | "postponed";
  detail: string;
  start: string;
  away: TeamLine;
  home: TeamLine;
  weAreHome: boolean;
  url: string;
  /** Where to watch. */
  tv?: string[];
  /** Live games only. */
  situation?: Situation | null;
}

export interface ScoreBugData {
  game: GameLine | null;
  next: GameLine | null;
}

export interface ScheduleInfo {
  /** False anywhere but macOS. */
  supported: boolean;
  enabled: boolean;
  hour: number;
  minute: number;
}

export interface Profile {
  ownerName: string;
  /** "Sam's Daily" */
  paperName: string;
  city: string;
  /** Weather location; empty place and 0,0 = the dateline city, looked up on the fly. */
  weatherPlace: string;
  weatherLat: number;
  weatherLon: number;
  /** MLB team for the score bug; 0 = none. */
  mlbTeamId: number;
  onboarded: boolean;
}

export interface PrintStatus {
  printDaily: boolean;
  browserFound: boolean;
  /** The printer that would be used right now. */
  printer: string | null;
  problem: string | null;
  /** The printer named in settings; "" = the system default. */
  printerSetting: string;
  /** Every printer queue on this Mac. */
  printers: string[];
  defaultPrinter: string | null;
  color: boolean;
  qr: boolean;
  duplex: boolean;
  /** 0 = no cap. */
  maxPages: number;
  copies: number;
}

export interface PrintOptions {
  printer: string;
  color: boolean;
  qr: boolean;
  duplex: boolean;
  maxPages: number;
  copies: number;
}

// ------------------------------------------------------------------ weather

export interface WeatherPlace {
  /** "Boise, Idaho, United States" */
  name: string;
  lat: number;
  lon: number;
  country: string;
}

export interface WeatherCurrent {
  temp: number;
  feelsLike: number;
  humidity: number;
  windMph: number;
  gustsMph: number;
  code: number;
  text: string;
  isDay: boolean;
}

export interface WeatherDay {
  date: string;
  weekday: string;
  high: number;
  low: number;
  code: number;
  text: string;
  rainChance: number;
  precipIn: number;
  snowIn: number;
  gustsMph: number;
  uv: number;
  sunrise: string;
  sunset: string;
}

export interface WeatherAlert {
  event: string;
  headline: string;
  severity: string;
  description: string;
  expires: string | null;
}

export interface WeatherReport {
  place: string;
  lat: number;
  lon: number;
  updated: string;
  current: WeatherCurrent;
  today: WeatherDay;
  yesterday: WeatherDay | null;
  days: WeatherDay[];
  alerts: WeatherAlert[];
  /** Why today is out of the ordinary; empty on a routine day. */
  unusual: string[];
  url: string;
  source: string;
}

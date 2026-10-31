//! # AERO-SHIELD LRCV V2.0 — Native Rust SSR Module for 6-Year-Old & Beginner Interactive Academy
//!
//! This module implements a strongly-typed Server-Side Rendering (SSR) DOM builder (`HtmlComponent`,
//! `DomElement`, `rsx_elem!`) and the complete 6-Lesson Interactive Aviation Academy in Romanian
//! for 6-year-olds and absolute beginners (`InteractiveLesson6Yo`, `LessonActionButton`, `Lessons6YoDeck`).
//!
//! ## UX Philosophy: Ultra-Spacious, One Concept Per Page
//! - Single centered card (`max-width: 1050px; margin: 0 auto; padding: 32px; gap: 22px`).
//! - 1) Spacious top step pills (`Lecția 1 🎈` ... `Lecția 6 🏆`).
//! - 2) Single centered lesson title (`26px`).
//! - 3) Large centered interactive 60 FPS Canvas (`#lessonInteractiveCanvas`, `width="760" height="340"`).
//! - 4) Three large, breathable action buttons below the canvas (`#lessonActionButtons`, `padding: 14px 22px`, `font-size: 17px`).
//! - 5) One single clear takeaway sentence (`font-size: 20px`) + large Prev/Next navigation buttons.
//!
//! ## Safety & Avionics Standards (DO-178C Level A Philosophy)
//! - **Zero `.unwrap()` / `.expect()` / `panic!()`** in production code.

use axum::{
    extract::Query,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Escapes special HTML characters to prevent injection and ensure valid markup.
#[must_use]
pub fn escape_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Core trait for strongly-typed Rust Server-Side Rendered (SSR) HTML components.
pub trait HtmlComponent {
    /// Renders the component into a valid HTML string without panicking.
    fn render_html(&self) -> String;
}

/// Strongly-typed DOM Node for pure Rust Server-Side Rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedDomNode {
    /// Escaped text node.
    Text(String),
    /// Trusted raw HTML/SVG/JS fragment (used for verified inline scripts/styles).
    RawHtml(String),
    /// Standard HTML element node with tag, attributes, and children.
    Element(DomElement),
}

/// Strongly-typed HTML DOM Element builder with zero-panic rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomElement {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub children: Vec<TypedDomNode>,
    pub self_closing: bool,
}

impl DomElement {
    /// Creates a new standard HTML element with the given tag name.
    #[must_use]
    pub fn new(tag: &'static str) -> Self {
        Self {
            tag,
            attrs: Vec::new(),
            children: Vec::new(),
            self_closing: false,
        }
    }

    /// Creates a self-closing HTML element (e.g., `<input>`, `<meta>`, `<hr>`).
    #[must_use]
    pub fn new_void(tag: &'static str) -> Self {
        Self {
            tag,
            attrs: Vec::new(),
            children: Vec::new(),
            self_closing: true,
        }
    }

    /// Adds an HTML attribute (value is automatically escaped during rendering).
    #[must_use]
    pub fn attr(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.attrs.push((name, value.into()));
        self
    }

    /// Appends a child `DomElement`.
    #[must_use]
    pub fn child(mut self, elem: DomElement) -> Self {
        self.children.push(TypedDomNode::Element(elem));
        self
    }

    /// Appends an escaped text node.
    #[must_use]
    pub fn text(mut self, txt: impl Into<String>) -> Self {
        self.children.push(TypedDomNode::Text(txt.into()));
        self
    }

    /// Appends a pre-verified raw HTML fragment (e.g., rich formatting or script).
    #[must_use]
    pub fn raw_html(mut self, raw: impl Into<String>) -> Self {
        self.children.push(TypedDomNode::RawHtml(raw.into()));
        self
    }

    /// Renders this DOM node recursively into an existing `String` buffer.
    pub fn write_into(&self, buf: &mut String) {
        let _ = write!(buf, "<{}", self.tag);
        for (k, v) in &self.attrs {
            let _ = write!(buf, " {}=\"{}\"", k, escape_html(v));
        }
        if self.self_closing {
            buf.push_str(" />");
            return;
        }
        buf.push('>');
        for child in &self.children {
            match child {
                TypedDomNode::Text(t) => buf.push_str(&escape_html(t)),
                TypedDomNode::RawHtml(r) => buf.push_str(r),
                TypedDomNode::Element(el) => el.write_into(buf),
            }
        }
        let _ = write!(buf, "</{}>", self.tag);
    }
}

impl HtmlComponent for DomElement {
    fn render_html(&self) -> String {
        let mut buf = String::with_capacity(1024);
        self.write_into(&mut buf);
        buf
    }
}

/// Declarative macro for constructing `DomElement` nodes ergonomically in pure Rust.
#[macro_export]
macro_rules! rsx_elem {
    ($tag:expr, [ $( ($attr_k:expr, $attr_v:expr) ),* $(,)? ], [ $( $child:expr ),* $(,)? ]) => {{
        let mut el = $crate::ui_lessons_6yo::DomElement::new($tag);
        $(
            el = el.attr($attr_k, $attr_v);
        )*
        $(
            el = el.child($child);
        )*
        el
    }};
    ($tag:expr, [ $( ($attr_k:expr, $attr_v:expr) ),* $(,)? ], text: $txt:expr) => {{
        let mut el = $crate::ui_lessons_6yo::DomElement::new($tag);
        $(
            el = el.attr($attr_k, $attr_v);
        )*
        el.text($txt)
    }};
    ($tag:expr, [ $( ($attr_k:expr, $attr_v:expr) ),* $(,)? ], raw: $raw:expr) => {{
        let mut el = $crate::ui_lessons_6yo::DomElement::new($tag);
        $(
            el = el.attr($attr_k, $attr_v);
        )*
        el.raw_html($raw)
    }};
}

/// Interactive button definition inside one of the 6 kid-friendly lessons.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LessonActionButton {
    pub id: &'static str,
    pub emoji: &'static str,
    pub label: &'static str,
    pub subtitle: &'static str,
    pub color_theme: &'static str,
    pub action_code: &'static str,
}

impl HtmlComponent for LessonActionButton {
    fn render_html(&self) -> String {
        let btn_class = format!("kid-action-btn theme-{}", self.color_theme);
        let onclick_js = format!("window.setLessonMode('{}', this)", self.action_code);

        let label_span = rsx_elem!(
            "span",
            [("class", "kid-btn-title")],
            text: self.label
        );
        let btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", self.id),
                ("class", btn_class),
                ("data-action-code", self.action_code),
                ("title", self.subtitle),
                ("onclick", onclick_js)
            ],
            [label_span]
        );
        btn.render_html()
    }
}

/// Strongly-typed specification of an Interactive Lesson for a 6-year-old / beginner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractiveLesson6Yo {
    pub step_number: u8,
    pub badge_emoji: &'static str,
    pub tab_label: &'static str,
    pub title: &'static str,
    pub kid_story_ro: &'static str,
    pub visual_rule_ro: &'static str,
    pub science_secret_ro: &'static str,
    pub default_mode: &'static str,
    pub fg_video_url: &'static str,
    pub action_buttons: Vec<LessonActionButton>,
}

impl HtmlComponent for InteractiveLesson6Yo {
    fn render_html(&self) -> String {
        let is_first = self.step_number == 1;
        let group_class = if is_first {
            "lesson-6yo-action-group active"
        } else {
            "lesson-6yo-action-group"
        };
        let group_id = format!("lesson-6yo-card-{}", self.step_number);
        let step_str = self.step_number.to_string();

        let mut buttons_row = DomElement::new("div")
            .attr("id", group_id)
            .attr("class", group_class)
            .attr("data-step", step_str)
            .attr("data-title", self.title)
            .attr("data-summary", self.kid_story_ro)
            .attr("data-default-mode", self.default_mode)
            .attr("data-fg-video-url", self.fg_video_url)
            .attr("role", "group")
            .attr("aria-label", self.title);

        for (idx, btn) in self.action_buttons.iter().enumerate() {
            let mut btn_html = btn.render_html();
            if idx == 0 {
                btn_html = btn_html.replacen("kid-action-btn ", "kid-action-btn active-mode ", 1);
            }
            buttons_row = buttons_row.raw_html(btn_html);
        }

        buttons_row.render_html()
    }
}

/// Returns the complete, ordered curriculum of 6 interactive lessons (1 concise concept per page).
#[must_use]
pub fn get_all_6yo_lessons() -> Vec<InteractiveLesson6Yo> {
    vec![
        // LECȚIA 1
        InteractiveLesson6Yo {
            step_number: 1,
            badge_emoji: "🎈",
            tab_label: "Lecția 1 🎈",
            title: "🎈 Lecția 1: „Problema: Ceața, Hackerul Rău 👾 și Musculița Google 🐝”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «🛡️ Pornește AERO-SHIELD + Musculița Google 🐝» de mai jos ca să vezi în zborul sintetic FlightGear cum tăiem firul hackerului cu Foarfeca Aurie ✂️!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «🛡️ Pornește AERO-SHIELD + Musculița Google 🐝» pentru rezolvarea FlightGear!",
            science_secret_ro: "Formula 5 (Scutul Anti-Hackeri / GPS Fals — Scădere și Comparație simplă): Minciuna GPS = |Poziția spusă de GPS − Poziția văzută de Cameră și Dealuri| (Δ_eroare = |P_GPS − P_real|). Exemplu cu numere: |500 m (unde minte hackerul că suntem) − 0 m (unde ne arată camera și dealul că suntem)| = 500 m > 2 m (limita permisă) ➔ 🚨 GPS FALS OPRIT AUTOMAT!",
            default_mode: "l1_fog",
            fg_video_url: "/real_videos/fg_lesson_1.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l1-fog",
                    emoji: "🌫️",
                    label: "🌫️ Pune Ceață",
                    subtitle: "Norul alb ascunde pista de ochii pilotului",
                    color_theme: "blue",
                    action_code: "l1_fog",
                },
                LessonActionButton {
                    id: "btn-l1-hacker",
                    emoji: "👾",
                    label: "👾 Hacker GPS",
                    subtitle: "Antena-magnet trage avionul spre pista fantomă!",
                    color_theme: "red",
                    action_code: "l1_hacker",
                },
                LessonActionButton {
                    id: "btn-l1-shield",
                    emoji: "🛡️",
                    label: "🟢 BUTONUL VERDE: 🛡️ Pornește AERO-SHIELD + Musculița Google 🐝",
                    subtitle: "Rezolvă lecția cu FlightGear + Foarfeca Aurie ✂️!",
                    color_theme: "green",
                    action_code: "l1_shield",
                },
            ],
        },
        // LECȚIA 2
        InteractiveLesson6Yo {
            step_number: 2,
            badge_emoji: "🔦",
            tab_label: "Lecția 2 🔦",
            title: "🔦 Lecția 2: „Ochii Magici T4/T5 ai Musculiței Google văd prin Ceață 👀”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «🔦 Aprinde Lupa SWIR + Ochii T4/T5 🐝» de mai jos ca să vezi în zborul sintetic FlightGear cum vedem becurile pistei direct prin ceață și fum!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «🔦 Aprinde Lupa SWIR + Ochii T4/T5 🐝» și plimbă mouse-ul pe ecran!",
            science_secret_ro: "Formula 2 (De ce vedem de 4× mai departe prin Ceață în Infraroșu SWIR — Înmulțire simplă): Distanța Văzută în Infraroșu (SWIR) = 4 × Distanța Văzută cu Ochiul (D_SWIR = 4 × D_ochi). Exemplu cu numere: Dacă pilotul vede cu ochiul doar la 50 metri în ceață → Camera noastră vede la 4 × 50 m = 200 metri!",
            default_mode: "l2_swir",
            fg_video_url: "/real_videos/fg_lesson_2.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l2-fog-smoke",
                    emoji: "🌫️",
                    label: "🌫️ Ceață Albă / 🖤 Fum Negru",
                    subtitle: "Comută între ceață deasă și fum întunecat",
                    color_theme: "amber",
                    action_code: "l2_fog_smoke",
                },
                LessonActionButton {
                    id: "btn-l2-swir",
                    emoji: "🔦",
                    label: "🟢 BUTONUL VERDE: 🔦 Aprinde Lupa SWIR + Ochii T4/T5 🐝",
                    subtitle: "Activează Lupa Magică SWIR și neuronii T4/T5 peste zborul FlightGear!",
                    color_theme: "green",
                    action_code: "l2_swir",
                },
                LessonActionButton {
                    id: "btn-l2-waves",
                    emoji: "💧",
                    label: "💧 Razele SWIR vs. Picăturile",
                    subtitle: "Vezi cum unda mică se lovește, iar unda mare SWIR trece spre T4/T5!",
                    color_theme: "purple",
                    action_code: "l2_waves",
                },
            ],
        },
        // LECȚIA 3
        InteractiveLesson6Yo {
            step_number: 3,
            badge_emoji: "🚂",
            tab_label: "Lecția 3 🚂",
            title: "🚂 Lecția 3: „Cum numără neuronii T4/T5 becurile + Inelul-Busolă E-PG cu 16 Luminițe 💡”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «📏 Rigla T4/T5 + Inelul E-PG 🐝» de mai jos ca să măsori viteza pe zborul sintetic FlightGear și să aprinzi Busola în Inel cu 16 Luminițe spre Craiova RWY 09!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «📏 Rigla T4/T5 + Inelul E-PG 🐝» pentru viteza fără GPS + Busola E-PG!",
            science_secret_ro: "Formula 4 (Amprenta Dealurilor sub Nori pentru Eroare < 1m la 100.000 km — Scădere simplă): Înălțimea Dealului de sub noi = Altitudinea Avionului (Barometru) − Distanța până la Pământ (Radar) (H_deal = H_avion − D_radar). Exemplu cu numere: 500 m (altitudinea avionului în nori) − 420 m (distanța măsurată de radar în jos) = 80 metri (înălțimea exactă a dealului de sub noi → îl găsim pe hartă cu eroare < 1 metru, fără GPS!)",
            default_mode: "l3_slow",
            fg_video_url: "/real_videos/fg_lesson_3.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l3-slow",
                    emoji: "🐢",
                    label: "🐢 Zboară Încet (T4/T5)",
                    subtitle: "Neuronii T4/T5 numără săgeți scurte între două poze (45 m/s)",
                    color_theme: "blue",
                    action_code: "l3_slow",
                },
                LessonActionButton {
                    id: "btn-l3-fast",
                    emoji: "🚀",
                    label: "🚀 Zboară Repede (T4/T5)",
                    subtitle: "Neuronii T4/T5 văd un salt mare între două poze (120 m/s)",
                    color_theme: "amber",
                    action_code: "l3_fast",
                },
                LessonActionButton {
                    id: "btn-l3-ruler",
                    emoji: "📏",
                    label: "🟢 BUTONUL VERDE: 📏 Rigla T4/T5 + Inelul E-PG 🐝",
                    subtitle: "Rezolvă lecția pe FlightGear cu Rigla Galbenă și Inelul-Busolă cu 16 Luminițe!",
                    color_theme: "green",
                    action_code: "l3_ruler",
                },
            ],
        },
        // LECȚIA 4
        InteractiveLesson6Yo {
            step_number: 4,
            badge_emoji: "☁️",
            tab_label: "Lecția 4 ☁️",
            title: "☁️ Lecția 4: „Stabilizatorul HS/VS și Senzorul de Vânt PFNd al Musculiței! 💨”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «🧠 HS/VS + PFNd: Separă Norul» de mai jos ca să vezi pe zborul FlightGear cum Stabilizatorul HS/VS și Senzorul PFNd taie ❌ Norul Mov de ✅ Pământul Verde!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «🧠 HS/VS + PFNd: Separă Norul» pentru stabilizarea zborului în vânt!",
            science_secret_ro: "Formula 1 (Eliminarea Balansului Avionului — Scădere simplă): Mișcarea Pământului = Mișcarea din Cameră − Rotirea Avionului (V_pământ = V_cameră − V_rotire) → 15 pixeli/s − 12 pixeli/s = 3 pixeli/s! Formula 3 (Anularea Vântului Lateral Drosophila — Adunare și Scădere): Abaterea de la Pistă = Împingerea Vântului + Corecția Avionului = 0 (E_lateral = V_vânt + V_corecție = 0) → +10 m/s − 10 m/s = 0 metri abatere!",
            default_mode: "l4_separate",
            fg_video_url: "/real_videos/fg_lesson_4.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l4-wind",
                    emoji: "💨",
                    label: "💨 Vânt în Nor (Senzor PFNd)",
                    subtitle: "Senzorul PFNd simte vântul pe antene/aripi cu 1 secundă înainte!",
                    color_theme: "amber",
                    action_code: "l4_wind",
                },
                LessonActionButton {
                    id: "btn-l4-tilt",
                    emoji: "🔄",
                    label: "🔄 Înclină Avionul (HS/VS)",
                    subtitle: "Stabilizatorul HS/VS scade rotația aripilor în 6 direcții!",
                    color_theme: "blue",
                    action_code: "l4_tilt",
                },
                LessonActionButton {
                    id: "btn-l4-separate",
                    emoji: "🧠",
                    label: "🟢 BUTONUL VERDE: 🧠 HS/VS + PFNd: Separă Norul",
                    subtitle: "Taie norul mincinos (❌) și citește doar solul verde fix (✅) pe FlightGear!",
                    color_theme: "green",
                    action_code: "l4_separate",
                },
            ],
        },
        // LECȚIA 5
        InteractiveLesson6Yo {
            step_number: 5,
            badge_emoji: "⚖️",
            tab_label: "Lecția 5 ⚖️",
            title: "⚖️ Lecția 5: „Cântarul Adevărului + Inelul-Busolă E-PG cu 16 Luminițe ⚖️”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «✅ Zbor Cinstit + Inel E-PG» (sau testează minciunile) ca să vezi pe zborul FlightGear cum Inelul E-PG cu 16 Luminițe, Cântarul ⚖️ și Paharul CUSUM prind hackerul!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «✅ Zbor Cinstit + Inel E-PG» ca să vezi echilibrul pe zborul FlightGear!",
            science_secret_ro: "Formula 5 (Scutul Anti-Hackeri / GPS Fals — Scădere și Comparație simplă): Minciuna GPS = |Poziția spusă de GPS − Poziția văzută de Cameră și Dealuri| (Δ_eroare = |P_GPS − P_real|). Exemplu cu numere: |500 m (unde minte hackerul că suntem) − 0 m (unde ne arată camera și dealul că suntem)| = 500 m > 2 m (limita permisă) ➔ 🚨 GPS FALS OPRIT AUTOMAT!",
            default_mode: "l5_honest",
            fg_video_url: "/real_videos/fg_lesson_5.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l5-honest",
                    emoji: "✅",
                    label: "🟢 BUTONUL VERDE: ✅ Zbor Cinstit + Inel E-PG",
                    subtitle: "Inelul E-PG cu 16 luminițe și Cântarul arată RWY 09 în echilibru pe FlightGear",
                    color_theme: "green",
                    action_code: "l5_honest",
                },
                LessonActionButton {
                    id: "btn-l5-big-lie",
                    emoji: "⚡",
                    label: "⚡ Minciună Mare (Salt 500m)",
                    subtitle: "Talerul GPS cade brusc, dar Inelul E-PG păstrează direcția adevărată!",
                    color_theme: "red",
                    action_code: "l5_big_lie",
                },
                LessonActionButton {
                    id: "btn-l5-slow-lie",
                    emoji: "🐍",
                    label: "🐍 Minciună Lentă (Pahar CUSUM)",
                    subtitle: "Picăturile roșii umplu Paharul de Minciuni în 3 secunde -> ALARMĂ!",
                    color_theme: "amber",
                    action_code: "l5_slow_lie",
                },
            ],
        },
        // LECȚIA 6
        InteractiveLesson6Yo {
            step_number: 6,
            badge_emoji: "🏆",
            tab_label: "Lecția 6 🏆",
            title: "🏆 Lecția 6: „Acum ești Comandantul Avionului Bio-Inspirat Drosophila + AERO-SHIELD!”",
            kid_story_ro: "👉 Apasă pe 🟢 BUTONUL VERDE «🏅 Harta Drosophila & Diplomă» de mai jos ca să vezi pe zborul FlightGear harta completă a Creierului Google Drosophila (T4/T5 → HS/VS → E-PG 16 Luminițe → PFNd)!",
            visual_rule_ro: "🟢 Apasă pe BUTONUL VERDE «🏅 Harta Drosophila & Diplomă» pentru absolvirea pe FlightGear!",
            science_secret_ro: "Cele 5 Formule Simple cu Numere (+, −, ×): (1) Balans: 15 px/s − 12 px/s = 3 px/s; (2) Ceață SWIR: 4 × 50 m = 200 m; (3) Vânt Drosophila PFNd: +10 m/s − 10 m/s = 0 m; (4) Dealuri sub Nori: 500 m − 420 m = 80 m (eroare < 1m la 100.000 km); (5) Anti-Hacker GPS: |500 m − 0 m| = 500 m > 2 m ➔ GPS Fals Oprit Automat!",
            default_mode: "l6_commander",
            fg_video_url: "/real_videos/fg_lesson_6.mp4",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l6-spotlight",
                    emoji: "✨",
                    label: "✨ Săgeți Luminoase pe Ecranul Mare",
                    subtitle: "Aprinde ghidul luminos peste Parbriz, Cursa celor 4 Avioane și Telecomandă!",
                    color_theme: "amber",
                    action_code: "l6_spotlight",
                },
                LessonActionButton {
                    id: "btn-l6-real-video",
                    emoji: "🎬",
                    label: "🎬 Pornește Filmarea Reală din Cockpit",
                    subtitle: "Comută în modul Demonstrație pe Filmare Reală & Telemetrie Drosophila",
                    color_theme: "blue",
                    action_code: "l6_real_video",
                },
                LessonActionButton {
                    id: "btn-l6-diploma",
                    emoji: "🏅",
                    label: "🟢 BUTONUL VERDE: 🏅 Harta Drosophila & Diplomă",
                    subtitle: "Vezi harta completă T4/T5 → HS/VS → E-PG → PFNd și Diploma de Comandant!",
                    color_theme: "green",
                    action_code: "l6_diploma",
                },
            ],
        },
    ]
}

/// Complete deck component containing all 6 interactive lessons in an ultra-spacious, one-concept-per-page layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lessons6YoDeck {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub lessons: Vec<InteractiveLesson6Yo>,
}

impl Default for Lessons6YoDeck {
    fn default() -> Self {
        Self {
            title: "🎈 ACADEMIA DE ZBOR AERO-SHIELD (CÂTE UN CONCEPT SIMPLU PE PAGINĂ)",
            subtitle: "Un singur desen interactiv mare și 3 butoane simple pentru fiecare pas!",
            lessons: get_all_6yo_lessons(),
        }
    }
}

impl HtmlComponent for Lessons6YoDeck {
    fn render_html(&self) -> String {
        let mut root = DomElement::new("section")
            .attr("id", "aero-lessons-6yo-root")
            .attr("class", "lessons-6yo-container")
            .attr("aria-label", "Academia Interactivă de Zbor — Câte un concept pe pagină");

        // Scoped Ultra-Spacious CSS
        let style_el = DomElement::new("style").raw_html(LESSONS_6YO_SCOPED_CSS);
        root = root.child(style_el);

        // 1. Simple, spacious step bar at the top (Lecția 1 🎈 ... Lecția 6 🏆)
        let mut stepper_tabs = DomElement::new("nav")
            .attr("class", "lessons-6yo-stepper-tabs")
            .attr("aria-label", "Pașii Academiei");

        for lesson in &self.lessons {
            let tab_class = if lesson.step_number == 1 {
                "lesson-6yo-step-btn active"
            } else {
                "lesson-6yo-step-btn"
            };
            let btn_id = format!("lesson-6yo-tab-{}", lesson.step_number);
            let onclick = format!("window.selectLesson6YoStep({})", lesson.step_number);
            let step_btn = rsx_elem!(
                "button",
                [
                    ("type", "button"),
                    ("id", btn_id),
                    ("class", tab_class),
                    ("data-step", lesson.step_number.to_string()),
                    ("onclick", onclick)
                ],
                text: lesson.tab_label
            );
            stepper_tabs = stepper_tabs.child(step_btn);
        }

        // 2. Short, single-line centered title of the current lesson (26px)
        let first_title = self
            .lessons
            .first()
            .map(|l| l.title)
            .unwrap_or("🎈 Lecția 1: „Problema: Ceața și Hackerul Rău 👾”");

        let current_title_el = rsx_elem!(
            "h2",
            [("id", "lessonCurrentTitle"), ("class", "lesson-6yo-single-title")],
            text: first_title
        );

        let first_fg_video = self
            .lessons
            .first()
            .map(|l| l.fg_video_url)
            .unwrap_or("/real_videos/fg_lesson_1.mp4");

        // 3. Two Completely Separate, Opaque Media Boxes (gap: 24px, opacity: 1.0, ZERO overlap):
        //    - Box 1: Interactive Animation Canvas (#lessonInteractiveCanvas)
        //    - Box 2: FlightGear Synthetic Flight Video (#fgLessonVideoPlayer)
        let box1_header = rsx_elem!(
            "div",
            [("class", "lesson-separate-box-header")],
            text: "🎨 Animația Interactivă a Lecției (Pas cu Pas)"
        );

        let canvas_el = rsx_elem!(
            "canvas",
            [
                ("id", "lessonInteractiveCanvas"),
                ("width", "760"),
                ("height", "340"),
                ("class", "lesson-interactive-canvas"),
                ("aria-label", "Desen interactiv 760x340 pentru conceptul curent")
            ],
            text: "Browserul tău nu suportă HTML5 Canvas."
        );

        let animation_box = rsx_elem!(
            "div",
            [
                ("id", "lessonAnimationSeparateBox"),
                ("class", "lesson-separate-media-card")
            ],
            [box1_header, canvas_el]
        );

        let box2_header = rsx_elem!(
            "div",
            [("class", "lesson-separate-box-header fg-video-header")],
            text: "🎬 Videoclipul Zborului Sintetic FlightGear (Rezolvare 3D)"
        );

        let fg_video_el = rsx_elem!(
            "video",
            [
                ("id", "fgLessonVideoPlayer"),
                ("src", first_fg_video),
                ("controls", "controls"),
                ("autoplay", "autoplay"),
                ("loop", "loop"),
                ("muted", "muted"),
                ("playsinline", "playsinline"),
                ("class", "fg-lesson-video-player"),
                ("aria-label", "Videoclip Zbor Sintetic FlightGear pentru lecția curentă")
            ],
            text: "Browserul tău nu suportă HTML5 Video."
        );

        let video_box = rsx_elem!(
            "div",
            [
                ("id", "lessonVideoSeparateBox"),
                ("class", "lesson-separate-media-card fg-video-card")
            ],
            [box2_header, fg_video_el]
        );

        let canvas_wrap = rsx_elem!(
            "div",
            [("class", "lesson-canvas-center-stage separate-dual-stage")],
            [animation_box, video_box]
        );

        // Dedicated Emerald-Green Hero Solve Button right below the FlightGear video/canvas stage
        let green_hero_solve_btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", "btnSolveWithFlightGearGreen"),
                ("class", "kid-action-btn theme-green solve-fg-green-hero-btn"),
                ("onclick", "window.triggerGreenFlightGearSolution()")
            ],
            text: "🟢 BUTONUL VERDE (REZOLVĂ LECȚIA CU FLIGHTGEAR): 🛡️ Pornește AERO-SHIELD + Musculița Google 🐝"
        );

        // 4. The 3 large action buttons right below the canvas (#lessonActionButtons)
        let mut action_buttons_host = DomElement::new("div")
            .attr("id", "lessonActionButtons")
            .attr("class", "lesson-action-buttons-host");

        for lesson in &self.lessons {
            action_buttons_host = action_buttons_host.raw_html(lesson.render_html());
        }

        // 5. One single takeaway sentence (20px, centered) + large Prev / Next buttons
        let first_summary = self
            .lessons
            .first()
            .map(|l| l.kid_story_ro)
            .unwrap_or("");

        let megaphone_banner = rsx_elem!(
            "p",
            [("id", "lessonMegaphoneBanner"), ("class", "lesson-single-takeaway")],
            raw: first_summary
        );

        let formulas_card = DomElement::new("div")
            .attr("id", "simple3Formulas100kCard")
            .attr("class", "simple-3-formulas-100k-card")
            .attr(
                "aria-label",
                "Cele 3 Formule Simple fără matematică grea pentru precizie sub 1 metru la 100.000 km",
            )
            .raw_html(SIMPLE_3_FORMULAS_100K_HTML);

        let prev_btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", "btn-lesson-6yo-prev"),
                ("class", "lesson-6yo-nav-arrow"),
                ("onclick", "window.stepLesson6YoDelta(-1)")
            ],
            text: "⬅️ Lecția Anterioară"
        );

        let next_btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", "btn-lesson-6yo-next"),
                ("class", "lesson-6yo-nav-arrow primary-next"),
                ("onclick", "window.stepLesson6YoDelta(1)")
            ],
            text: "➡️ Lecția Următoare"
        );

        let bottom_nav = rsx_elem!(
            "div",
            [("class", "lessons-6yo-bottom-nav")],
            [prev_btn, next_btn]
        );

        // Single central spacious card (`max-width: 1050px; margin: 0 auto; padding: 32px; gap: 22px`)
        let single_card = rsx_elem!(
            "div",
            [("id", "lessons-6yo-collapsible-body"), ("class", "lessons-6yo-central-card")],
            [
                stepper_tabs,
                current_title_el,
                canvas_wrap,
                green_hero_solve_btn,
                action_buttons_host,
                megaphone_banner,
                formulas_card,
                bottom_nav
            ]
        );

        root = root.child(single_card);

        // Spotlight Overlay Container for Lesson 6 ("✨ Arată-mi cu Săgeți Luminoase pe Ecranul Mare")
        let spotlight_overlay = DomElement::new("div")
            .attr("id", "aero-6yo-spotlight-overlay")
            .attr("class", "aero-6yo-spotlight-overlay hidden")
            .raw_html(SPOTLIGHT_OVERLAY_INNER_HTML);
        root = root.child(spotlight_overlay);

        // Interactive 60 FPS Canvas & Lesson Controller Script
        let script_el = DomElement::new("script").raw_html(LESSONS_6YO_INTERACTIVE_JS);
        root = root.child(script_el);

        root.render_html()
    }
}

/// Renders the embeddable SSR HTML component for the 6-Year-Old & Beginner Interactive Academy.
#[must_use]
pub fn render_lessons_6yo_component() -> String {
    Lessons6YoDeck::default().render_html()
}

/// Renders a complete standalone HTML5 page containing the 6-Year-Old & Beginner Interactive Academy.
#[must_use]
pub fn render_lessons_6yo_standalone_page() -> String {
    let component_html = render_lessons_6yo_component();
    let mut doc = String::with_capacity(component_html.len() + 1024);
    doc.push_str("<!DOCTYPE html>\n<html lang=\"ro\">\n<head>\n");
    doc.push_str("  <meta charset=\"UTF-8\" />\n");
    doc.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n");
    doc.push_str("  <title>AERO-SHIELD V2.0 — Academia de Zbor (Câte 1 Concept pe Pagină)</title>\n");
    doc.push_str("  <style>\n");
    doc.push_str("    body { margin: 0; padding: 28px 18px; background: #070B14; color: #FFFFFF; font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; }\n");
    doc.push_str("    .standalone-top-nav { max-width: 1050px; margin: 0 auto 20px auto; display: flex; justify-content: space-between; align-items: center; }\n");
    doc.push_str("    .standalone-back-link { color: #FFFFFF; text-decoration: none; font-weight: 800; font-size: 15px; padding: 10px 18px; border: 2px solid #38BDF8; border-radius: 10px; background: #0F172A; }\n");
    doc.push_str("    .standalone-back-link:hover { background: #0C4A6E; border-color: #FEF08A; color: #FEF08A; }\n");
    doc.push_str("  </style>\n");
    doc.push_str("</head>\n<body>\n");
    doc.push_str("  <div class=\"standalone-top-nav\">\n");
    doc.push_str("    <a class=\"standalone-back-link\" href=\"/\">⬅️ Înapoi la Simulatorul Principal AERO-SHIELD</a>\n");
    doc.push_str("    <span style=\"font-size:14px;font-weight:700;color:#FEF08A;background:#0F172A;padding:6px 12px;border-radius:8px;border:1px solid #FACC15;\">Câte 1 singur concept per pagină • Contrast Maxim WCAG AAA</span>\n");
    doc.push_str("  </div>\n");
    doc.push_str(&component_html);
    doc.push_str("\n</body>\n</html>\n");
    doc
}

/// Optional query parameters for filtering or selecting a default lesson step via HTTP GET.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Lessons6YoQuery {
    pub step: Option<u8>,
    pub standalone: Option<bool>,
}

/// Axum JSON endpoint (`GET /api/lessons-6yo`) returning the strongly-typed 6 lessons array.
pub async fn api_lessons_6yo_json(Query(query): Query<Lessons6YoQuery>) -> Json<Vec<InteractiveLesson6Yo>> {
    let all = get_all_6yo_lessons();
    if let Some(step) = query.step {
        let filtered: Vec<InteractiveLesson6Yo> = all
            .into_iter()
            .filter(|l| l.step_number == step)
            .collect();
        return Json(filtered);
    }
    Json(all)
}

/// Axum HTML endpoint (`GET /api/lessons-6yo/html` or `GET /lessons-6yo`) returning the SSR HTML.
pub async fn api_lessons_6yo_html(Query(query): Query<Lessons6YoQuery>) -> impl IntoResponse {
    if query.standalone.unwrap_or(false) {
        Html(render_lessons_6yo_standalone_page())
    } else {
        Html(render_lessons_6yo_component())
    }
}

/// Axum standalone page endpoint (`GET /lessons-6yo`) returning the full standalone SSR HTML page.
pub async fn page_lessons_6yo_standalone() -> Html<String> {
    Html(render_lessons_6yo_standalone_page())
}

/// Builds a ready-to-mount `axum::Router` for the 6-year-old interactive lessons endpoints.
pub fn lessons_6yo_router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/lessons-6yo", get(api_lessons_6yo_json))
        .route("/api/lessons-6yo/html", get(api_lessons_6yo_html))
        .route("/lessons-6yo", get(page_lessons_6yo_standalone))
}

/// Simple 5 Intuitive Mathematical Formulas + Concrete Examples with Simple Numbers (+, -, ×) + Zenith SWIR + DRA Card HTML (Zero heavy math, < 1m precision at 100,000 km, WCAG AAA contrast).
const SIMPLE_3_FORMULAS_100K_HTML: &str = r#"
<div class="simple-3-formulas-title">
  💡 Cele 5 Formule Matematice Intuitive + Exemple Concrete cu Numere Simple (+, −, ×) pe Înțelesul Oricui (Fără Matematică Grea!) — PRECIZIE &lt; 1 METRU CHIAR ȘI LA 100.000 KM FĂRĂ GPS:
</div>
<div class="simple-3-formulas-list">
  <div class="simple-formula-item">
    <span class="simple-formula-badge">1. 🔄 Formula 1 — Eliminarea Balansului Avionului (Scădere simplă / Formula 2 (Creierul Musculiței Google) HS/VS):</span>
    <strong class="simple-formula-eq">Mișcarea Pământului = Mișcarea din Cameră − Rotirea Avionului (V_pământ = V_cameră − V_rotire)</strong>
    <span class="simple-formula-desc"> — <strong>Exemplu cu numere:</strong> <code>15 pixeli/s</code> (cât se mișcă imaginea pe cameră) − <code>12 pixeli/s</code> (cât s-a înclinat botul avionului) = <code>3 pixeli/s</code> (mișcarea reală față de pistă!).</span>
  </div>
  <div class="simple-formula-item">
    <span class="simple-formula-badge">2. 🔦 Formula 2 — De ce vedem de 4× mai departe prin Ceață în Infraroșu SWIR (Înmulțire simplă):</span>
    <strong class="simple-formula-eq">Distanța Văzută în Infraroșu (SWIR) = 4 × Distanța Văzută cu Ochiul (D_SWIR = 4 × D_ochi)</strong>
    <span class="simple-formula-desc"> — <strong>Exemplu cu numere:</strong> Dacă pilotul vede cu ochiul doar la <code>50 metri</code> în ceață → Camera noastră vede la <code>4 × 50 m = 200 metri!</code></span>
  </div>
  <div class="simple-formula-item">
    <span class="simple-formula-badge">3. 🐝 Formula 3 — Anularea Vântului Lateral cu Creierul de Muscă Drosophila (Adunare și Scădere PFNd):</span>
    <strong class="simple-formula-eq">Abaterea de la Pistă = Împingerea Vântului + Corecția Avionului = 0 (E_lateral = V_vânt + V_corecție = 0)</strong>
    <span class="simple-formula-desc"> — <strong>Exemplu cu numere:</strong> <code>+10 m/s</code> (vântul ne împinge la dreapta în iarbă) − <code>10 m/s</code> (neuronii PFNd îndreaptă avionul spre stânga) = <code>0 metri abatere</code> (aterizăm fix pe linia din mijlocul pistei!).</span>
  </div>
  <div class="simple-formula-item">
    <span class="simple-formula-badge">4. ⛰️ Formula 4 — Amprenta Dealurilor sub Nori pentru Eroare &lt; 1m la 100.000 km (Scădere simplă / Formula 1 (Cheia Pământului)):</span>
    <strong class="simple-formula-eq">Înălțimea Dealului de sub noi = Altitudinea Avionului (Barometru) − Distanța până la Pământ (Radar) (H_deal = H_avion − D_radar)</strong>
    <span class="simple-formula-desc"> — <strong>Exemplu cu numere:</strong> <code>500 m</code> (altitudinea avionului în nori) − <code>420 m</code> (distanța măsurată de radar în jos) = <code>80 metri</code> (înălțimea exactă a dealului de sub noi → îl găsim pe hartă cu eroare &lt; 1 metru, fără GPS!).</span>
  </div>
  <div class="simple-formula-item" style="border-color:#F87171;">
    <span class="simple-formula-badge">5. 🛡️ Formula 5 — Scutul Anti-Hackeri / GPS Fals (Scădere și Comparație simplă / Formula 3 (Detectorul de Minciuni)):</span>
    <strong class="simple-formula-eq">Minciuna GPS = |Poziția spusă de GPS − Poziția văzută de Cameră și Dealuri| (Δ_eroare = |P_GPS − P_real|)</strong>
    <span class="simple-formula-desc"> — <strong>Exemplu cu numere:</strong> <code>|500 m</code> (unde minte hackerul că suntem) − <code>0 m</code> (unde ne arată camera și dealul că suntem)<code>| = 500 m &gt; 2 m</code> (limita permisă) ➔ 🚨 <strong>GPS FALS OPRIT AUTOMAT!</strong></span>
  </div>
  <div class="simple-formula-item" style="border-color:#F0ABFC;">
    <span class="simple-formula-badge">🔭 Bonus — Formula 4 (Busola Cerului — Camera de Sus Zenith SWIR + DRA):</span>
    <strong class="simple-formula-eq">Direcția Nord Adevărată = Stelele văzute Ziua în Infraroșu (peste nori) + Inelele de Lumină Polarizată (sub nori, ca Ochiul de Sus al Muștei DRA)</strong>
    <span class="simple-formula-desc"> — Chiar și ziua la amiază sau sub nori groși, camera orientată în sus vede stelele în infraroșu și inelele de lumină polarizată ale Soarelui — blocând eroarea busolei E-PG sub 0,005°!</span>
  </div>
</div>
<div style="margin-top:16px; background:#070B14; border:2px solid #34D399; border-radius:14px; padding:16px 18px; display:flex; flex-direction:column; gap:10px;">
  <div style="font-size:17.5px; font-weight:900; color:#FEF08A;">
    🔭 Camera Infraroșu de Sus (Zenith SWIR + Ochiul Dorsal DRA — Al 4-lea Pilon de Navigație)
  </div>
  <div style="font-size:14.5px; color:#FFFFFF; line-height:1.5;">
    <strong style="color:#FEF08A;">1️⃣ Regimul 1 — Deasupra norilor sau în ceață subțire (Stele văzute Ziua în Infraroșu SWIR 1.5 µm):</strong> Camera privește drept în sus în infraroșu (SWIR 1.5 µm). Deoarece împrăștierea luminii albastre (<em>Rayleigh scattering</em>) nu există în infraroșu, cerul este negru în SWIR chiar și la ora 12:00 ziua, permițând camerei să vadă stelele strălucitoare în plină zi!
  </div>
  <div style="font-size:14.5px; color:#FFFFFF; line-height:1.5;">
    <strong style="color:#34D399;">2️⃣ Regimul 2 — În interiorul norilor groși / ceții (Polarizarea Cerului prin Ochiul Dorsal al Muștei DRA):</strong> Chiar și când norii acoperă stelele, camera de sus citește inelele de polarizare ale luminii solare împrăștiate prin nori, exact ca fotoreceptorii specializați orientați în sus (<em>Dorsal Rim Area DRA, R7/R8</em>) ai musculiței Drosophila și ca „pietrele soarelui” vikinge, fixând inelul-busolă E-PG (16 neuroni) la &lt; 0,005° fără nicio derivă!
  </div>
  <div style="font-size:14.5px; color:#FFFFFF; line-height:1.5;">
    <strong style="color:#38BDF8;">3️⃣ Explicația Fizică Simplă (CMB vs. Infraroșu SWIR + Polarizare DRA):</strong> Fondul Cosmic de Microunde (CMB 2.7 K) este în domeniul microundelor și necesită antene mari, în timp ce Infraroșul (SWIR) + Polarizarea Cerului (DRA) orientate în sus ne oferă Stelele Ziua + Busola Polarizată prin Nori într-o cameră optică compactă!
  </div>
  <div style="background:#0F172A; border:2px solid #FACC15; border-left:6px solid #34D399; border-radius:10px; padding:10px 14px; font-size:14.5px; font-weight:900; color:#FEF08A;">
    ✈️ Folosit pe SR-71 Blackbird („R2-D2” Nortronics NAS-14V1), B-2 Spirit și U-2 Dragon Lady — adus acum în premieră pe avioane civile cu arhitectura Google Drosophila DRA!
  </div>
</div>
"#;

/// Spotlight overlay HTML for Lesson 6 ("✨ Arată-mi cu Săgeți Luminoase pe Ecranul Mare").
const SPOTLIGHT_OVERLAY_INNER_HTML: &str = r#"
<div class="spotlight-backdrop" onclick="window.closeSpotlightGuide6Yo()">
  <div class="spotlight-header-card" onclick="event.stopPropagation()">
    <div style="font-size:24px;font-weight:900;color:#fde047;">
      ✨ GHIDUL SIMPLU PE ECRANUL MARE (3 ZONE)
    </div>
    <div class="spotlight-pointer-grid">
      <div class="spotlight-pointer-card">
        <div class="spotlight-arrow-bounce">↙️ 1. PARBRIZUL MAGIC</div>
        <div class="spotlight-card-desc">Vezi pista prin ceață și becurile verzi numărate de Ochiul Magic.</div>
      </div>
      <div class="spotlight-pointer-card">
        <div class="spotlight-arrow-bounce">⬇️ 2. CURSA CELOR 4 AVIOANE</div>
        <div class="spotlight-card-desc">Avionul Verde (AERO-SHIELD) rămâne pe pistă când cel Roșu e păcălit de hacker!</div>
      </div>
      <div class="spotlight-pointer-card">
        <div class="spotlight-arrow-bounce">↘️ 3. TELECOMANDA</div>
        <div class="spotlight-card-desc">Apasă pe butoanele de test și privește Cântarul Adevărului în timp real!</div>
      </div>
    </div>
    <button type="button" class="spotlight-close-btn" onclick="window.closeSpotlightGuide6Yo()">
      👍 Am înțeles! Închide ghidul
    </button>
  </div>
</div>
"#;

/// Scoped CSS for the Ultra-Spacious Single-Concept-Per-Page 6-Year-Old Academy (WCAG AAA Contrast, Zero Moving Text).
const LESSONS_6YO_SCOPED_CSS: &str = r#"
.lessons-6yo-container {
  max-width: 1050px;
  margin: 0 auto;
  padding: 12px 0;
  color: #FFFFFF;
  font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
}
.lessons-6yo-central-card {
  max-width: 1050px;
  margin: 0 auto;
  padding: 32px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 22px;
  background: #070B14;
  border: 2px solid #38BDF8;
  border-radius: 22px;
  box-shadow: 0 18px 44px #000000;
}
.lessons-6yo-central-card.collapsed {
  display: none;
}
.lessons-6yo-stepper-tabs {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 14px;
  width: 100%;
}
.lesson-6yo-step-btn {
  background: #0F172A;
  color: #FFFFFF;
  border: 2px solid #38BDF8;
  border-radius: 999px;
  padding: 10px 20px;
  font-size: 16px;
  font-weight: 800;
  cursor: pointer;
  text-shadow: 0 1px 2px #000000;
}
.lesson-6yo-step-btn:hover {
  background: #0C4A6E;
  border-color: #FEF08A;
  color: #FEF08A;
}
.lesson-6yo-step-btn.active {
  background: #0369A1;
  color: #FEF08A;
  border: 2px solid #FEF08A;
  box-shadow: 0 0 0 2px #38BDF8;
}
.lesson-6yo-single-title {
  margin: 0;
  font-size: 26px;
  font-weight: 900;
  color: #FEF08A;
  background: #0F172A;
  border: 2px solid #FACC15;
  border-radius: 14px;
  padding: 12px 22px;
  text-align: center;
  line-height: 1.35;
  text-shadow: 0 1px 2px #000000;
}
.lesson-canvas-center-stage.separate-dual-stage {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 24px;
  align-items: stretch;
}
@media (max-width: 920px) {
  .lesson-canvas-center-stage.separate-dual-stage {
    grid-template-columns: 1fr;
  }
}
.lesson-separate-media-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: #0F172A;
  border: 2px solid #38BDF8;
  border-radius: 16px;
  padding: 14px;
  box-shadow: 0 12px 30px #000000;
  opacity: 1.0;
}
.lesson-separate-media-card.fg-video-card {
  border-color: #34D399;
}
.lesson-separate-box-header {
  font-size: 15px;
  font-weight: 900;
  color: #FFFFFF;
  text-align: center;
  padding: 8px 12px;
  border-radius: 10px;
  background: #070B14;
  border: 2px solid #38BDF8;
  text-shadow: 0 1px 2px #000000;
}
.lesson-separate-box-header.fg-video-header {
  color: #FEF08A;
  border-color: #34D399;
}
.lesson-interactive-canvas {
  position: static;
  width: 100%;
  height: auto;
  aspect-ratio: 760 / 340;
  display: block;
  border-radius: 12px;
  background: #070B14;
  border: 2px solid #38BDF8;
  opacity: 1.0 !important;
  cursor: crosshair;
}
.fg-lesson-video-player {
  position: static;
  width: 100%;
  height: auto;
  aspect-ratio: 760 / 340;
  object-fit: cover;
  display: block;
  border-radius: 12px;
  background: #070B14;
  border: 2px solid #34D399;
  opacity: 1.0 !important;
}
.solve-fg-green-hero-btn {
  width: 100%;
  max-width: 980px;
  font-size: 18px !important;
  padding: 16px 24px !important;
  letter-spacing: 0.2px;
}
.lesson-action-buttons-host {
  width: 100%;
  display: flex;
  justify-content: center;
}
.lesson-6yo-action-group {
  display: none;
  flex-wrap: wrap;
  justify-content: center;
  gap: 16px;
  width: 100%;
}
.lesson-6yo-action-group.active {
  display: flex;
}
.kid-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 14px 22px;
  border-radius: 14px;
  border: 2px solid #38BDF8;
  background: #0F172A;
  color: #FFFFFF;
  font-size: 17px;
  font-weight: 800;
  cursor: pointer;
  text-shadow: 0 1px 2px #000000;
}
.kid-action-btn:hover {
  border-color: #FEF08A !important;
  color: #FEF08A !important;
}
.kid-action-btn.active-mode {
  box-shadow: 0 0 0 3px #FEF08A !important;
}
.kid-action-btn.theme-blue,
.kid-action-btn.theme-cyan {
  background: #0C4A6E !important;
  border: 2px solid #38BDF8 !important;
  color: #FFFFFF !important;
}
.kid-action-btn.theme-red {
  background: #7F1D1D !important;
  border: 2px solid #F87171 !important;
  color: #FFFFFF !important;
}
.kid-action-btn.theme-green,
#btnSolveWithFlightGearGreen {
  background: #064E3B !important;
  border: 2px solid #34D399 !important;
  color: #FFFFFF !important;
}
.kid-action-btn.theme-amber {
  background: #78350F !important;
  border: 2px solid #FACC15 !important;
  color: #FFFFFF !important;
}
.kid-action-btn.theme-purple {
  background: #4C1D95 !important;
  border: 2px solid #C084FC !important;
  color: #FFFFFF !important;
}
.lesson-single-takeaway {
  margin: 0;
  max-width: 920px;
  font-size: 20px;
  font-weight: 700;
  line-height: 1.5;
  color: #FFFFFF;
  text-align: center;
  padding: 16px 24px;
  border-radius: 14px;
  background: #0F172A;
  border: 2px solid #FACC15;
  text-shadow: 0 1px 2px #000000;
}
.simple-3-formulas-100k-card {
  width: 100%;
  max-width: 980px;
  background: #070B14;
  border: 2px solid #FACC15;
  border-radius: 16px;
  padding: 20px 24px;
  color: #FFFFFF;
  box-shadow: 0 12px 28px #000000;
}
.simple-3-formulas-title {
  font-size: 18px;
  font-weight: 900;
  color: #FEF08A;
  margin-bottom: 14px;
  text-align: center;
  line-height: 1.4;
}
.simple-3-formulas-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.simple-formula-item {
  background: #0F172A;
  border: 2px solid #38BDF8;
  border-radius: 12px;
  padding: 12px 16px;
  font-size: 15.5px;
  line-height: 1.5;
  color: #FFFFFF;
  white-space: normal;
  overflow-wrap: break-word;
  word-break: break-word;
}
.simple-formula-badge {
  color: #FEF08A;
  font-weight: 900;
  margin-right: 6px;
  white-space: normal;
  overflow-wrap: break-word;
}
.simple-formula-eq {
  color: #FFFFFF;
  background: #064E3B;
  border: 1px solid #34D399;
  border-radius: 6px;
  padding: 2px 8px;
  font-weight: 900;
  white-space: normal;
  overflow-wrap: break-word;
  display: inline-block;
  margin: 2px 0;
}
.simple-formula-desc {
  color: #FFFFFF;
  font-weight: 600;
  white-space: normal;
  overflow-wrap: break-word;
}
.lesson-6yo-step-btn,
.kid-action-btn,
.lesson-6yo-single-title,
.lesson-single-takeaway,
.lesson-separate-box-header {
  white-space: normal;
  overflow-wrap: break-word;
  word-break: break-word;
}
.lessons-6yo-bottom-nav {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 20px;
  width: 100%;
  padding-top: 4px;
}
.lesson-6yo-nav-arrow {
  background: #0F172A;
  color: #FFFFFF;
  border: 2px solid #38BDF8;
  border-radius: 14px;
  padding: 14px 26px;
  font-size: 17px;
  font-weight: 800;
  cursor: pointer;
  text-shadow: 0 1px 2px #000000;
}
.lesson-6yo-nav-arrow:hover {
  border-color: #FEF08A;
  color: #FEF08A;
}
.lesson-6yo-nav-arrow.primary-next {
  background: #064E3B;
  border: 2px solid #34D399;
  color: #FFFFFF;
}
.aero-6yo-spotlight-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: #070B14;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.aero-6yo-spotlight-overlay.hidden {
  display: none;
}
.spotlight-header-card {
  max-width: 960px;
  width: 100%;
  background: #0F172A;
  border: 2px solid #FACC15;
  border-radius: 20px;
  padding: 28px;
  box-shadow: 0 20px 50px #000000;
  text-align: center;
}
.spotlight-pointer-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 18px;
  margin: 22px 0;
  text-align: left;
}
@media (max-width: 850px) {
  .spotlight-pointer-grid {
    grid-template-columns: 1fr;
  }
}
.spotlight-pointer-card {
  background: #070B14;
  border: 2px solid #38BDF8;
  border-radius: 14px;
  padding: 18px;
}
.spotlight-arrow-bounce {
  font-size: 16px;
  font-weight: 900;
  color: #FEF08A;
  margin-bottom: 8px;
}
.spotlight-card-desc {
  font-size: 15px;
  color: #FFFFFF;
  line-height: 1.5;
}
.spotlight-close-btn {
  background: #064E3B;
  color: #FFFFFF;
  font-weight: 900;
  font-size: 16px;
  padding: 12px 26px;
  border-radius: 999px;
  border: 2px solid #34D399;
  cursor: pointer;
}
"#;

/// Interactive 60 FPS HTML5 Canvas 2D Engine for all 6 lessons (rendered cleanly at 760x340px).
const LESSONS_6YO_INTERACTIVE_JS: &str = r#"
(function() {
  if (window.__aeroLessons6YoInitialized) return;
  window.__aeroLessons6YoInitialized = true;

  var LOGICAL_W = 600;
  var LOGICAL_H = 260;

  var state = {
    step: 1,
    mode: 'l1_fog',
    fgDisplayMode: 'hud',
    tick: 0,
    mouseX: 300,
    mouseY: 130,
    mouseInside: false,
    darkSmoke: false,
    tiltBurstUntil: 0,
    slowLieStartTick: 0,
    cupLevel: 0.0
  };

  function setMegaphone(msg) {
    var el = document.getElementById('lessonMegaphoneBanner');
    if (el) el.innerHTML = msg;
  }

  function fillLessonStageBackground(ctx, w, h, solidColor) {
    window._lessonUsedRects = [];
    ctx.fillStyle = solidColor || '#071024';
    ctx.fillRect(0, 0, w, h);
  }

  window.triggerGreenFlightGearSolution = function() {
    var activeGroup = document.querySelector('.lesson-6yo-action-group.active');
    if (activeGroup) {
      var greenBtn = activeGroup.querySelector('.kid-action-btn.theme-green');
      if (greenBtn) {
        var code = greenBtn.getAttribute('data-action-code');
        if (code) window.setLessonMode(code, greenBtn);
      }
    }
    var vid = document.getElementById('fgLessonVideoPlayer');
    if (vid && typeof vid.play === 'function') {
      var p = vid.play();
      if (p && typeof p.catch === 'function') p.catch(function(){});
    }
  };

  window.toggleLessons6YoPanel = function() {
    var body = document.getElementById('lessons-6yo-collapsible-body');
    if (!body) return;
    body.classList.toggle('collapsed');
  };

  window.closeSpotlightGuide6Yo = function() {
    var ov = document.getElementById('aero-6yo-spotlight-overlay');
    if (ov) ov.classList.add('hidden');
  };

  window.selectLesson6YoStep = function(stepNum) {
    var s = parseInt(stepNum, 10);
    if (isNaN(s) || s < 1) s = 1;
    if (s > 6) s = 6;
    state.step = s;

    var tabs = document.querySelectorAll('.lesson-6yo-step-btn');
    for (var i = 0; i < tabs.length; i++) {
      var tStep = parseInt(tabs[i].getAttribute('data-step') || '0', 10);
      if (tStep === s) tabs[i].classList.add('active');
      else tabs[i].classList.remove('active');
    }

    var groups = document.querySelectorAll('.lesson-6yo-action-group');
    for (var j = 0; j < groups.length; j++) {
      var cStep = parseInt(groups[j].getAttribute('data-step') || '0', 10);
      if (cStep === s) {
        groups[j].classList.add('active');
        var titleEl = document.getElementById('lessonCurrentTitle');
        var newTitle = groups[j].getAttribute('data-title');
        if (titleEl && newTitle) titleEl.textContent = newTitle;

        var fgVideoUrl = groups[j].getAttribute('data-fg-video-url') || ('/real_videos/fg_lesson_' + s + '.mp4');
        var fgLessonVideoPlayer = document.getElementById('fgLessonVideoPlayer');
        if (fgLessonVideoPlayer) {
          if (fgLessonVideoPlayer.getAttribute('src') !== fgVideoUrl) {
            fgLessonVideoPlayer.src = fgVideoUrl;
            fgLessonVideoPlayer.setAttribute('src', fgVideoUrl);
          }
          if (typeof fgLessonVideoPlayer.play === 'function') {
            fgLessonVideoPlayer.play().catch(function(){});
          }
        }

        var greenBtnInGroup = groups[j].querySelector('.kid-action-btn.theme-green');
        var heroGreenBtn = document.getElementById('btnSolveWithFlightGearGreen');
        if (greenBtnInGroup && heroGreenBtn) {
          heroGreenBtn.textContent = greenBtnInGroup.textContent + ' (FlightGear Video 🎬)';
        }

        var defMode = groups[j].getAttribute('data-default-mode') || ('l' + s + '_default');
        var firstBtn = groups[j].querySelector('.kid-action-btn');
        window.setLessonMode(defMode, firstBtn);
      } else {
        groups[j].classList.remove('active');
      }
    }
  };
  window.select6YoLesson = window.selectLesson6YoStep;

  window.stepLesson6YoDelta = function(delta) {
    var next = state.step + delta;
    if (next < 1) next = 6;
    if (next > 6) next = 1;
    window.selectLesson6YoStep(next);
  };

  window.setLessonMode = function(actionCode, btnElem) {
    state.mode = actionCode;

    if (btnElem && btnElem.parentElement) {
      var siblings = btnElem.parentElement.querySelectorAll('.kid-action-btn');
      for (var i = 0; i < siblings.length; i++) {
        siblings[i].classList.remove('active-mode');
      }
      btnElem.classList.add('active-mode');
    }

    if (actionCode === 'l1_fog') {
      setMegaphone('🌫️ Norul de ceață acoperă pista — dar Musculița Google (Drosophila 🐝) și AERO-SHIELD știu drumul!');
    } else if (actionCode === 'l1_hacker') {
      setMegaphone('👾 Hackerul trimite un semnal GPS mincinos și trage avionul pe linia roșie spre pădure!');
    } else if (actionCode === 'l1_shield') {
      setMegaphone('🛡️ AERO-SHIELD + Creierul Musculiței Google 🐝 taie firul hackerului cu Foarfeca Aurie ✂️!');
    } else if (actionCode === 'l2_fog_smoke') {
      state.darkSmoke = !state.darkSmoke;
      setMegaphone(state.darkSmoke
        ? '🖤 Fum negru dens: lumina normală se oprește, dar Lupa SWIR și Ochii Magici T4/T5 văd direct prin el!'
        : '🌫️ Ceață albă groasă: mișcă mouse-ul pe desen ca să privești prin Lupa Infraroșie SWIR + T4/T5!');
    } else if (actionCode === 'l2_swir') {
      setMegaphone('🔦 Mișcă mouse-ul: Ochii Magici T4/T5 ai Musculiței Google văd becurile calde prin Lupa SWIR!');
    } else if (actionCode === 'l2_waves') {
      setMegaphone('💧 Unda mică se lovește de picături, dar unda lungă SWIR pășește peste ele direct spre neuronii T4/T5!');
    } else if (actionCode === 'l3_slow') {
      setMegaphone('🐢 Zbor încet (45 m/s): neuronii T4/T5 numără săgeți scurte, iar Inelul-Busolă E-PG cu 16 Luminițe stă pe RWY 09!');
    } else if (actionCode === 'l3_fast') {
      setMegaphone('🚀 Zbor rapid (120 m/s): neuronii T4/T5 numără săgeți galbene lungi fără niciun satelit GPS!');
    } else if (actionCode === 'l3_ruler') {
      setMegaphone('📏 Rigla T4/T5 măsoară viteza, iar Busola în Inel E-PG cu 16 Luminițe 🐝 arată direcția spre Craiova RWY 09!');
    } else if (actionCode === 'l4_wind') {
      setMegaphone('💨 Senzorul de Vânt PFNd simte rafala pe aripi/antene cu 1 secundă înainte să împingă avionul!');
    } else if (actionCode === 'l4_tilt') {
      state.tiltBurstUntil = state.tick + 65;
      setMegaphone('📢 Stabilizatorul HS/VS (6 direcții): „Doar s-a înclinat avionul, nu ne-am mutat de pe pistă!”');
    } else if (actionCode === 'l4_separate') {
      setMegaphone('🧠 Stabilizatorul HS/VS + Senzorul de Vânt PFNd taie ❌ Norul Mov și păstrează doar ✅ Pământul Verde!');
    } else if (actionCode === 'l5_honest') {
      state.cupLevel = 0.0;
      setMegaphone('✅ Zbor cinstit: Inelul-Busolă E-PG cu 16 Luminițe 🐝 și Cântarul Adevărului țin direcția RWY 09!');
    } else if (actionCode === 'l5_big_lie') {
      state.cupLevel = 1.0;
      setMegaphone('⚡ Minciuna mare (+500m) răstoarnă Cântarul, dar Inelul E-PG cu 16 Luminițe ține minte drumul și cu ochii închiși!');
    } else if (actionCode === 'l5_slow_lie') {
      state.slowLieStartTick = state.tick;
      state.cupLevel = 0.05;
      setMegaphone('🐍 Minciuna lentă umple Paharul de Minciuni (CUSUM) în 3 secunde — Inelul E-PG prinde hoțul -> ALARMĂ!');
    } else if (actionCode === 'l6_spotlight') {
      var ov = document.getElementById('aero-6yo-spotlight-overlay');
      if (ov) ov.classList.remove('hidden');
      setMegaphone('✨ Privește Săgețile Galbene pe Ecranul Mare și Harta Creierului Google Drosophila!');
    } else if (actionCode === 'l6_real_video') {
      setMegaphone('🎬 Am pornit demonstrația cu Filmarea Reală din Cockpit și Creierul Drosophila pe simulatorul mare!');
      if (typeof window.startRealFootageDemo === 'function') {
        window.startRealFootageDemo();
      } else {
        var realBtn = document.querySelector('[data-action="real-video"], #btn-real-video, #btnRealVideoDemo');
        if (realBtn && typeof realBtn.click === 'function') realBtn.click();
      }
    } else if (actionCode === 'l6_diploma') {
      setMegaphone('🏅 Felicitări, Comandante! Stăpânești Harta Google Drosophila (T4/T5 → HS/VS → E-PG 16 LED → PFNd)!');
    }
  };

  function setupCanvasEvents() {
    var canvas = document.getElementById('lessonInteractiveCanvas');
    if (!canvas) return;
    function updatePos(clientX, clientY) {
      var rect = canvas.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      state.mouseX = ((clientX - rect.left) / rect.width) * LOGICAL_W;
      state.mouseY = ((clientY - rect.top) / rect.height) * LOGICAL_H;
      state.mouseInside = true;
    }
    canvas.addEventListener('mousemove', function(e) {
      updatePos(e.clientX, e.clientY);
    });
    canvas.addEventListener('mouseleave', function() {
      state.mouseInside = false;
    });
    canvas.addEventListener('touchmove', function(e) {
      if (e.touches && e.touches.length > 0) {
        updatePos(e.touches[0].clientX, e.touches[0].clientY);
      }
    }, { passive: true });
  }

  function drawAirplaneIcon(ctx, x, y, angleRad, color) {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(angleRad);
    ctx.fillStyle = color || '#38bdf8';
    ctx.beginPath();
    ctx.moveTo(22, 0);
    ctx.lineTo(-14, -12);
    ctx.lineTo(-8, 0);
    ctx.lineTo(-14, 12);
    ctx.closePath();
    ctx.fill();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.restore();
  }

  // Utilitar WCAG AAA: Casetă 100% opacă #070B14 cu bordură 2px, anti-coliziune și clamping în [8..LOGICAL_W-8]
  function drawStaticBadge(ctx, cx, cy, text, borderColor, isYellowText) {
    ctx.save();
    var str = String(text || '');
    var padX = 10;
    var maxBoxW = LOGICAL_W - 16;
    var fontPx = 12;
    ctx.font = 'bold ' + fontPx + 'px sans-serif';
    var textW = Math.ceil(ctx.measureText(str).width);
    while (textW + padX * 2 > maxBoxW && fontPx > 9.5) {
      fontPx -= 0.5;
      ctx.font = 'bold ' + fontPx + 'px sans-serif';
      textW = Math.ceil(ctx.measureText(str).width);
    }
    var boxW = Math.min(maxBoxW, textW + padX * 2);
    var boxH = 24;
    var bx = Math.max(8, Math.min(LOGICAL_W - boxW - 8, Math.round(cx - boxW / 2)));
    var by = Math.max(6, Math.min(LOGICAL_H - boxH - 6, Math.round(cy - boxH / 2)));

    if (!window._lessonUsedRects) window._lessonUsedRects = [];
    var used = window._lessonUsedRects;
    var stepDir = (cy > LOGICAL_H * 0.65) ? -1 : 1;
    for (var attempt = 0; attempt < 8; attempt++) {
      var hit = false;
      for (var k = 0; k < used.length; k++) {
        var r = used[k];
        if (!(bx + boxW + 2 < r.x || bx - 2 > r.x + r.w || by + boxH + 2 < r.y || by - 2 > r.y + r.h)) {
          hit = true;
          break;
        }
      }
      if (!hit) break;
      by += stepDir * (boxH + 4);
      if (by + boxH > LOGICAL_H - 4) {
        stepDir = -1;
        by = Math.max(4, LOGICAL_H - boxH - 4);
      } else if (by < 4) {
        stepDir = 1;
        by = 4;
      }
    }
    by = Math.max(4, Math.min(LOGICAL_H - boxH - 4, by));
    used.push({ x: bx, y: by, w: boxW, h: boxH });

    ctx.fillStyle = '#070B14';
    ctx.fillRect(bx, by, boxW, boxH);
    ctx.strokeStyle = borderColor || '#38BDF8';
    ctx.lineWidth = 2;
    ctx.strokeRect(bx, by, boxW, boxH);

    ctx.fillStyle = isYellowText ? '#FEF08A' : '#FFFFFF';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(str, bx + boxW / 2, by + boxH / 2 + 1, boxW - 12);
    ctx.restore();
  }

  // Widget Vizual Animat: „🐝 Creierul Google Drosophila: Inelul-Busolă E-PG cu 16 Luminițe”
  function drawDrosophilaEpgRingWidget(ctx, cx, cy, r, t, activeWedge, showTitle) {
    ctx.save();
    if (!window._lessonUsedRects) window._lessonUsedRects = [];
    if (showTitle) {
      var rx = cx - r - 32;
      var ry = cy - r - 28;
      var rw = (r + 32) * 2;
      var rh = (r + 24) * 2;
      ctx.fillStyle = '#070B14';
      ctx.strokeStyle = '#FACC15';
      ctx.lineWidth = 2;
      ctx.fillRect(rx, ry, rw, rh);
      ctx.strokeRect(rx, ry, rw, rh);
      drawStaticBadge(ctx, cx, cy - r - 14, '🐝 E-PG 16 LUMINIȚE', '#FACC15', true);
      window._lessonUsedRects.push({ x: rx, y: ry, w: rw, h: rh });
    } else {
      window._lessonUsedRects.push({ x: cx - r - 8, y: cy - r - 8, w: (r + 8) * 2, h: (r + 8) * 2 });
    }

    ctx.strokeStyle = '#38BDF8';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.stroke();

    var targetIdx = ((activeWedge % 16) + 16) % 16;
    for (var i = 0; i < 16; i++) {
      var ang = (i / 16) * Math.PI * 2 - Math.PI / 2;
      var nx = cx + Math.cos(ang) * r;
      var ny = cy + Math.sin(ang) * r;
      var dist = Math.abs(i - targetIdx);
      if (dist > 8) dist = 16 - dist;

      var isPeak = (dist === 0);
      var isNeighbor = (dist === 1);
      ctx.beginPath();
      ctx.arc(nx, ny, isPeak ? 5.2 : (isNeighbor ? 3.8 : 2.4), 0, Math.PI * 2);
      if (isPeak) {
        ctx.fillStyle = (t % 16 < 8) ? '#34D399' : '#FEF08A';
      } else if (isNeighbor) {
        ctx.fillStyle = '#22C55E';
      } else {
        ctx.fillStyle = '#1E293B';
      }
      ctx.fill();

      if (isPeak) {
        ctx.strokeStyle = '#FEF08A';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.lineTo(nx, ny);
        ctx.stroke();
      }
    }

    drawStaticBadge(ctx, cx, cy, 'RWY 09', '#34D399', true);
    ctx.restore();
  }

  function drawLesson1(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    ctx.fillStyle = '#0F172A';
    ctx.fillRect(450, 45, 120, 75);
    ctx.strokeStyle = '#34D399';
    ctx.lineWidth = 2;
    ctx.strokeRect(450, 45, 120, 75);
    window._lessonUsedRects.push({ x: 450, y: 45, w: 120, h: 75 });
    drawStaticBadge(ctx, 495, 24, '🛬 PISTA CRAIOVA RWY 09', '#34D399', true);

    for (var i = 0; i < 5; i++) {
      ctx.fillStyle = (t % 20 < 10) ? '#34D399' : '#FEF08A';
      ctx.beginPath();
      ctx.arc(465 + i * 22, 82, 4, 0, Math.PI * 2);
      ctx.fill();
    }

    if (state.mode === 'l1_hacker' || state.mode === 'l1_shield') {
      ctx.save();
      ctx.fillStyle = '#450A0A';
      ctx.fillRect(430, 180, 140, 56);
      ctx.strokeStyle = '#F87171';
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 4]);
      ctx.strokeRect(430, 180, 140, 56);
      ctx.setLineDash([]);
      ctx.restore();
      window._lessonUsedRects.push({ x: 430, y: 180, w: 140, h: 56 });
      drawStaticBadge(ctx, 485, 158, '👻 PISTA FALSĂ (PĂDURE!)', '#F87171', false);
    }

    if (state.mode === 'l1_hacker') {
      ctx.strokeStyle = '#EF4444';
      ctx.lineWidth = 3.5;
      ctx.setLineDash([8, 6]);
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.quadraticCurveTo(260, 195, 430, 205);
      ctx.stroke();
      ctx.setLineDash([]);
      drawStaticBadge(ctx, 210, 22, '👾 HACKER GPS: Trage avionul pe linia roșie!', '#F87171', true);
    } else if (state.mode === 'l1_shield') {
      ctx.strokeStyle = '#7F1D1D';
      ctx.lineWidth = 2.5;
      ctx.setLineDash([6, 6]);
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(250, 165);
      ctx.stroke();
      ctx.setLineDash([]);

      drawStaticBadge(ctx, 250, 165, '✂️ TĂIAT!', '#FACC15', true);

      ctx.strokeStyle = '#22C55E';
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(450, 82);
      ctx.stroke();

      drawStaticBadge(ctx, 235, 22, '🛡️ AERO-SHIELD + Musculița Google 🐝 taie minciuna!', '#34D399', true);
      drawDrosophilaEpgRingWidget(ctx, 92, 198, 26, t, 4, true);
    } else {
      ctx.strokeStyle = '#38BDF8';
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(450, 82);
      ctx.stroke();
      drawStaticBadge(ctx, 210, 22, '🌫️ ZBOR ÎN CEAȚĂ SPRE PISTA RWY 09', '#38BDF8', false);
    }

    ctx.save();
    ctx.fillStyle = '#334155';
    ctx.strokeStyle = '#94A3B8';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(265, 92, 42, 0, Math.PI * 2);
    ctx.arc(310, 82, 48, 0, Math.PI * 2);
    ctx.arc(350, 96, 38, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    ctx.restore();
    drawStaticBadge(ctx, 310, 92, '🌫️ CEAȚĂ GROASĂ', '#38BDF8', true);

    var prog = (t % 140) / 140;
    var ax = 80 + prog * 130;
    var ay = (state.mode === 'l1_hacker') ? (95 + prog * 70) : 90;
    var ang = (state.mode === 'l1_hacker') ? 0.35 : -0.03;
    drawAirplaneIcon(ctx, ax, ay, ang, state.mode === 'l1_shield' ? '#34D399' : '#38BDF8');
  }

  function drawLesson2(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    if (state.mode === 'l2_waves') {
      drawStaticBadge(ctx, 295, 24, '👀 Lumina Obișnuită (Valuri Mici) -> Se lovește de picături!', '#38BDF8', false);

      for (var d = 0; d < 6; d++) {
        ctx.fillStyle = '#0284C7';
        ctx.beginPath();
        ctx.arc(280 + d * 42, 72 + (d % 2) * 16, 11, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = '#FFFFFF';
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }

      ctx.strokeStyle = '#38BDF8';
      ctx.lineWidth = 3;
      ctx.beginPath();
      for (var x = 30; x < 265; x += 4) {
        var y = 78 + Math.sin((x + t * 4) * 0.25) * 14;
        if (x === 30) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      drawStaticBadge(ctx, 165, 52, '💥 Nu trece de ceață!', '#F87171', true);

      ctx.strokeStyle = '#334155';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(20, 130);
      ctx.lineTo(w - 20, 130);
      ctx.stroke();

      drawStaticBadge(ctx, 300, 154, '🔦 Lupa SWIR + Ochii Magici T4/T5 ai Musculiței Google 🐝 -> Văd pista!', '#34D399', true);

      for (var d2 = 0; d2 < 6; d2++) {
        ctx.fillStyle = '#1E293B';
        ctx.strokeStyle = '#38BDF8';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.arc(280 + d2 * 42, 208 + (d2 % 2) * 14, 10, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      }

      ctx.strokeStyle = '#22C55E';
      ctx.lineWidth = 4;
      ctx.beginPath();
      for (var x2 = 30; x2 < 540; x2 += 5) {
        var y2 = 208 + Math.sin((x2 - t * 4) * 0.055) * 20;
        if (x2 === 30) ctx.moveTo(x2, y2);
        else ctx.lineTo(x2, y2);
      }
      ctx.stroke();
      drawStaticBadge(ctx, 485, 238, '🐝 T4/T5 VEDE PISTA!', '#FACC15', true);
      return;
    }

    var lensX = state.mouseInside ? state.mouseX : (300 + Math.cos(t * 0.04) * 130);
    var lensY = state.mouseInside ? state.mouseY : (135 + Math.sin(t * 0.06) * 40);

    ctx.fillStyle = state.darkSmoke ? '#0F172A' : '#1E293B';
    ctx.fillRect(20, 20, w - 40, h - 40);
    ctx.strokeStyle = '#38BDF8';
    ctx.lineWidth = 2;
    ctx.strokeRect(20, 20, w - 40, h - 40);

    ctx.save();
    ctx.beginPath();
    ctx.arc(lensX, lensY, 74, 0, Math.PI * 2);
    ctx.clip();

    ctx.fillStyle = '#070B14';
    ctx.fillRect(0, 0, w, h);

    ctx.strokeStyle = '#22C55E';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(300, 45);
    ctx.lineTo(130, 235);
    ctx.moveTo(300, 45);
    ctx.lineTo(470, 235);
    ctx.stroke();

    for (var r = 0; r < 6; r++) {
      var ry = 70 + r * 28;
      var spread = 22 + r * 24;
      ctx.fillStyle = '#34D399';
      ctx.beginPath();
      ctx.arc(300 - spread, ry, 4.5, 0, Math.PI * 2);
      ctx.arc(300 + spread, ry, 4.5, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#FEF08A';
      ctx.fillRect(297, ry - 6, 6, 14);
    }

    ctx.restore();

    ctx.strokeStyle = '#34D399';
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.arc(lensX, lensY, 74, 0, Math.PI * 2);
    ctx.stroke();

    // Etichete 100% STATICE (nu se mișcă cu lupa!)
    drawStaticBadge(
      ctx,
      300,
      36,
      state.darkSmoke
        ? '🖤 FUM NEGRU — Plimbă Lupa SWIR + Ochii T4/T5 cu mouse-ul!'
        : '🌫️ CEAȚĂ DENSĂ — Plimbă Lupa SWIR + Ochii T4/T5 cu mouse-ul!',
      '#FACC15',
      true
    );
    drawStaticBadge(ctx, 300, 236, '🔦 CERCUL VERDE = LUPA SWIR + 🐝 OCHII T4/T5 (Văd Pista!)', '#34D399', false);
  }

  function drawLesson3(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    var speedPx = (state.mode === 'l3_fast') ? 5.2 : 1.8;
    var stepDist = (state.mode === 'l3_fast') ? 78 : 28;

    drawAirplaneIcon(ctx, 230, 42, 0, '#38BDF8');
    ctx.fillStyle = '#0F172A';
    ctx.strokeStyle = '#38BDF8';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(230, 52);
    ctx.lineTo(90, 210);
    ctx.lineTo(370, 210);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = '#0F172A';
    ctx.fillRect(20, 185, 410, 42);
    ctx.strokeStyle = '#38BDF8';
    ctx.lineWidth = 2;
    ctx.strokeRect(20, 185, 410, 42);

    var offset = (t * speedPx) % 90;
    for (var i = -1; i < 6; i++) {
      var bx = 410 - (i * 80 + offset);
      if (bx < 30 || bx > 415) continue;

      var prevX = bx + stepDist;
      if (prevX < 420) {
        ctx.fillStyle = '#334155';
        ctx.beginPath();
        ctx.arc(prevX, 206, 7, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = '#FEF08A';
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.moveTo(prevX, 168);
        ctx.lineTo(bx, 168);
        ctx.stroke();

        ctx.fillStyle = '#FEF08A';
        ctx.beginPath();
        ctx.moveTo(bx, 168);
        ctx.lineTo(bx + 8, 163);
        ctx.lineTo(bx + 8, 173);
        ctx.closePath();
        ctx.fill();
      }

      ctx.fillStyle = '#34D399';
      ctx.beginPath();
      ctx.arc(bx, 206, 8, 0, Math.PI * 2);
      ctx.fill();
    }

    if (state.mode === 'l3_ruler') {
      drawStaticBadge(ctx, 228, 124, '📏 T4/T5 EMD: Δu = 52 px -> 85 m/s!', '#FACC15', true);
    }

    // Draw the 16-LED E-PG Ring Attractor Compass on the right side of the canvas
    drawDrosophilaEpgRingWidget(ctx, 512, 128, 44, t, 4, true);

    var label = (state.mode === 'l3_fast')
      ? '🚀 T4/T5 VITEZĂ MARE (120 m/s) + Inel E-PG 16 Luminițe'
      : '🐢 T4/T5 VITEZĂ MICĂ (45 m/s) + Inel E-PG 16 Luminițe';
    drawStaticBadge(ctx, 225, 242, label, '#34D399', true);
  }

  function drawLesson4(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    var isTilting = (t < state.tiltBurstUntil);
    var tiltAngle = isTilting ? Math.sin(t * 0.45) * 0.28 : 0.0;

    drawAirplaneIcon(ctx, 88, 30, tiltAngle, '#38BDF8');
    drawStaticBadge(ctx, 345, 28, '🐝 Stabilizator HS/VS (6 direcții) + Senzor Vânt PFNd', '#FACC15', true);

    var cloudShift = (t * 2.4) % 80;
    ctx.fillStyle = '#1E1B4B';
    ctx.strokeStyle = '#C084FC';
    ctx.lineWidth = 2;
    ctx.fillRect(40, 82, w - 80, 64);
    ctx.strokeRect(40, 82, w - 80, 64);

    drawStaticBadge(ctx, 300, 98, '☁️ STRATUL 1: NORUL MOV ÎMPINS DE VÂNT 💨 (Tăiat de PFNd ❌)', '#C084FC', false);

    for (var c = 0; c < 5; c++) {
      var cx = 75 + ((c * 95 + cloudShift) % 450);
      ctx.strokeStyle = '#F87171';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(cx, 132);
      ctx.lineTo(cx + 32, 122);
      ctx.stroke();
    }

    ctx.fillStyle = '#064E3B';
    ctx.strokeStyle = '#34D399';
    ctx.lineWidth = 2;
    ctx.fillRect(40, 166, w - 80, 68);
    ctx.strokeRect(40, 166, w - 80, 68);

    drawStaticBadge(ctx, 300, 184, '🌍 STRATUL 2: PĂMÂNTUL VERDE FIX ✅ (Derotat de neuronii HS/VS!)', '#34D399', true);

    var groundShift = (t * 1.8) % 90;
    for (var g = 0; g < 5; g++) {
      var gx = 520 - ((g * 90 + groundShift) % 450);
      ctx.strokeStyle = '#FEF08A';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(gx, 216);
      ctx.lineTo(gx - 28, 216);
      ctx.stroke();
    }

    if (isTilting) {
      drawStaticBadge(ctx, 300, 60, '📢 HS/VS: „Doar s-a înclinat avionul în 6 direcții, nu am sărit de pe pistă!”', '#FACC15', true);
    }
  }

  function drawLesson5(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    if (state.mode === 'l5_slow_lie') {
      var elapsed = t - state.slowLieStartTick;
      state.cupLevel = Math.min(1.0, elapsed / 180.0);
    }

    var isAlarm = (state.mode === 'l5_big_lie') || (state.cupLevel >= 0.99);

    var scaleCx = 145;
    var scaleCy = 132;
    var beamTilt = (state.mode === 'l5_big_lie') ? 0.42 : (state.mode === 'l5_slow_lie' ? 0.08 : 0.0);

    drawStaticBadge(ctx, 145, 26, '⚖️ 1. CÂNTARUL ADEVĂRULUI', '#38BDF8', true);

    ctx.fillStyle = '#334155';
    ctx.fillRect(scaleCx - 6, scaleCy, 12, 72);
    ctx.fillRect(scaleCx - 45, scaleCy + 68, 90, 10);

    // Doar brațul grafic al balanței se rotește; textele rămân 100% STATICE!
    ctx.save();
    ctx.translate(scaleCx, scaleCy);
    ctx.rotate(beamTilt);
    ctx.strokeStyle = isAlarm ? '#F87171' : '#FEF08A';
    ctx.lineWidth = 5;
    ctx.beginPath();
    ctx.moveTo(-75, 0);
    ctx.lineTo(75, 0);
    ctx.stroke();

    ctx.fillStyle = '#064E3B';
    ctx.strokeStyle = '#34D399';
    ctx.lineWidth = 2;
    ctx.fillRect(-95, 14, 44, 16);
    ctx.strokeRect(-95, 14, 44, 16);

    ctx.fillStyle = isAlarm ? '#7F1D1D' : '#0C4A6E';
    ctx.strokeStyle = isAlarm ? '#F87171' : '#38BDF8';
    ctx.fillRect(51, 14, 44, 16);
    ctx.strokeRect(51, 14, 44, 16);
    ctx.restore();

    // Etichete 100% statice pentru talerele balanței
    drawStaticBadge(ctx, 78, 192, '🐝 E-PG+IMU', '#34D399', false);
    drawStaticBadge(ctx, 212, 192, state.mode === 'l5_big_lie' ? '⚡ GPS +500m!' : '📡 GPS', isAlarm ? '#F87171' : '#38BDF8', isAlarm);

    // 2. Center Widget: 16-LED E-PG Ring Attractor Compass keeping RWY 09 memory
    drawDrosophilaEpgRingWidget(ctx, 325, 132, 40, t, 4, true);

    var cupX = 455;
    var cupY = 64;
    var cupW = 92;
    var cupH = 135;

    drawStaticBadge(ctx, 501, 26, '🥛 3. PAHARUL CUSUM', '#FACC15', true);

    if (state.mode === 'l5_slow_lie' && state.cupLevel < 1.0) {
      var dropY = 44 + ((t * 4) % 40);
      ctx.fillStyle = '#EF4444';
      ctx.beginPath();
      ctx.arc(cupX + cupW / 2, dropY, 6, 0, Math.PI * 2);
      ctx.fill();
    }

    var fillH = Math.round(cupH * state.cupLevel);
    if (fillH > 0) {
      ctx.fillStyle = isAlarm ? '#DC2626' : '#D97706';
      ctx.fillRect(cupX + 4, cupY + cupH - fillH, cupW - 8, fillH - 2);
    }

    ctx.strokeStyle = '#FFFFFF';
    ctx.lineWidth = 3;
    ctx.strokeRect(cupX, cupY, cupW, cupH);

    ctx.strokeStyle = '#F87171';
    ctx.setLineDash([5, 4]);
    ctx.beginPath();
    ctx.moveTo(cupX - 10, cupY + 18);
    ctx.lineTo(cupX + cupW + 10, cupY + 18);
    ctx.stroke();
    ctx.setLineDash([]);

    if (isAlarm) {
      drawStaticBadge(ctx, 300, 240, '🚨 ALARMĂ! Minciuna e tăiată ✂️ — Inelul E-PG ține minte RWY 09!', '#F87171', true);
    } else {
      drawStaticBadge(ctx, 300, 240, '✅ Echilibru + Inel E-PG 16 Luminițe (Pahar CUSUM: ' + Math.round(state.cupLevel * 100) + '%)', '#34D399', false);
    }
  }

  function drawLesson6(ctx, w, h, t) {
    fillLessonStageBackground(ctx, w, h, '#070B14');

    ctx.strokeStyle = '#FACC15';
    ctx.lineWidth = 3;
    ctx.strokeRect(16, 12, w - 32, h - 24);

    drawStaticBadge(ctx, 300, 28, '🏆 HARTA CREIERULUI GOOGLE DROSOPHILA 🐝 + AERO-SHIELD V2.0', '#FACC15', true);

    // Top pipeline: T4/T5 -> HS/VS -> E-PG 16 LED -> PFNd Vânt (100% static & opaque)
    var stages = [
      { x: 32, label: '👀 1. Ochi T4/T5', sub: 'Numără becurile' },
      { x: 162, label: '🔄 2. HS/VS 6-DOF', sub: 'Scade înclinarea' },
      { x: 292, label: '🧭 3. Inel E-PG (16)', sub: 'Busola cu memorie' },
      { x: 422, label: '💨 4. Senzor PFNd', sub: 'Simte vântul (-1s)' }
    ];
    for (var s = 0; s < stages.length; s++) {
      var st = stages[s];
      ctx.fillStyle = '#0F172A';
      ctx.strokeStyle = '#34D399';
      ctx.lineWidth = 2;
      ctx.fillRect(st.x, 48, 118, 44);
      ctx.strokeRect(st.x, 48, 118, 44);
      ctx.fillStyle = '#FEF08A';
      ctx.font = 'bold 11px sans-serif';
      ctx.fillText(st.label, st.x + 6, 66, 106);
      ctx.fillStyle = '#FFFFFF';
      ctx.font = 'bold 10px sans-serif';
      ctx.fillText(st.sub, st.x + 6, 82, 106);
    }

    var boxes = [
      { x: 32, title: '1. PARBRIZUL MAGIC', sub: 'SWIR + Neuronii T4/T5' },
      { x: 192, title: '2. CURSA 4 AVIOANE', sub: 'Drosophila vs. Hacker' },
      { x: 352, title: '3. TELECOMANDA', sub: 'Cântar + Pahar CUSUM' }
    ];

    for (var i = 0; i < boxes.length; i++) {
      var b = boxes[i];
      ctx.fillStyle = '#0F172A';
      ctx.strokeStyle = '#38BDF8';
      ctx.lineWidth = 2;
      ctx.fillRect(b.x, 108, 146, 68);
      ctx.strokeRect(b.x, 108, 146, 68);
      window._lessonUsedRects.push({ x: b.x, y: 108, w: 146, h: 68 });

      ctx.fillStyle = '#FEF08A';
      ctx.font = 'bold 11.5px sans-serif';
      ctx.fillText(b.title, b.x + 8, 132, 130);
      ctx.fillStyle = '#FFFFFF';
      ctx.font = 'bold 10.5px sans-serif';
      ctx.fillText(b.sub, b.x + 8, 154, 130);

      drawStaticBadge(ctx, b.x + 73, 198, '⬇️ VEZI JOS', '#FACC15', true);
    }

    drawDrosophilaEpgRingWidget(ctx, 536, 152, 28, t, Math.floor(t / 25) % 16, false);

    drawStaticBadge(ctx, 300, 232, '👇 Privește mai jos pe Ecranul Mare al Simulatorului! 👇', '#34D399', true);
  }

  function renderLoop() {
    state.tick += 1;
    var canvas = document.getElementById('lessonInteractiveCanvas');
    if (canvas && canvas.getContext) {
      var ctx = canvas.getContext('2d');
      if (ctx) {
        var cw = canvas.width || 760;
        var ch = canvas.height || 340;
        ctx.save();
        ctx.scale(cw / LOGICAL_W, ch / LOGICAL_H);
        if (state.step === 1) drawLesson1(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        else if (state.step === 2) drawLesson2(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        else if (state.step === 3) drawLesson3(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        else if (state.step === 4) drawLesson4(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        else if (state.step === 5) drawLesson5(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        else drawLesson6(ctx, LOGICAL_W, LOGICAL_H, state.tick);
        ctx.restore();
      }
    }
    window.requestAnimationFrame(renderLoop);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function() {
      setupCanvasEvents();
      window.requestAnimationFrame(renderLoop);
    });
  } else {
    setupCanvasEvents();
    window.requestAnimationFrame(renderLoop);
  }
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_6yo_lessons_has_six_complete_steps() {
        let lessons = get_all_6yo_lessons();
        assert_eq!(lessons.len(), 6);
        for (idx, lesson) in lessons.iter().enumerate() {
            assert_eq!(lesson.step_number as usize, idx + 1);
            assert!(!lesson.title.is_empty());
            assert!(!lesson.kid_story_ro.is_empty());
            assert!(!lesson.visual_rule_ro.is_empty());
            assert!(!lesson.science_secret_ro.is_empty());
            let expected_fg_url = format!("/real_videos/fg_lesson_{}.mp4", idx + 1);
            assert_eq!(lesson.fg_video_url, expected_fg_url);
            assert_eq!(lesson.action_buttons.len(), 3);
        }
    }

    #[test]
    fn test_render_lessons_6yo_component_contains_spacious_760x340_canvas_and_all_lessons() {
        let html = render_lessons_6yo_component();
        assert!(html.contains("id=\"lessonAnimationSeparateBox\""));
        assert!(html.contains("id=\"lessonVideoSeparateBox\""));
        assert!(html.contains("🎨 Animația Interactivă a Lecției (Pas cu Pas)"));
        assert!(html.contains("🎬 Videoclipul Zborului Sintetic FlightGear (Rezolvare 3D)"));
        assert!(html.contains("id=\"fgLessonVideoPlayer\""));
        assert!(html.contains("id=\"btnSolveWithFlightGearGreen\""));
        assert!(html.contains("/real_videos/fg_lesson_1.mp4"));
        assert!(html.contains("/real_videos/fg_lesson_2.mp4"));
        assert!(html.contains("/real_videos/fg_lesson_3.mp4"));
        assert!(html.contains("/real_videos/fg_lesson_4.mp4"));
        assert!(html.contains("/real_videos/fg_lesson_5.mp4"));
        assert!(html.contains("/real_videos/fg_lesson_6.mp4"));
        assert!(html.contains("🟢 BUTONUL VERDE"));
        assert!(html.contains("id=\"lessonInteractiveCanvas\""));
        assert!(html.contains("width=\"760\""));
        assert!(html.contains("height=\"340\""));
        assert!(html.contains("id=\"lessonActionButtons\""));
        assert!(html.contains("Lecția 1:"));
        assert!(html.contains("Lecția 2:"));
        assert!(html.contains("Lecția 3:"));
        assert!(html.contains("Lecția 4:"));
        assert!(html.contains("Lecția 5:"));
        assert!(html.contains("Lecția 6:"));
        assert!(html.contains("Drosophila"));
        assert!(html.contains("T4/T5"));
        assert!(html.contains("E-PG"));
        assert!(html.contains("PFNd"));
        assert!(html.contains("drawDrosophilaEpgRingWidget"));
        assert!(html.contains("drawStaticBadge"));
        assert!(html.contains("id=\"simple3Formulas100kCard\""));
        assert!(html.contains("100.000 KM"));
        assert!(html.contains("Formula 1 (Cheia Pământului)"));
        assert!(html.contains("Formula 2 (Creierul Musculiței Google)"));
        assert!(html.contains("Formula 3 (Detectorul de Minciuni)"));
        assert!(html.contains("Formula 4 (Busola Cerului"));
        assert!(html.contains("Camera Infraroșu de Sus"));
        assert!(html.contains("SR-71 Blackbird"));
        assert!(html.contains("⬅️ Lecția Anterioară"));
        assert!(html.contains("➡️ Lecția Următoare"));
    }

    #[test]
    fn test_render_lessons_6yo_standalone_page_is_valid_html5_document() {
        let page = render_lessons_6yo_standalone_page();
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.contains("<html lang=\"ro\">"));
        assert!(page.contains("id=\"lessonInteractiveCanvas\""));
        assert!(page.ends_with("</html>\n"));
    }

    #[test]
    fn test_json_serialization_of_lessons_deck() {
        let lessons = get_all_6yo_lessons();
        let serialized = serde_json::to_string(&lessons);
        assert!(serialized.is_ok());
        let json_str = serialized.unwrap_or_default();
        assert!(json_str.contains("l1_shield"));
        assert!(json_str.contains("l2_swir"));
        assert!(json_str.contains("l3_ruler"));
        assert!(json_str.contains("l4_separate"));
        assert!(json_str.contains("l5_slow_lie"));
        assert!(json_str.contains("l6_spotlight"));
    }
}

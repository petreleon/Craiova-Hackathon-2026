//! # AERO-SHIELD LRCV V2.0 — Native Rust SSR Module for 6-Year-Old & Beginner Interactive Academy
//!
//! This module implements a strongly-typed Server-Side Rendering (SSR) DOM builder (`HtmlComponent`,
//! `DomElement`, `rsx_elem!`) and the complete 6-Lesson Interactive Aviation Academy in Romanian
//! for 6-year-olds and absolute beginners (`InteractiveLesson6Yo`, `LessonActionButton`, `Lessons6YoDeck`).
//!
//! ## Safety & Avionics Standards (DO-178C Level A Philosophy)
//! - **Zero `.unwrap()` / `.expect()` / `panic!()`** in production code.
//! - Deterministic HTML escaping and pure Rust SSR tree construction.
//! - Exposes ready-to-mount `axum` routes (`/api/lessons-6yo`, `/api/lessons-6yo/html`, `/lessons-6yo`).

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

        let emoji_span = rsx_elem!(
            "span",
            [("class", "kid-btn-emoji")],
            text: self.emoji
        );
        let label_span = rsx_elem!(
            "span",
            [("class", "kid-btn-title")],
            text: self.label
        );
        let sub_span = rsx_elem!(
            "span",
            [("class", "kid-btn-sub")],
            text: self.subtitle
        );
        let text_wrap = rsx_elem!(
            "span",
            [("class", "kid-btn-text-wrap")],
            [label_span, sub_span]
        );
        let btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", self.id),
                ("class", btn_class),
                ("data-action-code", self.action_code),
                ("onclick", onclick_js)
            ],
            [emoji_span, text_wrap]
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
    pub action_buttons: Vec<LessonActionButton>,
}

impl HtmlComponent for InteractiveLesson6Yo {
    fn render_html(&self) -> String {
        let is_first = self.step_number == 1;
        let card_class = if is_first {
            "lesson-6yo-story-card active"
        } else {
            "lesson-6yo-story-card"
        };
        let card_id = format!("lesson-6yo-card-{}", self.step_number);
        let step_str = self.step_number.to_string();

        let badge = rsx_elem!(
            "div",
            [("class", "lesson-6yo-badge")],
            text: format!("{} PASUL {} DIN 6 — {}", self.badge_emoji, self.step_number, self.tab_label)
        );

        let heading = rsx_elem!(
            "h3",
            [("class", "lesson-6yo-card-title")],
            text: self.title
        );

        let story_box = rsx_elem!(
            "div",
            [("class", "lesson-6yo-story-box")],
            raw: self.kid_story_ro
        );

        let rule_box = rsx_elem!(
            "div",
            [("class", "lesson-6yo-rule-box")],
            raw: self.visual_rule_ro
        );

        let secret_box = rsx_elem!(
            "div",
            [("class", "lesson-6yo-secret-box")],
            raw: self.science_secret_ro
        );

        let mut buttons_row = DomElement::new("div")
            .attr("class", "lesson-6yo-actions-row")
            .attr("role", "group")
            .attr("aria-label", format!("Butoane interactive Lecția {}", self.step_number));

        for (idx, btn) in self.action_buttons.iter().enumerate() {
            let mut btn_html = btn.render_html();
            if idx == 0 {
                btn_html = btn_html.replacen("kid-action-btn ", "kid-action-btn active-mode ", 1);
            }
            buttons_row = buttons_row.raw_html(btn_html);
        }

        let card = rsx_elem!(
            "article",
            [
                ("id", card_id),
                ("class", card_class),
                ("data-step", step_str),
                ("data-default-mode", self.default_mode)
            ],
            [badge, heading, story_box, rule_box, secret_box, buttons_row]
        );

        card.render_html()
    }
}

/// Returns the complete, ordered curriculum of 6 interactive lessons for a 6-year-old / beginner.
#[must_use]
pub fn get_all_6yo_lessons() -> Vec<InteractiveLesson6Yo> {
    vec![
        // LECȚIA 1
        InteractiveLesson6Yo {
            step_number: 1,
            badge_emoji: "🎈",
            tab_label: "Lecția 1 🎈",
            title: "🎈 Lecția 1: „Problema: Ceața și Hackerul Rău 👾”",
            kid_story_ro: concat!(
                "<strong>Povestea pe înțelesul unui copil de 6 ani:</strong> ",
                "Avionul nostru vrea să aterizeze în siguranță pe pistă, ca o pasăre care se întoarce la cuib. ",
                "Dar apar <strong>două pericole mari</strong>: ",
                "<br/>1️⃣ Un <strong>nor gros de ceață albă 🌫️</strong> îi acoperă parbrizul, așa că pilotul nu mai vede luminile pistei! ",
                "<br/>2️⃣ Un <strong>Hacker Rău 👾</strong> de pe pământ trimite cu o antenă-magnet un semnal GPS mincinos ca să păcălească avionul ",
                "că pista e mai la dreapta (în pădure!). ",
                "<br/>Din fericire, când apeși pe <strong>🛡️ Pornește AERO-SHIELD</strong>, creierul nostru inteligent scoate ",
                "<strong>Foarfeca Aurie ✂️</strong>, taie raza roșie a hackerului și aprinde raza verde spre pista adevărată!"
            ),
            visual_rule_ro: concat!(
                "👀 <strong>Uită-te pe desenul animat:</strong> ",
                "🔴 <em>Linia Roșie Punctată</em> = Minciuna Hackerului GPS | ",
                "🟢 <em>Raza Verde Strălucitoare</em> = Drumul Adevărat găsit de AERO-SHIELD!"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Secretul Inginerilor (AERO-SHIELD V2.0):</strong> ",
                "În aviația reală, atacul se numește <em>GNSS Spoofing</em>, iar ceața groasă se numește <em>Condiții Cat IIIc (Vizibilitate Zero)</em>. ",
                "AERO-SHIELD combină senzorii inerțiali (IMU), camera infraroșie SWIR și filtrul matematic ESKF + Chi-Pătrat pentru a tăia instantaneu GPS-ul fals!"
            ),
            default_mode: "l1_fog",
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
                    label: "🛡️ Pornește AERO-SHIELD",
                    subtitle: "Foarfeca aurie taie minciuna și salvează avionul!",
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
            title: "🔦 Lecția 2: „Ochiul Magic care Vede prin Ceață și Fum 👀”",
            kid_story_ro: concat!(
                "<strong>Cum putem vedea prin ceață când ochii noștri văd doar un perete alb?</strong> ",
                "Lumina obișnuită (albastră/verde, pe care o văd ochii noștri) are <strong>valuri mici și dese</strong> — ",
                "când întâlnește picăturile minuscule de apă din ceață sau particulele de fum, se lovește de ele ca o minge de ping-pong și sare înapoi! ",
                "<br/>Dar <strong>Ochiul Magic Infraroșu (SWIR + LWIR) 🔦</strong> folosește o lumină invizibilă cu <strong>pași uriași (unde lungi)</strong>, ",
                "care pășește direct peste picăturile de ceață și peste fumul negru, văzând becurile calde ale pistei ca prin sticlă curată!"
            ),
            visual_rule_ro: concat!(
                "🖱️ <strong>Joacă-te cu mouse-ul sau degetul pe desen!</strong> ",
                "Mișcă <strong>Lupa Magică Infraroșie 🔍</strong> peste norul de ceață și vezi cum în interiorul cercului apar becurile verzi ale pistei!"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Secretul Fizicii (Legea Mie & Beer-Lambert):</strong> ",
                "Camera normală lucrează la λ = 0.55 μm (unde picăturile de ceață împrăștie lumina puternic). ",
                "Camerele noastre SWIR (1.55 μm) și Termică LWIR (10 μm) au lungimi de undă mult mai mari decât particulele de fum și ceață fină, ",
                "oferind o transmisie optică de până la 5x–10x mai bună!"
            ),
            default_mode: "l2_swir",
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
                    label: "🔦 Aprinde Ochiul Infraroșu (SWIR)",
                    subtitle: "Activează Lupa Magică (mișcă mouse-ul pe ecran!)",
                    color_theme: "green",
                    action_code: "l2_swir",
                },
                LessonActionButton {
                    id: "btn-l2-waves",
                    emoji: "💧",
                    label: "💧 Arată Razele vs. Picăturile",
                    subtitle: "Vezi cum unda mică se lovește, iar unda mare trece!",
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
            title: "🚂 Lecția 3: „Cum aflăm viteza fără GPS? (Numărăm becurile de pe jos!) 💡”",
            kid_story_ro: concat!(
                "<strong>Imaginează-ți că ești într-un tren și te uiți pe geam la stâlpii de lumină de lângă șină!</strong> ",
                "Chiar dacă nu ai GPS, știi imediat cât de repede mergi: ",
                "dacă stâlpii trec încet pe lângă geam (<em>țac... țac... țac...</em>), trenul merge încet ca o țestoasă 🐢. ",
                "Dacă stâlpii zboară pe lângă geam (<em>țac-țac-țac!</em>), trenul fuge ca o rachetă 🚀! ",
                "<br/>Exact așa face și <strong>Ochiul Magic al Avionului</strong>: face două poze foarte repede și măsoară cu o ",
                "<strong>Riglă Luminoasă 📏</strong> câți centimetri s-au mutat becurile de pe pământ între Poza 1 și Poza 2!"
            ),
            visual_rule_ro: concat!(
                "📏 <strong>Regula de Aur a Vitezei Vizuale:</strong> ",
                "Săgeți Galbene Scurte = <strong>Viteză Mică (45 m/s)</strong> | ",
                "Săgeți Galbene Lungi = <strong>Viteză Mare (120 m/s)</strong>!"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Secretul Matematic (Flux Optic Lucas-Kanade):</strong> ",
                "Fiecare bec sau colț de clădire este un <em>punct de reper (feature)</em>. ",
                "Măsurând deplasarea în pixeli Δu între două cadre consecutive (la 60 Hz) și cunoscând înălțimea h de la radio-altimetru, ",
                "calculăm viteza reală a avionului: V = (Δu / f) · (h / Δt), complet independent de sateliții GPS!"
            ),
            default_mode: "l3_slow",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l3-slow",
                    emoji: "🐢",
                    label: "🐢 Zboară Încet",
                    subtitle: "Becurile se mișcă puțin între două poze (45 m/s)",
                    color_theme: "blue",
                    action_code: "l3_slow",
                },
                LessonActionButton {
                    id: "btn-l3-fast",
                    emoji: "🚀",
                    label: "🚀 Zboară Repede",
                    subtitle: "Becurile fac un salt mare între două poze (120 m/s)",
                    color_theme: "amber",
                    action_code: "l3_fast",
                },
                LessonActionButton {
                    id: "btn-l3-ruler",
                    emoji: "📏",
                    label: "📏 Arată Rigla dintre Poze",
                    subtitle: "Pune Rigla Galbenă între Poza 1 (umbră) și Poza 2 (acum)!",
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
            title: "☁️ Lecția 4: „Capcana Norului Împins de Vânt și a Înclinării Avionului! 💨”",
            kid_story_ro: concat!(
                "<strong>Atenție! Un calculator simplu poate fi păcălit de două iluzii optice:</strong> ",
                "<br/>1️⃣ <strong>Capcana Norului Plutitor 💨:</strong> Dacă sub avion plutește un nor împins de vânt, ",
                "un aparat prost crede că <em>pământul fuge</em>, când de fapt doar norul e suflat de vânt! ",
                "<br/>2️⃣ <strong>Capcana Înclinării Avionului 🔄:</strong> Când o pală de vânt clatină aripile avionului (exact cum dai tu din cap stânga-dreapta), ",
                "toată imaginea din cameră fuge într-o parte, deși avionul nu a sărit cu 50 de metri mai încolo! ",
                "<br/><strong>Cum rezolvă AERO-SHIELD? 🧠</strong> Separă imaginea în <strong>2 Straturi</strong>: ",
                "taie cu un X roșu <strong>Norul Mov ❌</strong> (care se mișcă haotic) și păstrează doar <strong>Pământul Verde ✅</strong> (care stă nemișcat), ",
                "iar giroscopul din burta avionului scade imediat clătinatul aripilor!"
            ),
            visual_rule_ro: concat!(
                "📢 <strong>Megafonul din Cockpit:</strong> ",
                "Când vântul clatină brusc avionul timp de 1 secundă, AERO-SHIELD strigă în difuzor: ",
                "<em>„Stai liniștit, doar s-a înclinat avionul, nu ne-am mutat de pe pistă!”</em>"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Secretul Inginerilor (RANSAC Homography + Derotare IMU ω × r):</strong> ",
                "Folosim descompunerea în 2 straturi (<em>Two-Layer Cloud-Ground Separation</em>) pentru a respinge vectorii de flux optic de pe nori ",
                "și scădem viteza unghiulară măsurată de giroscop (ω_x, ω_y, ω_z) înainte de a calcula viteza la sol!"
            ),
            default_mode: "l4_separate",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l4-wind",
                    emoji: "💨",
                    label: "💨 Suflă Vântul în Nor",
                    subtitle: "Vântul împinge norul mov cu săgeți strâmbe!",
                    color_theme: "amber",
                    action_code: "l4_wind",
                },
                LessonActionButton {
                    id: "btn-l4-tilt",
                    emoji: "🔄",
                    label: "🔄 Înclină Avionul",
                    subtitle: "Rafală de 1 secundă: clatină aripile și ascultă explicația!",
                    color_theme: "blue",
                    action_code: "l4_tilt",
                },
                LessonActionButton {
                    id: "btn-l4-separate",
                    emoji: "🧠",
                    label: "🧠 Separă Norul Mov de Pământul Verde",
                    subtitle: "Ignoră norul mincinos (❌) și citește doar solul fix (✅)!",
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
            title: "⚖️ Lecția 5: „Cântarul Adevărului: Cum prindem orice minciună? ⚖️”",
            kid_story_ro: concat!(
                "<strong>Cum își dă seama avionul în mai puțin de o clipire că GPS-ul minte?</strong> ",
                "În creierul avionului avem două instrumente magice de detectiv: ",
                "<br/>⚖️ <strong>1. Cântarul Adevărului (Pentru Minciuni Mari și Bruște ⚡):</strong> ",
                "Pe talerul din stânga punem ce spun <em>Ochiul Infraroșu + Simțul Echilibrului</em>. ",
                "Pe talerul din dreapta punem ce spune <em>GPS-ul</em>. Dacă hackerul spune brusc că avionul a sărit 500 de metri în pădure, ",
                "talerul din dreapta cade cu zgomot și se aprinde <strong>Alarma Roșie 🚨</strong>!",
                "<br/>🥛 <strong>2. Paharul de Minciuni (Pentru Minciuni Mici și Viclene 🐍):</strong> ",
                "Dar dacă hackerul e șiret și împinge avionul doar câte puțin, <em>câte o picătură pe secundă</em>, ca să nu încline cântarul? ",
                "Fiecare mică nepotrivire cade ca o <strong>picătură roșie 💧</strong> în <em>Paharul de Minciuni</em>! ",
                "În 3 secunde paharul se umple până la linia roșie și declanșează alarma!"
            ),
            visual_rule_ro: concat!(
                "🕵️ <strong>Nicio minciună nu scapă!</strong> ",
                "Minciuna Mare răstoarnă <strong>Cântarul ⚖️</strong> pe loc, iar Minciuna Lentă umple <strong>Paharul 🥛</strong> picătură cu picătură în 3 secunde!"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Secretul Matematic (Testul Chi-Pătrat χ² + Detectorul CUSUM):</strong> ",
                "Cântarul Adevărului este <em>Poarta de Inovație Mahalanobis χ²</em> (prag 7.815 pentru 3 grade de libertate), ",
                "iar Paharul de Minciuni este <em>Suma Cumulativă Page-Hinkley (CUSUM)</em> care integrează abaterile subtile (Ramp Spoofing) în timp!"
            ),
            default_mode: "l5_honest",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l5-honest",
                    emoji: "✅",
                    label: "✅ Zbor Cinstit (Echilibru)",
                    subtitle: "Cântarul stă drept și Paharul de Minciuni este gol",
                    color_theme: "green",
                    action_code: "l5_honest",
                },
                LessonActionButton {
                    id: "btn-l5-big-lie",
                    emoji: "⚡",
                    label: "⚡ Minciună Mare (Salt 500m)",
                    subtitle: "Talerul GPS cade brusc și sună alarma Chi-Pătrat!",
                    color_theme: "red",
                    action_code: "l5_big_lie",
                },
                LessonActionButton {
                    id: "btn-l5-slow-lie",
                    emoji: "🐍",
                    label: "🐍 Trimite Minciună Lentă (Paharul de Minciuni)",
                    subtitle: "Picăturile roșii umplu paharul în 3 secunde -> ALARMĂ!",
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
            title: "🏆 Lecția 6: „Acum ești Comandant! Testează pe Simulatorul Mare și pe Filmarea Reală!”",
            kid_story_ro: concat!(
                "<strong>Felicitări, Comandante! 👩‍✈️👨‍✈️ Ai învățat toate cele 5 secrete ale scutului AERO-SHIELD!</strong> ",
                "Acum ești gata să privești <strong>Ecranul Mare de Comandă al Avionului</strong> de mai jos, unde totul se întâmplă în timp real (de 60 de ori pe secundă!): ",
                "<br/>🟢 <strong>1. Parbrizul Magic (Stânga):</strong> Vezi pista prin ceață și razele verzi de viteză optică. ",
                "<br/>🏁 <strong>2. Cursa celor 4 Avioane (Mijloc):</strong> Vezi cum avionul fără scut (Roșu 🔴) este păcălit de hacker și iese în decor, ",
                "în timp ce avionul nostru <strong>AERO-SHIELD (Verde 🟢)</strong> rămâne lipit de linia albă a adevărului! ",
                "<br/>🎮 <strong>3. Telecomanda de Experimente & Filmarea Reală (Dreapta/Sus):</strong> Poți porni oricând un atac de hacker sau ",
                "poți vedea <strong>Filmarea Reală din Cockpit 🎬</strong>!"
            ),
            visual_rule_ro: concat!(
                "✨ <strong>Apasă primul buton de mai jos</strong> ca să aprindem <strong>Săgeți Luminoase Galbene</strong> ",
                "direct pe Ecranul Mare al Simulatorului și să îți arătăm unde este fiecare panou!"
            ),
            science_secret_ro: concat!(
                "🔬 <strong>Arhitectura Completă (Rust 100% + Fizică 6-DOF + 4 Estimatoare în Paralel):</strong> ",
                "Pe ecranul principal rulează simultan 4 algoritmi: (1) Pure INS, (2) Unfiltered GPS+INS, (3) Naive Optical Flow și ",
                "(4) AERO-SHIELD V2.0 (ESKF 15-stări + SWIR/LWIR + Homografie 2-Straturi + Chi-Pătrat/CUSUM + Rețea Neuronală PINN)."
            ),
            default_mode: "l6_commander",
            action_buttons: vec![
                LessonActionButton {
                    id: "btn-l6-spotlight",
                    emoji: "✨",
                    label: "✨ Arată-mi cu Săgeți Luminoase pe Ecranul Mare",
                    subtitle: "Aprinde ghidul luminos peste Parbriz, Cursa celor 4 Avioane și Telecomandă!",
                    color_theme: "amber",
                    action_code: "l6_spotlight",
                },
                LessonActionButton {
                    id: "btn-l6-real-video",
                    emoji: "🎬",
                    label: "🎬 Pornește Filmarea Reală din Cockpit",
                    subtitle: "Comută în modul Demonstrație pe Filmare Reală & Telemetrie",
                    color_theme: "blue",
                    action_code: "l6_real_video",
                },
                LessonActionButton {
                    id: "btn-l6-diploma",
                    emoji: "🏅",
                    label: "🏅 Diplomă de Comandant",
                    subtitle: "Afișează Diploma Oficială de Tânăr Inginer AERO-SHIELD!",
                    color_theme: "green",
                    action_code: "l6_diploma",
                },
            ],
        },
    ]
}

/// Complete deck component containing all 6 interactive lessons, navigation controls, and the 60 FPS Canvas 2D laboratory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lessons6YoDeck {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub lessons: Vec<InteractiveLesson6Yo>,
}

impl Default for Lessons6YoDeck {
    fn default() -> Self {
        Self {
            title: "🎈 ACADEMIA DE ZBOR AERO-SHIELD (EXPLICAT PENTRU UN COPIL DE 6 ANI ȘI PENTRU ORICE ÎNCEPĂTOR)",
            subtitle: "Parcurge cei 6 pași interactivi în 60 de secunde! Apasă pe butoanele mari și colorate și joacă-te cu mouse-ul pe desenul animat!",
            lessons: get_all_6yo_lessons(),
        }
    }
}

impl HtmlComponent for Lessons6YoDeck {
    fn render_html(&self) -> String {
        let mut root = DomElement::new("section")
            .attr("id", "aero-lessons-6yo-root")
            .attr("class", "lessons-6yo-container")
            .attr("aria-label", "Academia Interactivă de Zbor pentru Copii de 6 Ani și Începători");

        // Inject Scoped CSS for the 6yo Academy Component
        let style_el = DomElement::new("style").raw_html(LESSONS_6YO_SCOPED_CSS);
        root = root.child(style_el);

        // Top Banner Header with Collapse/Expand & Quick Jump
        let header_title = rsx_elem!(
            "div",
            [("class", "lessons-6yo-header-title")],
            text: self.title
        );
        let header_sub = rsx_elem!(
            "div",
            [("class", "lessons-6yo-header-sub")],
            text: self.subtitle
        );
        let header_left = rsx_elem!(
            "div",
            [("class", "lessons-6yo-header-left")],
            [header_title, header_sub]
        );

        let toggle_btn = rsx_elem!(
            "button",
            [
                ("type", "button"),
                ("id", "btn-toggle-6yo-body"),
                ("class", "lessons-6yo-collapse-btn"),
                ("onclick", "window.toggleLessons6YoPanel()")
            ],
            text: "🔽 Ascunde / Arată Academia Copiilor"
        );

        let header_bar = rsx_elem!(
            "header",
            [("class", "lessons-6yo-header-bar")],
            [header_left, toggle_btn]
        );
        root = root.child(header_bar);

        // Stepper Navigation Bar (Lecția 1 🎈 ... Lecția 6 🏆 + Prev/Next)
        let mut stepper_tabs = DomElement::new("div")
            .attr("class", "lessons-6yo-stepper-tabs")
            .attr("role", "tablist");

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

        let nav_arrows = rsx_elem!(
            "div",
            [("class", "lessons-6yo-nav-arrows")],
            [prev_btn, next_btn]
        );

        let nav_bar = rsx_elem!(
            "nav",
            [("class", "lessons-6yo-nav-bar"), ("aria-label", "Pașii Academiei")],
            [stepper_tabs, nav_arrows]
        );

        // Main Split Grid: Left = SSR Story Cards + Big Kid Buttons; Right = 600x260 Interactive Canvas Laboratory
        let mut cards_column = DomElement::new("div").attr("class", "lessons-6yo-cards-col");
        for lesson in &self.lessons {
            cards_column = cards_column.raw_html(lesson.render_html());
        }

        // Right Column: Interactive Canvas 600x260 + Status Pill + Megaphone Banner
        let canvas_badge = rsx_elem!(
            "div",
            [("id", "lessonCanvasStatusBadge"), ("class", "lesson-canvas-status-badge")],
            text: "🎨 LABORATORUL ANIMAT (60 FPS) — Apasă pe butoane sau mișcă mouse-ul pe desen!"
        );

        let canvas_el = rsx_elem!(
            "canvas",
            [
                ("id", "lessonInteractiveCanvas"),
                ("width", "600"),
                ("height", "260"),
                ("class", "lesson-interactive-canvas"),
                ("aria-label", "Desen interactiv pentru explicarea navigatiei AERO-SHIELD")
            ],
            text: "Browserul tău nu suportă HTML5 Canvas."
        );

        let megaphone_banner = rsx_elem!(
            "div",
            [("id", "lessonMegaphoneBanner"), ("class", "lesson-megaphone-banner")],
            text: "📢 Sfatul Comandantului: Apasă pe cele 3 butoane colorate din stânga ca să vezi ce se întâmplă cu avionul!"
        );

        let canvas_col = rsx_elem!(
            "div",
            [("class", "lessons-6yo-canvas-col")],
            [canvas_badge, canvas_el, megaphone_banner]
        );

        let split_grid = rsx_elem!(
            "div",
            [("class", "lessons-6yo-split-grid")],
            [cards_column, canvas_col]
        );

        let body_wrap = rsx_elem!(
            "div",
            [("id", "lessons-6yo-collapsible-body"), ("class", "lessons-6yo-body-wrap")],
            [nav_bar, split_grid]
        );

        root = root.child(body_wrap);

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
    doc.push_str("  <title>AERO-SHIELD V2.0 — Academia de Zbor pentru Copii de 6 Ani și Începători</title>\n");
    doc.push_str("  <style>\n");
    doc.push_str("    body { margin: 0; padding: 18px; background: #060b16; color: #f8fafc; font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; }\n");
    doc.push_str("    .standalone-top-nav { max-width: 1360px; margin: 0 auto 14px auto; display: flex; justify-content: space-between; align-items: center; }\n");
    doc.push_str("    .standalone-back-link { color: #38bdf8; text-decoration: none; font-weight: 700; font-size: 14px; padding: 8px 14px; border: 1px solid rgba(56,189,248,0.4); border-radius: 8px; background: rgba(15,23,42,0.8); }\n");
    doc.push_str("    .standalone-back-link:hover { background: rgba(56,189,248,0.18); }\n");
    doc.push_str("  </style>\n");
    doc.push_str("</head>\n<body>\n");
    doc.push_str("  <div class=\"standalone-top-nav\">\n");
    doc.push_str("    <a class=\"standalone-back-link\" href=\"/\">⬅️ Înapoi la Simulatorul Principal AERO-SHIELD Cockpit</a>\n");
    doc.push_str("    <span style=\"font-size:13px;color:#94a3b8;\">SSR 100% Rust (`ui_lessons_6yo.rs`) • DO-178C Zero-Unwrap</span>\n");
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

/// Spotlight overlay HTML for Lesson 6 ("✨ Arată-mi cu Săgeți Luminoase pe Ecranul Mare").
const SPOTLIGHT_OVERLAY_INNER_HTML: &str = r#"
<div class="spotlight-backdrop" onclick="window.closeSpotlightGuide6Yo()">
  <div class="spotlight-header-card" onclick="event.stopPropagation()">
    <div style="font-size:20px;font-weight:900;color:#fde047;">
      ✨ GHIDUL COMANDANTULUI PE ECRANUL MARE (3 ZONE PRINCIPALE)
    </div>
    <p style="margin:6px 0 10px 0;font-size:14px;color:#e2e8f0;">
      Iată unde trebuie să te uiți pe simulatorul mare de dedesubt ca să vezi cum AERO-SHIELD salvează avionul:
    </p>
    <div class="spotlight-pointer-grid">
      <div class="spotlight-pointer-card pointer-left">
        <div class="spotlight-arrow-bounce">↙️ 1. PARBRIZUL MAGIC (STÂNGA JOS)</div>
        <div class="spotlight-card-title">🔦 Ochiul Infraroșu & Săgețile de Viteză</div>
        <div class="spotlight-card-desc">
          Aici vezi pista de aterizare chiar și prin ceață groasă! Punctele și săgețile verzi sunt becurile de pe pământ numărate de Ochiul Magic (Lecțiile 2, 3 și 4).
        </div>
      </div>
      <div class="spotlight-pointer-card pointer-center">
        <div class="spotlight-arrow-bounce">⬇️ 2. CURSA CELOR 4 AVIOANE (MIJLOC JOS)</div>
        <div class="spotlight-card-title">🏁 Cine rămâne pe Pista Adevărată?</div>
        <div class="spotlight-card-desc">
          Urmărește <strong>Linia Verde (AERO-SHIELD)</strong> față de <strong>Linia Roșie (Avionul fără scut, păcălit de Hacker)</strong>! Când pornești atacul, linia roșie fuge în pădure, dar linia verde aterizează la fix!
        </div>
      </div>
      <div class="spotlight-pointer-card pointer-right">
        <div class="spotlight-arrow-bounce">↘️ 3. TELECOMANDA & CÂNTARUL (DREAPTA JOS)</div>
        <div class="spotlight-card-title">⚖️ Butoanele de Atac & Alarma de Minciuni</div>
        <div class="spotlight-card-desc">
          Aici ai butoanele cu care poți băga ceață, vânt sau Hacker GPS și tot aici vezi <strong>Cântarul Chi-Pătrat / Paharul CUSUM</strong> care devine roșu când prinde hackerul (Lecția 5)!
        </div>
      </div>
    </div>
    <button type="button" class="spotlight-close-btn" onclick="window.closeSpotlightGuide6Yo()">
      👍 Am înțeles! Închide Săgețile Luminoase și lasă-mă să pilotez!
    </button>
  </div>
</div>
"#;

/// Scoped CSS for the 6-Year-Old Academy SSR component.
const LESSONS_6YO_SCOPED_CSS: &str = r#"
.lessons-6yo-container {
  max-width: 1380px;
  margin: 10px auto 14px auto;
  background: linear-gradient(145deg, rgba(15, 23, 42, 0.96), rgba(9, 14, 28, 0.98));
  border: 2px solid rgba(56, 189, 248, 0.45);
  border-radius: 16px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.55), 0 0 24px rgba(56, 189, 248, 0.14);
  padding: 14px 18px;
  color: #f8fafc;
  font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
}
.lessons-6yo-header-bar {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.22);
}
.lessons-6yo-header-title {
  font-size: 17px;
  font-weight: 900;
  letter-spacing: 0.3px;
  color: #fde047;
  text-shadow: 0 0 12px rgba(253, 224, 71, 0.25);
}
.lessons-6yo-header-sub {
  font-size: 13px;
  color: #cbd5e1;
  margin-top: 3px;
}
.lessons-6yo-collapse-btn {
  background: rgba(30, 41, 59, 0.9);
  color: #38bdf8;
  border: 1px solid rgba(56, 189, 248, 0.45);
  border-radius: 10px;
  padding: 7px 13px;
  font-size: 12.5px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.18s ease;
}
.lessons-6yo-collapse-btn:hover {
  background: rgba(56, 189, 248, 0.18);
  transform: translateY(-1px);
}
.lessons-6yo-body-wrap {
  margin-top: 12px;
}
.lessons-6yo-body-wrap.collapsed {
  display: none;
}
.lessons-6yo-nav-bar {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}
.lessons-6yo-stepper-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.lesson-6yo-step-btn {
  background: rgba(30, 41, 59, 0.85);
  color: #cbd5e1;
  border: 1.5px solid rgba(148, 163, 184, 0.3);
  border-radius: 999px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 800;
  cursor: pointer;
  transition: all 0.18s ease;
}
.lesson-6yo-step-btn:hover {
  border-color: #38bdf8;
  color: #ffffff;
}
.lesson-6yo-step-btn.active {
  background: linear-gradient(135deg, #0284c7, #2563eb);
  color: #ffffff;
  border-color: #7dd3fc;
  box-shadow: 0 0 14px rgba(56, 189, 248, 0.45);
  transform: scale(1.03);
}
.lessons-6yo-nav-arrows {
  display: flex;
  gap: 8px;
}
.lesson-6yo-nav-arrow {
  background: rgba(30, 41, 59, 0.9);
  color: #e2e8f0;
  border: 1px solid rgba(148, 163, 184, 0.35);
  border-radius: 10px;
  padding: 7px 13px;
  font-size: 12.5px;
  font-weight: 700;
  cursor: pointer;
}
.lesson-6yo-nav-arrow.primary-next {
  background: linear-gradient(135deg, #16a34a, #059669);
  border-color: #4ade80;
  color: #ffffff;
}
.lessons-6yo-split-grid {
  display: grid;
  grid-template-columns: 1.15fr 1fr;
  gap: 16px;
  align-items: stretch;
}
@media (max-width: 1080px) {
  .lessons-6yo-split-grid {
    grid-template-columns: 1fr;
  }
}
.lesson-6yo-story-card {
  display: none;
  background: rgba(15, 23, 42, 0.75);
  border: 1px solid rgba(148, 163, 184, 0.25);
  border-radius: 14px;
  padding: 14px 16px;
}
.lesson-6yo-story-card.active {
  display: block;
}
.lesson-6yo-badge {
  display: inline-block;
  background: rgba(56, 189, 248, 0.16);
  color: #7dd3fc;
  border: 1px solid rgba(56, 189, 248, 0.35);
  border-radius: 999px;
  padding: 3px 11px;
  font-size: 11.5px;
  font-weight: 800;
  margin-bottom: 6px;
}
.lesson-6yo-card-title {
  margin: 4px 0 8px 0;
  font-size: 18px;
  font-weight: 900;
  color: #ffffff;
}
.lesson-6yo-story-box {
  font-size: 13.8px;
  line-height: 1.52;
  color: #e2e8f0;
  background: rgba(2, 6, 23, 0.55);
  border-left: 4px solid #38bdf8;
  padding: 9px 12px;
  border-radius: 8px;
  margin-bottom: 8px;
}
.lesson-6yo-rule-box {
  font-size: 13px;
  line-height: 1.45;
  color: #fef08a;
  background: rgba(234, 179, 8, 0.12);
  border: 1px solid rgba(250, 204, 21, 0.35);
  padding: 7px 11px;
  border-radius: 8px;
  margin-bottom: 8px;
}
.lesson-6yo-secret-box {
  font-size: 12px;
  line-height: 1.42;
  color: #94a3b8;
  background: rgba(15, 23, 42, 0.65);
  border: 1px dashed rgba(148, 163, 184, 0.3);
  padding: 6px 10px;
  border-radius: 8px;
  margin-bottom: 10px;
}
.lesson-6yo-actions-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
@media (max-width: 700px) {
  .lesson-6yo-actions-row {
    grid-template-columns: 1fr;
  }
}
.kid-action-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  text-align: left;
  padding: 9px 11px;
  border-radius: 12px;
  border: 2px solid rgba(148, 163, 184, 0.35);
  background: rgba(30, 41, 59, 0.85);
  color: #f8fafc;
  cursor: pointer;
  transition: all 0.16s ease;
}
.kid-action-btn:hover {
  transform: translateY(-2px);
  filter: brightness(1.12);
}
.kid-action-btn.active-mode {
  box-shadow: 0 0 0 2px #fde047, 0 6px 16px rgba(0, 0, 0, 0.45);
  transform: translateY(-1px);
}
.kid-action-btn.theme-blue {
  background: linear-gradient(135deg, rgba(3, 105, 161, 0.55), rgba(30, 58, 138, 0.65));
  border-color: #38bdf8;
}
.kid-action-btn.theme-red {
  background: linear-gradient(135deg, rgba(185, 28, 28, 0.55), rgba(127, 29, 29, 0.65));
  border-color: #f87171;
}
.kid-action-btn.theme-green {
  background: linear-gradient(135deg, rgba(21, 128, 61, 0.55), rgba(6, 95, 70, 0.65));
  border-color: #4ade80;
}
.kid-action-btn.theme-amber {
  background: linear-gradient(135deg, rgba(180, 83, 9, 0.55), rgba(120, 53, 15, 0.65));
  border-color: #fbbf24;
}
.kid-action-btn.theme-purple {
  background: linear-gradient(135deg, rgba(109, 40, 217, 0.55), rgba(76, 29, 149, 0.65));
  border-color: #c084fc;
}
.kid-btn-emoji {
  font-size: 22px;
  line-height: 1;
  flex-shrink: 0;
}
.kid-btn-text-wrap {
  display: flex;
  flex-direction: column;
}
.kid-btn-title {
  font-size: 12.8px;
  font-weight: 800;
  color: #ffffff;
}
.kid-btn-sub {
  font-size: 11px;
  color: #cbd5e1;
  margin-top: 2px;
}
.lessons-6yo-canvas-col {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: rgba(2, 6, 23, 0.85);
  border: 1.5px solid rgba(56, 189, 248, 0.35);
  border-radius: 14px;
  padding: 10px 12px;
}
.lesson-canvas-status-badge {
  font-size: 12px;
  font-weight: 800;
  color: #38bdf8;
  margin-bottom: 6px;
  text-align: center;
}
.lesson-interactive-canvas {
  width: 100%;
  max-width: 600px;
  height: 260px;
  margin: 0 auto;
  display: block;
  border-radius: 10px;
  background: #040814;
  border: 1px solid rgba(148, 163, 184, 0.3);
  cursor: crosshair;
}
.lesson-megaphone-banner {
  margin-top: 8px;
  padding: 7px 10px;
  border-radius: 8px;
  background: rgba(15, 23, 42, 0.9);
  border: 1px solid rgba(250, 204, 21, 0.4);
  color: #fde047;
  font-size: 12.5px;
  font-weight: 700;
  text-align: center;
}
.aero-6yo-spotlight-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(2, 6, 23, 0.82);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}
.aero-6yo-spotlight-overlay.hidden {
  display: none;
}
.spotlight-header-card {
  max-width: 980px;
  width: 100%;
  background: linear-gradient(145deg, #0f172a, #020617);
  border: 2px solid #fde047;
  border-radius: 18px;
  padding: 20px 24px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
  text-align: center;
}
.spotlight-pointer-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  margin: 14px 0 18px 0;
  text-align: left;
}
@media (max-width: 850px) {
  .spotlight-pointer-grid {
    grid-template-columns: 1fr;
  }
}
.spotlight-pointer-card {
  background: rgba(30, 41, 59, 0.85);
  border: 1.5px solid rgba(56, 189, 248, 0.45);
  border-radius: 12px;
  padding: 12px 14px;
}
.spotlight-arrow-bounce {
  font-size: 13px;
  font-weight: 900;
  color: #fde047;
  margin-bottom: 6px;
}
.spotlight-card-title {
  font-size: 14px;
  font-weight: 800;
  color: #ffffff;
  margin-bottom: 4px;
}
.spotlight-card-desc {
  font-size: 12.5px;
  color: #cbd5e1;
  line-height: 1.45;
}
.spotlight-close-btn {
  background: linear-gradient(135deg, #eab308, #ca8a04);
  color: #020617;
  font-weight: 900;
  font-size: 14px;
  padding: 10px 22px;
  border-radius: 999px;
  border: none;
  cursor: pointer;
}
"#;

/// Interactive 60 FPS HTML5 Canvas 2D Engine for all 6 lessons.
const LESSONS_6YO_INTERACTIVE_JS: &str = r#"
(function() {
  if (window.__aeroLessons6YoInitialized) return;
  window.__aeroLessons6YoInitialized = true;

  var state = {
    step: 1,
    mode: 'l1_fog',
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

    var cards = document.querySelectorAll('.lesson-6yo-story-card');
    for (var j = 0; j < cards.length; j++) {
      var cStep = parseInt(cards[j].getAttribute('data-step') || '0', 10);
      if (cStep === s) {
        cards[j].classList.add('active');
        var defMode = cards[j].getAttribute('data-default-mode') || ('l' + s + '_default');
        var firstBtn = cards[j].querySelector('.kid-action-btn');
        window.setLessonMode(defMode, firstBtn);
      } else {
        cards[j].classList.remove('active');
      }
    }
  };

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
      setMegaphone('🌫️ <strong>Norul de Ceață</strong> acoperă pista! Ochiul obișnuit nu mai vede unde să aterizeze.');
    } else if (actionCode === 'l1_hacker') {
      setMegaphone('👾 <strong>ALERTĂ HACKER!</strong> Antena-magnet trage avionul pe linia roșie spre pădure!');
    } else if (actionCode === 'l1_shield') {
      setMegaphone('🛡️ <strong>AERO-SHIELD ACTIV!</strong> Foarfeca Aurie ✂️ taie minciuna hackerului și aprinde raza verde spre pistă!');
    } else if (actionCode === 'l2_fog_smoke') {
      state.darkSmoke = !state.darkSmoke;
      setMegaphone(state.darkSmoke
        ? '🖤 <strong>Fum Negru Dens:</strong> Lumina vizibilă este blocată complet, dar Ochiul Termic/SWIR trece prin el!'
        : '🌫️ <strong>Ceață Albă Groasă:</strong> Mișcă mouse-ul pe desen ca să folosești Lupa Magică Infraroșie!');
    } else if (actionCode === 'l2_swir') {
      setMegaphone('🔦 <strong>Lupa Magică SWIR este aprinsă!</strong> Mișcă mouse-ul peste norul de pe desen ca să vezi becurile verzi!');
    } else if (actionCode === 'l2_waves') {
      setMegaphone('💧 <strong>Fizica pe înțelesul copiilor:</strong> Unda albastră mică se lovește de picături; unda infraroșie mare pășește peste ele!');
    } else if (actionCode === 'l3_slow') {
      setMegaphone('🐢 <strong>Zbor Încet (45 m/s):</strong> Becurile de pe jos se mută puțin între două poze — săgeți galbene scurte!');
    } else if (actionCode === 'l3_fast') {
      setMegaphone('🚀 <strong>Zbor Rapid (120 m/s):</strong> Becurile fac un salt lung între două poze — săgeți galbene lungi!');
    } else if (actionCode === 'l3_ruler') {
      setMegaphone('📏 <strong>Rigla dintre Poze:</strong> Măsurăm exact câți pixeli s-au mutat becurile între Poza 1 și Poza 2!');
    } else if (actionCode === 'l4_wind') {
      setMegaphone('💨 <strong>Capcana Norului:</strong> Vântul împinge norul mov într-o parte! Dacă ne luăm după nor, greșim drumul!');
    } else if (actionCode === 'l4_tilt') {
      state.tiltBurstUntil = state.tick + 65;
      setMegaphone('📢 <strong>Rafală de 1 secundă!</strong> „Stai liniștit, doar s-a înclinat avionul, nu ne-am mutat de pe pistă!”');
    } else if (actionCode === 'l4_separate') {
      setMegaphone('🧠 <strong>Separare în 2 Straturi:</strong> Tăiem cu ❌ Norul Mov mincinos și citim doar ✅ Pământul Verde nemișcat!');
    } else if (actionCode === 'l5_honest') {
      state.cupLevel = 0.0;
      setMegaphone('✅ <strong>Zbor Cinstit:</strong> Cântarul Adevărului stă în echilibru perfect și Paharul de Minciuni este gol!');
    } else if (actionCode === 'l5_big_lie') {
      state.cupLevel = 1.0;
      setMegaphone('⚡ <strong>MINCIUNĂ MARE PRINSĂ PE LOC!</strong> Saltul de 500m a trântit Cântarul Chi-Pătrat! GPS-ul fals este tăiat!');
    } else if (actionCode === 'l5_slow_lie') {
      state.slowLieStartTick = state.tick;
      state.cupLevel = 0.05;
      setMegaphone('🐍 <strong>Minciună Lentă (1m/s):</strong> Privește cum picăturile roșii 💧 umplu Paharul de Minciuni în 3 secunde!');
    } else if (actionCode === 'l6_spotlight') {
      var ov = document.getElementById('aero-6yo-spotlight-overlay');
      if (ov) ov.classList.remove('hidden');
      setMegaphone('✨ <strong>Săgețile Luminoase sunt active!</strong> Privește explicațiile celor 3 panouri mari ale simulatorului!');
    } else if (actionCode === 'l6_real_video') {
      setMegaphone('🎬 <strong>Mod Filmare Reală & Cockpit:</strong> Am activat demonstrația pe ecranul mare al simulatorului!');
      if (typeof window.startRealFootageDemo === 'function') {
        window.startRealFootageDemo();
      } else {
        var realBtn = document.querySelector('[data-action="real-video"], #btn-real-video, #btnRealVideoDemo');
        if (realBtn && typeof realBtn.click === 'function') realBtn.click();
      }
    } else if (actionCode === 'l6_diploma') {
      setMegaphone('🏅 <strong>FELICITĂRI, COMANDANTE!</strong> Ai absolvit Academia AERO-SHIELD V2.0 cu nota 10 cu felicitări!');
    }
  };

  function setupCanvasEvents() {
    var canvas = document.getElementById('lessonInteractiveCanvas');
    if (!canvas) return;
    function updatePos(clientX, clientY) {
      var rect = canvas.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      state.mouseX = ((clientX - rect.left) / rect.width) * canvas.width;
      state.mouseY = ((clientY - rect.top) / rect.height) * canvas.height;
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

  function drawLesson1(ctx, w, h, t) {
    // Sky & Ground
    ctx.fillStyle = '#071024';
    ctx.fillRect(0, 0, w, h);

    // True Runway on the right
    ctx.fillStyle = '#1e293b';
    ctx.fillRect(450, 45, 120, 75);
    ctx.strokeStyle = '#4ade80';
    ctx.lineWidth = 2;
    ctx.strokeRect(450, 45, 120, 75);
    ctx.fillStyle = '#4ade80';
    ctx.font = 'bold 12px sans-serif';
    ctx.fillText('🛬 PISTA ADEVĂRATĂ', 454, 38);

    // Runway lights
    for (var i = 0; i < 5; i++) {
      ctx.fillStyle = (t % 20 < 10) ? '#4ade80' : '#bbf7d0';
      ctx.beginPath();
      ctx.arc(465 + i * 22, 82, 4, 0, Math.PI * 2);
      ctx.fill();
    }

    // Fake Ghost Runway (bottom right) when Hacker is active or Shield cuts it
    if (state.mode === 'l1_hacker' || state.mode === 'l1_shield') {
      ctx.save();
      ctx.globalAlpha = state.mode === 'l1_shield' ? 0.35 : 0.85;
      ctx.fillStyle = '#3b0712';
      ctx.fillRect(430, 175, 140, 60);
      ctx.strokeStyle = '#f87171';
      ctx.setLineDash([5, 4]);
      ctx.strokeRect(430, 175, 140, 60);
      ctx.setLineDash([]);
      ctx.fillStyle = '#fda4af';
      ctx.font = 'bold 12px sans-serif';
      ctx.fillText('👻 PISTA FALSĂ (ÎN PĂDURE!)', 415, 168);

      // Hacker icon + magnet
      ctx.font = '24px sans-serif';
      ctx.fillText('👾🧲', 360, 212);
      ctx.restore();
    }

    // Green True Path vs Red Hacker Path
    if (state.mode === 'l1_hacker') {
      ctx.strokeStyle = '#ef4444';
      ctx.lineWidth = 3.5;
      ctx.setLineDash([8, 6]);
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.quadraticCurveTo(260, 195, 430, 205);
      ctx.stroke();
      ctx.setLineDash([]);
    } else if (state.mode === 'l1_shield') {
      // Cut red line
      ctx.strokeStyle = 'rgba(239,68,68,0.45)';
      ctx.lineWidth = 2.5;
      ctx.setLineDash([6, 6]);
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(250, 165);
      ctx.stroke();
      ctx.setLineDash([]);

      // Golden Scissors cutting the hacker beam
      ctx.font = '24px sans-serif';
      ctx.fillText('✂️✨', 238, 168);

      // Bright Green AERO-SHIELD Beam to True Runway
      ctx.strokeStyle = '#22c55e';
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(450, 82);
      ctx.stroke();

      ctx.fillStyle = '#fde047';
      ctx.font = 'bold 13px sans-serif';
      ctx.fillText('🛡️ SCUT ACTIV: Minciuna GPS este tăiată!', 155, 60);
    } else {
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.moveTo(95, 95);
      ctx.lineTo(450, 82);
      ctx.stroke();
    }

    // Fog Cloud in the middle
    ctx.save();
    ctx.fillStyle = state.mode === 'l1_shield' ? 'rgba(226,232,240,0.42)' : 'rgba(241,245,249,0.88)';
    ctx.beginPath();
    ctx.arc(265, 92, 46, 0, Math.PI * 2);
    ctx.arc(310, 82, 52, 0, Math.PI * 2);
    ctx.arc(350, 96, 42, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#0f172a';
    ctx.font = 'bold 13px sans-serif';
    ctx.fillText('🌫️ CEAȚĂ GROASĂ', 250, 95);
    ctx.restore();

    // Airplane animated position
    var prog = (t % 140) / 140;
    var ax = 80 + prog * 130;
    var ay = (state.mode === 'l1_hacker') ? (95 + prog * 70) : 90;
    var ang = (state.mode === 'l1_hacker') ? 0.35 : -0.03;
    drawAirplaneIcon(ctx, ax, ay, ang, state.mode === 'l1_shield' ? '#4ade80' : '#38bdf8');
  }

  function drawLesson2(ctx, w, h, t) {
    ctx.fillStyle = '#050b18';
    ctx.fillRect(0, 0, w, h);

    if (state.mode === 'l2_waves') {
      // Top half: Blue short wave bouncing off water droplets
      ctx.fillStyle = '#93c5fd';
      ctx.font = 'bold 13px sans-serif';
      ctx.fillText('👀 Lumina Obișnuită (Valuri Mici) -> Se lovește de picăturile de ceață și se întoarce!', 24, 30);

      for (var d = 0; d < 6; d++) {
        ctx.fillStyle = '#60a5fa';
        ctx.beginPath();
        ctx.arc(280 + d * 42, 70 + (d % 2) * 16, 11, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#ffffff';
        ctx.font = '11px sans-serif';
        ctx.fillText('💧', 273 + d * 42, 74 + (d % 2) * 16);
      }

      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 3;
      ctx.beginPath();
      for (var x = 30; x < 265; x += 4) {
        var y = 75 + Math.sin((x + t * 4) * 0.25) * 14;
        if (x === 30) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
      ctx.fillStyle = '#f87171';
      ctx.font = 'bold 16px sans-serif';
      ctx.fillText('💥 BING! Nu trece!', 145, 54);

      // Divider
      ctx.strokeStyle = 'rgba(148,163,184,0.3)';
      ctx.beginPath();
      ctx.moveTo(20, 130);
      ctx.lineTo(w - 20, 130);
      ctx.stroke();

      // Bottom half: Long SWIR wave stepping smoothly over droplets to the runway
      ctx.fillStyle = '#4ade80';
      ctx.font = 'bold 13px sans-serif';
      ctx.fillText('🔦 Ochiul Magic Infraroșu SWIR (Valuri Lungi) -> Pășește peste picături până la pistă!', 24, 158);

      for (var d2 = 0; d2 < 6; d2++) {
        ctx.fillStyle = 'rgba(96,165,250,0.45)';
        ctx.beginPath();
        ctx.arc(280 + d2 * 42, 205 + (d2 % 2) * 14, 10, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.strokeStyle = '#22c55e';
      ctx.lineWidth = 4;
      ctx.beginPath();
      for (var x2 = 30; x2 < 540; x2 += 5) {
        var y2 = 205 + Math.sin((x2 - t * 4) * 0.055) * 22;
        if (x2 === 30) ctx.moveTo(x2, y2);
        else ctx.lineTo(x2, y2);
      }
      ctx.stroke();
      ctx.fillStyle = '#fde047';
      ctx.font = 'bold 14px sans-serif';
      ctx.fillText('🛬 VEDE PISTA!', 475, 185);
      return;
    }

    // Default / SWIR Magic Magnifier mode
    var lensX = state.mouseInside ? state.mouseX : (300 + Math.cos(t * 0.04) * 130);
    var lensY = state.mouseInside ? state.mouseY : (130 + Math.sin(t * 0.06) * 45);

    // Thick Fog or Dark Smoke covering screen
    ctx.fillStyle = state.darkSmoke ? '#1c1917' : '#cbd5e1';
    ctx.fillRect(20, 20, w - 40, h - 40);
    ctx.fillStyle = state.darkSmoke ? '#a8a29e' : '#334155';
    ctx.font = 'bold 16px sans-serif';
    ctx.fillText(
      state.darkSmoke
        ? '🖤 FUM NEGRU DENS — Mișcă Lupa Infraroșie cu mouse-ul!'
        : '🌫️ PERETE ALB DE CEAȚĂ — Mișcă Lupa Infraroșie cu mouse-ul!',
      75,
      55
    );

    // Magic SWIR Lens Circle
    ctx.save();
    ctx.beginPath();
    ctx.arc(lensX, lensY, 78, 0, Math.PI * 2);
    ctx.clip();

    // Clear night runway inside the SWIR lens
    ctx.fillStyle = '#04121f';
    ctx.fillRect(0, 0, w, h);

    // Perspective runway inside lens
    ctx.strokeStyle = '#22c55e';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(300, 45);
    ctx.lineTo(130, 235);
    ctx.moveTo(300, 45);
    ctx.lineTo(470, 235);
    ctx.stroke();

    // Glowing green runway lights + optical flow arrows inside lens
    for (var r = 0; r < 6; r++) {
      var ry = 70 + r * 28;
      var spread = 22 + r * 24;
      ctx.fillStyle = '#4ade80';
      ctx.beginPath();
      ctx.arc(300 - spread, ry, 4.5, 0, Math.PI * 2);
      ctx.arc(300 + spread, ry, 4.5, 0, Math.PI * 2);
      ctx.fill();

      // Centerline dashes
      ctx.fillStyle = '#fde047';
      ctx.fillRect(297, ry - 6, 6, 14);
    }

    ctx.restore();

    // Lens glowing rim
    ctx.strokeStyle = '#22c55e';
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.arc(lensX, lensY, 78, 0, Math.PI * 2);
    ctx.stroke();

    ctx.fillStyle = '#4ade80';
    ctx.font = 'bold 12px sans-serif';
    ctx.fillText('🔦 LUPA SWIR 1.55μm', lensX - 58, lensY - 86);
  }

  function drawLesson3(ctx, w, h, t) {
    ctx.fillStyle = '#061020';
    ctx.fillRect(0, 0, w, h);

    var speedPx = (state.mode === 'l3_fast') ? 5.2 : 1.8;
    var stepDist = (state.mode === 'l3_fast') ? 78 : 28;

    // Airplane at top looking down with camera cone
    drawAirplaneIcon(ctx, 300, 42, 0, '#38bdf8');
    ctx.fillStyle = 'rgba(56,189,248,0.14)';
    ctx.beginPath();
    ctx.moveTo(300, 52);
    ctx.lineTo(140, 215);
    ctx.lineTo(460, 215);
    ctx.closePath();
    ctx.fill();

    // Ground track
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(20, 195, w - 40, 50);
    ctx.strokeStyle = '#334155';
    ctx.lineWidth = 2;
    ctx.strokeRect(20, 195, w - 40, 50);

    var offset = (t * speedPx) % 90;
    for (var i = -1; i < 8; i++) {
      var bx = 560 - (i * 90 + offset);
      if (bx < 30 || bx > 570) continue;

      // Previous photo ghost bulb (Poza 1)
      if (state.mode === 'l3_ruler' || state.mode === 'l3_fast' || state.mode === 'l3_slow') {
        var prevX = bx + stepDist;
        if (prevX < 565) {
          ctx.fillStyle = 'rgba(148,163,184,0.35)';
          ctx.beginPath();
          ctx.arc(prevX, 195, 7, 0, Math.PI * 2);
          ctx.fill();

          // Yellow velocity arrow from Poza 1 to Poza 2
          ctx.strokeStyle = '#fde047';
          ctx.lineWidth = 3;
          ctx.beginPath();
          ctx.moveTo(prevX, 172);
          ctx.lineTo(bx, 172);
          ctx.stroke();

          // Arrowhead
          ctx.fillStyle = '#fde047';
          ctx.beginPath();
          ctx.moveTo(bx, 172);
          ctx.lineTo(bx + 8, 167);
          ctx.lineTo(bx + 8, 177);
          ctx.closePath();
          ctx.fill();
        }
      }

      // Current bright ground light bulb (Poza 2)
      ctx.fillStyle = '#4ade80';
      ctx.beginPath();
      ctx.arc(bx, 195, 8, 0, Math.PI * 2);
      ctx.fill();
    }

    if (state.mode === 'l3_ruler') {
      // Big Yellow Ruler overlay in the center
      ctx.fillStyle = 'rgba(250,204,21,0.92)';
      ctx.fillRect(205, 112, 190, 28);
      ctx.strokeStyle = '#713f12';
      ctx.lineWidth = 2;
      ctx.strokeRect(205, 112, 190, 28);
      ctx.fillStyle = '#020617';
      ctx.font = 'bold 12px sans-serif';
      ctx.fillText('📏 RIGLA: Δu = 52 pixeli -> 85 m/s!', 212, 130);
    }

    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 14px sans-serif';
    var label = (state.mode === 'l3_fast')
      ? '🚀 VITEZĂ MARE (120 m/s): Săgețile galbene dintre Poza 1 și Poza 2 sunt LUNGI!'
      : '🐢 VITEZĂ MICĂ (45 m/s): Săgețile galbene dintre Poza 1 și Poza 2 sunt SCURTE!';
    ctx.fillText(label, 35, 240);
  }

  function drawLesson4(ctx, w, h, t) {
    ctx.fillStyle = '#060d1f';
    ctx.fillRect(0, 0, w, h);

    var isTilting = (t < state.tiltBurstUntil);
    var tiltAngle = isTilting ? Math.sin(t * 0.45) * 0.28 : 0.0;

    drawAirplaneIcon(ctx, 300, 38, tiltAngle, '#38bdf8');

    // LAYER 1: Floating Purple Cloud pushed by wind (Middle)
    var cloudShift = (t * 2.4) % 80;
    ctx.fillStyle = 'rgba(168, 85, 247, 0.28)';
    ctx.strokeStyle = '#c084fc';
    ctx.lineWidth = 2;
    ctx.fillRect(60, 82, w - 120, 62);
    ctx.strokeRect(60, 82, w - 120, 62);

    ctx.fillStyle = '#e9d5ff';
    ctx.font = 'bold 12.5px sans-serif';
    ctx.fillText('☁️ STRATUL 1: NORUL MOV ÎMPINS DE VÂNT 💨 (Se mișcă strâmb!)', 75, 102);

    for (var c = 0; c < 5; c++) {
      var cx = 95 + ((c * 95 + cloudShift) % 440);
      ctx.strokeStyle = '#f43f5e';
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.moveTo(cx, 122);
      ctx.lineTo(cx + 32, 114);
      ctx.stroke();

      if (state.mode === 'l4_separate') {
        ctx.fillStyle = '#ef4444';
        ctx.font = 'bold 15px sans-serif';
        ctx.fillText('❌', cx + 10, 134);
      }
    }

    // LAYER 2: Solid Green Ground (Bottom)
    ctx.fillStyle = 'rgba(22, 163, 74, 0.22)';
    ctx.strokeStyle = '#4ade80';
    ctx.lineWidth = 2;
    ctx.fillRect(40, 168, w - 80, 68);
    ctx.strokeRect(40, 168, w - 80, 68);

    ctx.fillStyle = '#bbf7d0';
    ctx.font = 'bold 12.5px sans-serif';
    ctx.fillText('🌍 STRATUL 2: PĂMÂNTUL VERDE FIX ✅ (Adevărul folosit de AERO-SHIELD!)', 58, 190);

    var groundShift = (t * 1.8) % 90;
    for (var g = 0; g < 5; g++) {
      var gx = 520 - ((g * 90 + groundShift) % 450);
      ctx.strokeStyle = '#4ade80';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(gx, 215);
      ctx.lineTo(gx - 28, 215);
      ctx.stroke();
      ctx.fillStyle = '#4ade80';
      ctx.fillText('✅', gx - 18, 230);
    }

    if (isTilting) {
      ctx.fillStyle = 'rgba(250, 204, 21, 0.96)';
      ctx.fillRect(65, 48, 470, 30);
      ctx.fillStyle = '#020617';
      ctx.font = 'bold 12.5px sans-serif';
      ctx.fillText('📢 „Stai liniștit, doar s-a înclinat avionul (ω×r), nu am sărit de pe pistă!”', 76, 68);
    }
  }

  function drawLesson5(ctx, w, h, t) {
    ctx.fillStyle = '#060c1a';
    ctx.fillRect(0, 0, w, h);

    // Update Lie Cup for Slow Lie (fills in ~3 seconds = 180 frames at 60fps)
    if (state.mode === 'l5_slow_lie') {
      var elapsed = t - state.slowLieStartTick;
      state.cupLevel = Math.min(1.0, elapsed / 180.0);
    }

    var isAlarm = (state.mode === 'l5_big_lie') || (state.cupLevel >= 0.99);

    // Left Side: Truth Balance Scale (Chi-Square Gate)
    var scaleCx = 195;
    var scaleCy = 135;
    var beamTilt = (state.mode === 'l5_big_lie') ? 0.42 : (state.mode === 'l5_slow_lie' ? 0.08 : 0.0);

    ctx.fillStyle = '#e2e8f0';
    ctx.font = 'bold 13px sans-serif';
    ctx.fillText('⚖️ 1. CÂNTARUL ADEVĂRULUI (Chi-Pătrat)', 50, 32);

    // Pillar
    ctx.fillStyle = '#64748b';
    ctx.fillRect(scaleCx - 6, scaleCy, 12, 85);
    ctx.fillRect(scaleCx - 45, scaleCy + 80, 90, 10);

    // Tilting Beam
    ctx.save();
    ctx.translate(scaleCx, scaleCy);
    ctx.rotate(beamTilt);
    ctx.strokeStyle = isAlarm ? '#f87171' : '#fde047';
    ctx.lineWidth = 5;
    ctx.beginPath();
    ctx.moveTo(-95, 0);
    ctx.lineTo(95, 0);
    ctx.stroke();

    // Left Pan (Vision + IMU)
    ctx.fillStyle = '#22c55e';
    ctx.fillRect(-125, 22, 65, 24);
    ctx.fillStyle = '#020617';
    ctx.font = 'bold 11px sans-serif';
    ctx.fillText('👀 Ochi+IMU', -122, 38);

    // Right Pan (GPS)
    ctx.fillStyle = isAlarm ? '#ef4444' : '#38bdf8';
    ctx.fillRect(60, 22, 65, 24);
    ctx.fillStyle = '#ffffff';
    ctx.fillText(state.mode === 'l5_big_lie' ? '⚡ +500m!' : '📡 GPS', 66, 38);
    ctx.restore();

    // Right Side: The Lie Cup (Paharul de Minciuni CUSUM)
    var cupX = 430;
    var cupY = 68;
    var cupW = 96;
    var cupH = 135;

    ctx.fillStyle = '#e2e8f0';
    ctx.font = 'bold 13px sans-serif';
    ctx.fillText('🥛 2. PAHARUL DE MINCIUNI (CUSUM)', 355, 32);

    // Falling red droplets in slow lie mode
    if (state.mode === 'l5_slow_lie' && state.cupLevel < 1.0) {
      var dropY = 42 + ((t * 4) % 45);
      ctx.fillStyle = '#ef4444';
      ctx.beginPath();
      ctx.arc(cupX + cupW / 2, dropY, 6, 0, Math.PI * 2);
      ctx.fill();
    }

    // Liquid inside cup
    var fillH = Math.round(cupH * state.cupLevel);
    if (fillH > 0) {
      ctx.fillStyle = isAlarm ? '#dc2626' : '#f59e0b';
      ctx.fillRect(cupX + 4, cupY + cupH - fillH, cupW - 8, fillH - 2);
    }

    // Cup glass outline
    ctx.strokeStyle = '#94a3b8';
    ctx.lineWidth = 3;
    ctx.strokeRect(cupX, cupY, cupW, cupH);

    // Red Alarm Threshold Line
    ctx.strokeStyle = '#ef4444';
    ctx.setLineDash([5, 4]);
    ctx.beginPath();
    ctx.moveTo(cupX - 10, cupY + 18);
    ctx.lineTo(cupX + cupW + 10, cupY + 18);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#fca5a5';
    ctx.font = 'bold 11px sans-serif';
    ctx.fillText('PRAG ALARMĂ', cupX + 8, cupY + 14);

    if (isAlarm) {
      ctx.fillStyle = '#ef4444';
      ctx.font = 'bold 15px sans-serif';
      ctx.fillText('🚨 ALARMĂ! MINCIUNA GPS A FOST PRINSĂ ȘI TĂIATĂ! ✂️', 95, 246);
    } else {
      ctx.fillStyle = '#4ade80';
      ctx.font = 'bold 14px sans-serif';
      ctx.fillText('✅ Totul este cinstit (Nivel Pahar: ' + Math.round(state.cupLevel * 100) + '%)', 145, 246);
    }
  }

  function drawLesson6(ctx, w, h, t) {
    ctx.fillStyle = '#071126';
    ctx.fillRect(0, 0, w, h);

    // Golden Commander Badge & 3 Miniature Dashboard Cards
    ctx.strokeStyle = '#fde047';
    ctx.lineWidth = 3;
    ctx.strokeRect(20, 18, w - 40, h - 36);

    ctx.fillStyle = '#fde047';
    ctx.font = 'bold 17px sans-serif';
    ctx.fillText('🏅 DIPLOMĂ DE COMANDANT AERO-SHIELD V2.0 👩‍✈️👨‍✈️', 88, 52);

    ctx.fillStyle = '#e2e8f0';
    ctx.font = '13px sans-serif';
    ctx.fillText('Acum știi cum funcționează cele 3 panouri mari de mai jos:', 125, 78);

    // 3 Mini-Boxes with Bouncing Yellow Arrows pointing down to the main simulator
    var boxes = [
      { x: 45, title: '1. PARBRIZUL MAGIC', sub: 'SWIR + Flux Optic' },
      { x: 225, title: '2. CURSA 4 AVIOANE', sub: 'Verde vs. Hacker Roșu' },
      { x: 405, title: '3. TELECOMANDA', sub: 'Cântarul + Film Real' }
    ];

    var bounce = Math.sin(t * 0.12) * 6;
    for (var i = 0; i < boxes.length; i++) {
      var b = boxes[i];
      ctx.fillStyle = 'rgba(30, 41, 59, 0.95)';
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 2;
      ctx.fillRect(b.x, 96, 150, 74);
      ctx.strokeRect(b.x, 96, 150, 74);

      ctx.fillStyle = '#ffffff';
      ctx.font = 'bold 12px sans-serif';
      ctx.fillText(b.title, b.x + 10, 122);
      ctx.fillStyle = '#7dd3fc';
      ctx.font = '11.5px sans-serif';
      ctx.fillText(b.sub, b.x + 10, 145);

      // Glowing yellow arrow pointing down toward the main dashboard
      ctx.fillStyle = '#fde047';
      ctx.font = 'bold 24px sans-serif';
      ctx.fillText('⬇️', b.x + 62, 205 + bounce);
    }

    ctx.fillStyle = '#4ade80';
    ctx.font = 'bold 13px sans-serif';
    ctx.fillText('👇 Privește mai jos pe Ecranul Mare al Simulatorului! 👇', 120, 232);
  }

  function renderLoop() {
    state.tick += 1;
    var canvas = document.getElementById('lessonInteractiveCanvas');
    if (canvas && canvas.getContext) {
      var ctx = canvas.getContext('2d');
      if (ctx) {
        var w = canvas.width;
        var h = canvas.height;
        if (state.step === 1) drawLesson1(ctx, w, h, state.tick);
        else if (state.step === 2) drawLesson2(ctx, w, h, state.tick);
        else if (state.step === 3) drawLesson3(ctx, w, h, state.tick);
        else if (state.step === 4) drawLesson4(ctx, w, h, state.tick);
        else if (state.step === 5) drawLesson5(ctx, w, h, state.tick);
        else drawLesson6(ctx, w, h, state.tick);
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
            assert_eq!(lesson.action_buttons.len(), 3);
        }
    }

    #[test]
    fn test_render_lessons_6yo_component_contains_canvas_and_all_lessons() {
        let html = render_lessons_6yo_component();
        assert!(html.contains("id=\"lessonInteractiveCanvas\""));
        assert!(html.contains("width=\"600\""));
        assert!(html.contains("height=\"260\""));
        assert!(html.contains("Lecția 1: „Problema: Ceața și Hackerul Rău 👾”"));
        assert!(html.contains("Lecția 2: „Ochiul Magic care Vede prin Ceață și Fum 👀”"));
        assert!(html.contains("Lecția 3: „Cum aflăm viteza fără GPS? (Numărăm becurile de pe jos!) 💡”"));
        assert!(html.contains("Lecția 4: „Capcana Norului Împins de Vânt și a Înclinării Avionului! 💨”"));
        assert!(html.contains("Lecția 5: „Cântarul Adevărului: Cum prindem orice minciună? ⚖️”"));
        assert!(html.contains("Lecția 6: „Acum ești Comandant! Testează pe Simulatorul Mare și pe Filmarea Reală!”"));
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

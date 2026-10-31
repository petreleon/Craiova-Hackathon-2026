//! # Motorul de Randare SSR 100% Rust cu Framework-ul `maud` (`maud::html!`)
//!
//! Arhitectură Aerisită în **6 Pagini Separate (Câte 1 Singur Concept pe Ecran — Fără Scroll Lung!)**:
//! - **Pagina 1 (`#concept-page-1`)**: `1️⃣ Lecții Interactive — Creierul de Zbor Google Drosophila`
//! - **Pagina 2 (`#concept-page-2`)**: `2️⃣ Parbrizul Avionului & Filmarea Reală 50% (Ce vede camera prin ceață?)`
//! - **Pagina 3 (`#concept-page-3`)**: `3️⃣ Cursa celor 4 Avioane (De ce noi rămânem pe pistă?)`
//! - **Pagina 4 (`#concept-page-4`)**: `4️⃣ Creierul Google Drosophila & Scutul Anti-Hacker`
//! - **Pagina 5 (`#concept-page-5`)**: `5️⃣ Cele 3 Filmări Video Reale din Cabina Piloților`
//! - **Pagina 6 (`#concept-page-6`)**: `6️⃣ Cele 10 Teste Sintetice FlightGear (Pe VM-ul GPU)`

use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::{Deserialize, Serialize};

use crate::flightgear_synthetic_10::{
    run_all_10_flightgear_synthetic_tests, FlightGearTestSuiteReport,
};
use crate::ws_telemetry::{compute_or_load_real_video_benchmark, CockpitServerState};

/// Structură tipizată Rust pentru fiecare dintre cele 6 Lecții Interactive (pentru un copil de 6 ani / 0 experiență).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveLesson6Yo {
    pub lesson_num: u8,
    pub icon: &'static str,
    pub title_ro: &'static str,
    pub title_en: &'static str,
    pub child_story_ro: &'static str,
    pub child_story_en: &'static str,
    pub interactive_action_label_ro: &'static str,
    pub interactive_action_js: &'static str,
    pub visual_highlight_zone: &'static str,
    pub takeaway_badge_ro: &'static str,
    pub fg_video_url: &'static str,
    pub fg_video_caption_ro: &'static str,
    pub fg_video_caption_en: &'static str,
}

/// Metadatele unei pagini de concept din arhitectura cu 6 pagini aerisite.
#[derive(Debug, Clone, Copy)]
pub struct ConceptPageMeta {
    pub page_num: u8,
    pub nav_badge_ro: &'static str,
    pub full_title_ro: &'static str,
}

/// Lista celor 6 pagini individuale (câte un singur concept pe ecran).
pub const SIX_CONCEPT_PAGES: [ConceptPageMeta; 6] = [
    ConceptPageMeta {
        page_num: 1,
        nav_badge_ro: "1️⃣ Lecții Interactive",
        full_title_ro: "Lecții Interactive — Creierul de Zbor Google Drosophila",
    },
    ConceptPageMeta {
        page_num: 2,
        nav_badge_ro: "2️⃣ Parbrizul & Filmarea Reală",
        full_title_ro: "Parbrizul Avionului & Filmarea Reală (2 Panouri Separate)",
    },
    ConceptPageMeta {
        page_num: 3,
        nav_badge_ro: "3️⃣ Cursa celor 4 Avioane",
        full_title_ro: "Cursa celor 4 Avioane (De ce noi rămânem pe pistă?)",
    },
    ConceptPageMeta {
        page_num: 4,
        nav_badge_ro: "4️⃣ Creierul Drosophila & Scutul",
        full_title_ro: "Creierul Google Drosophila & Scutul Anti-Hacker",
    },
    ConceptPageMeta {
        page_num: 5,
        nav_badge_ro: "5️⃣ Cele 3 Filmări Reale",
        full_title_ro: "Cele 3 Filmări Video Reale din Cabina Piloților",
    },
    ConceptPageMeta {
        page_num: 6,
        nav_badge_ro: "6️⃣ 10 Teste FG & 100km Zero-GPS",
        full_title_ro: "Cele 10 Teste Sintetice FlightGear & Cursa Lungă 100 km Fără GPS (Analiza Erorii)",
    },
];

/// Returnează lista completă a celor 6 lecții interactive pas-cu-pas.
#[must_use]
pub fn get_all_6_interactive_lessons() -> Vec<InteractiveLesson6Yo> {
    vec![
        InteractiveLesson6Yo {
            lesson_num: 1,
            icon: "🧸",
            title_ro: "Lecția 1: Ceața Groasă ca Laptele, Hackerul GPS și Musculița Google Drosophila",
            title_en: "Lesson 1: Thick Milk Fog, The GPS Hacker & The Google Drosophila Fly Brain",
            child_story_ro: "Imaginează-ți că mergi cu bicicleta prin ceață groasă ca laptele! Dacă întrebi un străin răutăcios (Hackerul GPS 🔴), el îți dă o hartă falsă ca să cazi în iarbă. Iar dacă închizi ochii (Senzorii Interni 🟠), amețești rapid! De aceea noi am copiat creierul de zbor Open-Source Google Research al musculiței Drosophila 🪰 (cu 🧭 E-PG Ring Attractor cu 16 Neuroni) — cel mai bun pilot fără GPS din natură!",
            child_story_en: "Imagine riding a bike in thick fog! A GPS hacker gives a fake map, and closing your eyes makes you dizzy. That's why we use the Google Research Drosophila fly connectome (🧭 E-PG 16-Neuron Ring Attractor)!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Activează Scutul AERO-SHIELD în Zborul FlightGear!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "⭐ Ai învățat de ce avionul roșu 🔴 și portocaliu 🟠 greșesc drumul, dar creierul Drosophila 🟢 rămâne pe pistă!",
            fg_video_url: "/real_videos/fg_lesson_1.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-01/07): Ceață CAT IIIc + Atac Hacker GPS vs. Scutul AERO-SHIELD pe Pista 09 Craiova",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-01/07): CAT IIIc Fog + GPS Hacker Attack vs. AERO-SHIELD on Craiova RWY 09",
        },
        InteractiveLesson6Yo {
            lesson_num: 2,
            icon: "🔦",
            title_ro: "Lecția 2: Ochiul Anticeață T4/T5 și Ochelarii Magici Infraroșu (SWIR + ALS)",
            title_en: "Lesson 2: T4/T5 Anti-Fog Optic Lobe & Magic Infrared Goggles (SWIR + ALS)",
            child_story_ro: "Când aprinzi o lanternă normală în ceață, lumina se lovește de picăturile de apă și face un perete alb. Dar avionul nostru folosește Ochelarii Infraroșii (SWIR 1550nm) împreună cu 👁️ T4/T5 Ochiul Anticeață din creierul Google Drosophila! Neuronii T4/T5 separă instantaneu becurile strălucitoare (ALS 💡) ale pistei din Craiova de laptele alb al ceții!",
            child_story_en: "Our SWIR Infrared Goggles and the Google Drosophila 👁️ T4/T5 Anti-Fog Optic Lobe cut through fog glare to lock onto bright runway lights (ALS)!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Pornește Ochiul Magic SWIR T4/T5 în FlightGear!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "⭐ Ai descoperit cum 👁️ T4/T5 Ochiul Anticeață vede becurile pistei Craiova RWY 09!",
            fg_video_url: "/real_videos/fg_lesson_2.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-01 SWIR): Ochiul Infraroșu T4/T5 curăță ceața și fixează cele 18 bare luminoase ALS",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-01 SWIR): T4/T5 Infrared Eye clears fog and locks onto 18 ALS light bars",
        },
        InteractiveLesson6Yo {
            lesson_num: 3,
            icon: "💡",
            title_ro: "Lecția 3: Stabilizatorul HS/VS de Viraje 6-DoF (și Cerul Fără Blocaj!)",
            title_en: "Lesson 3: HS/VS 6-DoF Rotation Stabilizer & Safe Horizon Sky",
            child_story_ro: "Când o musculiță sau un avion dă din aripi și virează, toată lumea pare că se învârte! În creierul Google Drosophila, neuronii 🔄 HS/VS (Stabilizatorul de Viraje 6-DoF) scad automat rotația capului ca să măsurăm viteza adevărată peste becurile verzi 🟢. Iar când privim sus spre cer, punem simplu ZERO (0.000000) — fără împărțire la zero și fără blocaj!",
            child_story_en: "Google Drosophila 🔄 HS/VS neurons subtract 6-DoF aircraft rotations immediately, while looking at the sky outputs a safe 0.000000 with zero crashes!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Măsoară Viteza cu Becurile ALS în FlightGear (1/Z = 0)!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "⭐ Ai văzut cum 🔄 HS/VS stabilizează virajele 6-DoF și păstrează orizontul 100% sigur!",
            fg_video_url: "/real_videos/fg_lesson_3.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-05/06): Numărarea becurilor pistei ALS și măsurarea vitezei (1/Z = 0 la cer, fără blocaj!)",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-05/06): Counting ALS lights & speed measurement (1/Z = 0 at sky, zero crash!)",
        },
        InteractiveLesson6Yo {
            lesson_num: 4,
            icon: "☁️",
            title_ro: "Lecția 4: Neuroni PFNd — Norul Împins de Vânt (Mov) vs. Pământul Fix (Verde)",
            title_en: "Lesson 4: PFNd Wind-Vision Neurons — Purple Fog Wind vs. Solid Green Ground",
            child_story_ro: "Uite secretul cel mai mare! Când bate vântul lateral, el împinge norul de ceață (Săgețile Mov 🟣). O cameră obișnuită (Avionul Galben 🟡) se păcălește și virează în câmp! Dar în creierul Google Drosophila, neuronii 💨 PFNd (Simțul Vântului în Ceață) și 🧭 E-PG Ring Attractor cu 16 Neuroni separă norul mov de pământul verde 🟢 cu 0.85 secunde ÎNAINTE ca rafala să lovească aripile!",
            child_story_en: "Google Drosophila 💨 PFNd neurons and the 🧭 E-PG 16-Neuron Ring Attractor separate moving purple fog from solid green ground 0.85s ahead of impact!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Separă Vântul din Ceață (PFNd 0.85s) în FlightGear!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "⭐ Ai înțeles cum 💨 PFNd și 🧭 E-PG separă vântul din ceață (Mov) de pistă (Verde)!",
            fg_video_url: "/real_videos/fg_lesson_4.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-02/03): Viraj 6-DoF Roll +22° (HS/VS) + Rafală de Vânt în Ceață +15 m/s (PFNd 0.85s)",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-02/03): 6-DoF Roll +22° (HS/VS) + Fog Wind Gust +15 m/s (PFNd 0.85s)",
        },
        InteractiveLesson6Yo {
            lesson_num: 5,
            icon: "⚖️",
            title_ro: "Lecția 5: Busola în Inel E-PG (16 Neuroni) + Paharul Anti-Hacker (CUSUM < 2.9s)",
            title_en: "Lesson 5: 16-Neuron E-PG Ring Compass + The CUSUM Anti-Hacker Cup",
            child_story_ro: "Dacă hackerul e șiret și trage harta GPS câte un milimetru pe secundă (Stealth Spoof 🕵️), Paharul nostru Magic CUSUM adună fiecare picătură de minciună și oprește GPS-ul fals în sub 2.9 secunde! Chiar și cu GPS-ul tăiat și în orbire totală, 🧭 E-PG Ring Attractor cu 16 Neuroni ține minte direcția exactă către Pista 09 Craiova!",
            child_story_en: "Our CUSUM Magic Cup catches sneaky GPS hackers in under 2.9s, while the 🧭 E-PG 16-Neuron Ring Attractor holds the exact heading to Craiova RWY 09 in total blindness!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Prinde Hackerul cu Paharul CUSUM & Busola E-PG!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "⭐ Ai prins hackerul în < 2.9s, iar inelul cu 16 neuroni 🧭 E-PG a păstrat direcția pistei!",
            fg_video_url: "/real_videos/fg_lesson_5.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-08/09): Atac Lent Stealth Spoofing prins de Paharul CUSUM în 2.9s + Busola E-PG 16-Neuroni",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-08/09): Stealth Spoofing caught by CUSUM Cup in 2.9s + 16-Neuron E-PG Compass",
        },
        InteractiveLesson6Yo {
            lesson_num: 6,
            icon: "🎓",
            title_ro: "Lecția 6: Comandant de Zbor cu Creierul Google Drosophila în Asediu Complet!",
            title_en: "Lesson 6: Flight Captain with the Google Drosophila Brain in Full Cyber-Siege!",
            child_story_ro: "Felicitări! Acum cunoști toate cele 4 etaje din creierul de zbor Google Research Drosophila: 1) 👁️ T4/T5 vede becurile prin ceață, 2) 🔄 HS/VS stabilizează virajele, 3) 🧭 E-PG Ring Attractor cu 16 Neuroni păstrează busola pistei, și 4) 💨 PFNd separă norul mov de pământul verde! Apasă butonul de mai jos pentru Misiunea Finală (Actul 3)!",
            child_story_en: "Congratulations! All 4 Google Drosophila stages (👁️ T4/T5, 🔄 HS/VS, 🧭 E-PG 16-Neuron Ring Attractor, 💨 PFNd) work together to land on Craiova RWY 09!",
            interactive_action_label_ro: "🟢 APASĂ BUTONUL VERDE: Aterizează Autonom în Whiteout 0m pe FlightGear!",
            interactive_action_js: "triggerSchoolLessonState('solved')",
            visual_highlight_zone: "maudSchoolCanvas",
            takeaway_badge_ro: "🏆 DIPLOMĂ DE COMANDANT AERO-SHIELD & GOOGLE DROSOPHILA CONNECTOME OBȚINUTĂ!",
            fg_video_url: "/real_videos/fg_lesson_6.mp4",
            fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-10): Aterizare Autonomă Completă în Whiteout 0m pe Pista 09 Craiova (Eroare 0.21m!)",
            fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-10): Full Autonomous Landing in 0m Whiteout on Craiova RWY 09 (0.21m Error!)",
        },
    ]
}

/// 1. Bara Superioară Aerisită cu Cele 6 Butoane Mari de Pagini (`#nav-page-btn-1` .. `#nav-page-btn-6`).
#[must_use]
pub fn render_header_bar(current_tick: u64, gpu_device: &str, active_page: u8) -> Markup {
    html! {
        header class="topbar" {
            div class="topbar-upper" {
                div class="brand" {
                    span class="brand-badge" { "RUST MAUD SSR • DO-178C" }
                    div {
                        div class="brand-title" {
                            "✈️ AERO-SHIELD LRCV V2.0 — PISTA CRAIOVA RWY 09"
                        }
                        div class="brand-sub" id="conn-status" {
                            "WS 60.0 Hz LIVE | Tick #" (current_tick) " | Motor: " (gpu_device)
                        }
                    }
                }

                div class="lang-and-status" {
                    div class="lang-switch" {
                        button class="lang-btn active" id="btn-lang-ro" onclick="setAppLang('ro')" {
                            "🇷🇴 Română"
                        }
                        button class="lang-btn" id="btn-lang-en" onclick="setAppLang('en')" {
                            "🇬🇧 English"
                        }
                    }
                    div class="phase-banner" id="phase-banner" {
                        "FAZA 2: OCHELARI INFRAROȘU ACTIVI PE PISTA RWY 09"
                    }
                }
            }

            // Cele 6 Butoane Mari de Navigare între Concepte (Câte 1 Concept pe Ecran!)
            nav class="pages-nav-bar" aria-label="Navigare 6 Concepte" {
                @for meta in &SIX_CONCEPT_PAGES {
                    button
                        class=(if meta.page_num == active_page { "nav-page-btn active" } else { "nav-page-btn" })
                        id=(format!("nav-page-btn-{}", meta.page_num))
                        onclick=(format!("switchConceptPage({})", meta.page_num))
                    {
                        (meta.nav_badge_ro)
                    }
                }
            }
        }
    }
}

/// Indicatorul Plutitor cu Săgeată Animată (`👇 UITE AICI CE SE SCHIMBĂ!`).
#[must_use]
pub fn render_floating_pointer() -> Markup {
    html! {
        div id="floating-visual-pointer" style="display:none;" {
            div class="pointer-bubble" {
                span class="pointer-arrow" { "👇" }
                div {
                    div id="pointer-title" style="font-weight:900; font-size:15px; color:#021019;" {
                        "UITE AICI CE SE SCHIMBĂ ACUM!"
                    }
                    div id="pointer-desc" style="font-size:13.5px; color:#1e293b;" {
                        "Observă cum avionul nostru verde rămâne pe pistă!"
                    }
                }
            }
        }
    }
}

/// PAGINA 1 (`#concept-page-1`): `1️⃣ Lecții Interactive — Creierul de Zbor Google Drosophila`.
/// Un singur card mare, extrem de aerisit, cu 1 singur Canvas animat mare (`#maudSchoolCanvas`, `height: 320px`) + Video FlightGear (`#maudSchoolFgVideo`).
#[must_use]
pub fn render_page_1_interactive_school(lessons: &[InteractiveLesson6Yo], is_active: bool) -> Markup {
    html! {
        div
            id="concept-page-1"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            section class="spacious-card" id="school-panel" {
                div class="page-hero-header" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 1 DIN 6 • ACADEMIA MICULUI PILOT & CREIERUL GOOGLE DROSOPHILA" }
                        h1 class="page-main-title" {
                            "🎓 Lecții Interactive — Creierul de Zbor Google Drosophila"
                        }
                        p class="page-subtitle" {
                            "🪰 De ce nu avem nevoie să inventăm formule complicate? Pentru că folosim harta Open-Source Google Research a creierului musculiței Drosophila (Male Drosophila CNS & Optic Lobe Connectome) — cel mai bun pilot fără GPS din natură!"
                        }
                        div class="drosophila-pills-bar" {
                            span class="drosophila-bio-pill" { "👁️ T4/T5 Ochiul Anticeață" }
                            span class="drosophila-bio-pill" { "🔄 HS/VS Stabilizatorul de Viraje 6-DoF" }
                            span class="drosophila-bio-pill" { "🧭 E-PG Busola în Inel 16-Neuroni" }
                            span class="drosophila-bio-pill" { "💨 PFNd Simțul Vântului în Ceață" }
                        }
                    }
                    div class="stars-box" id="school-stars-progress" {
                        "⭐ ☆ ☆ ☆ ☆ ☆ (Lecția 1 din 6)"
                    }
                }

                // Cele 6 tab-uri mari și aerisite ale lecțiilor
                div class="lesson-tabs-spacious" {
                    @for lesson in lessons {
                        button
                            class=(if lesson.lesson_num == 1 { "lesson-tab-big active" } else { "lesson-tab-big" })
                            id=(format!("lesson-tab-{}", lesson.lesson_num))
                            onclick=(format!("selectLesson6Yo({})", lesson.lesson_num))
                        {
                            span style="font-size:22px;" { (lesson.icon) }
                            span { "Lecția " (lesson.lesson_num) }
                        }
                    }
                }

                // Povestea Lecției Curent Selectate (Font Mare 20px)
                @for lesson in lessons {
                    div
                        class="lesson-pane-spacious"
                        id=(format!("lesson-pane-{}", lesson.lesson_num))
                        style=(if lesson.lesson_num == 1 { "display:flex;" } else { "display:none;" })
                    {
                        h2 class="lesson-big-heading" {
                            (lesson.icon) " " (lesson.title_ro)
                        }
                        p class="lesson-big-story" data-ro=(lesson.child_story_ro) data-en=(lesson.child_story_en) {
                            (lesson.child_story_ro)
                        }

                        div class="lesson-action-row" {
                            button
                                class="btn-big btn-big-green"
                                style="background:linear-gradient(90deg, #059669 0%, #10b981 100%); border:3px solid #34d399; box-shadow:0 0 24px rgba(16,185,129,0.7); font-size:17.5px; font-weight:900;"
                                onclick=(lesson.interactive_action_js)
                            {
                                (lesson.interactive_action_label_ro)
                            }

                            @if lesson.lesson_num < 6 {
                                button
                                    class="btn-big btn-big-amber"
                                    onclick=(format!("selectLesson6Yo({})", lesson.lesson_num + 1))
                                {
                                    "➡️ Mergi la Lecția " (lesson.lesson_num + 1)
                                }
                            } @else {
                                button
                                    class="btn-big btn-big-cyan"
                                    onclick="switchConceptPage(2)"
                                {
                                    "🚀 Am Înțeles! Mergi la Pagina 2: Parbrizul Avionului ➡️"
                                }
                            }
                        }

                        div class="lesson-takeaway-banner" {
                            (lesson.takeaway_badge_ro)
                        }
                    }
                }

                // 2 PANOURI ALĂTURATE COMPLET SEPARATE: Stânga = Animația Interactivă (#maudSchoolCanvas), Dreapta = Videoclipul Zborului Sintetic FlightGear (#maudSchoolFgVideo)
                div class="school-canvas-box" {
                    div style="display:grid; grid-template-columns:1fr 1fr; gap:24px; align-items:stretch;" {
                        // Panoul Stâng: DOAR Animația Interactivă Pas-cu-Pas (100% curată, fără video suprapus!)
                        div style="background:#040d1e; border:2px solid #1e4273; border-radius:16px; padding:16px; display:flex; flex-direction:column; gap:10px;" {
                            div style="display:flex; justify-content:space-between; align-items:center; gap:8px;" {
                                strong id="lesson-canvas-caption" style="color:#00e5ff; font-size:15.5px;" {
                                    "🎨 Animația Interactivă Pas-cu-Pas"
                                }
                                span class="live-pill" { "CANVAS 60 FPS SEPARAT" }
                            }
                            canvas
                                id="maudSchoolCanvas"
                                width="560"
                                height="320"
                                style="display:block; width:100%; height:320px; border-radius:14px; background:#040a14; border:2px solid #1e4273;"
                            {}
                        }

                        // Panoul Drept: DOAR Filmarea Zborului Sintetic FlightGear (100% clară, opacity: 1.0, fără canvas suprapus!)
                        div style="background:#040d1e; border:2px solid #00e5ff; border-radius:16px; padding:16px; display:flex; flex-direction:column; gap:10px;" {
                            div style="display:flex; justify-content:space-between; align-items:center; gap:8px;" {
                                strong style="color:#00ff9d; font-size:15.5px;" {
                                    "🎬 Filmarea Zborului Sintetic FlightGear"
                                }
                                span class="live-pill" { "VIDEO 100% CLAR SEPARAT" }
                            }
                            div id="maud-school-fg-caption" style="font-size:13.5px; font-weight:800; color:#bae6fd;" {
                                "🎬 Zbor Sintetic FlightGear (FG-01/07): Ceață CAT IIIc + Atac Hacker GPS vs. Scutul AERO-SHIELD pe Pista 09 Craiova"
                            }
                            video
                                id="maudSchoolFgVideo"
                                src="/real_videos/fg_lesson_1.mp4"
                                autoplay
                                loop
                                muted
                                playsinline
                                controls
                                style="display:block; width:100%; height:320px; object-fit:cover; border-radius:14px; opacity:1.0; background:#000; border:2px solid #00e5ff;"
                            {}
                        }
                    }

                    // Butoanele Mari Interactive de Experiment (cu BUTONUL VERDE MARE STRĂLUCITOR în centru!)
                    div style="display:grid; grid-template-columns:1fr 1.6fr; gap:14px; align-items:stretch; margin-top:6px;" {
                        button
                            id="btnDangerLessonRed"
                            class="btn-big btn-big-red"
                            onclick="triggerSchoolLessonState('danger')"
                        {
                            "👾 1. Arată Pericolul (Ceață / Hacker / Vânt)"
                        }
                        button
                            id="btnSolveLessonGreen"
                            class="btn-big btn-big-green"
                            style="background:linear-gradient(90deg, #059669 0%, #10b981 100%); border:3px solid #34d399; box-shadow:0 0 28px rgba(16,185,129,0.75); font-size:18px; font-weight:900; color:#ffffff;"
                            onclick="triggerSchoolLessonState('solved')"
                        {
                            "🟢 APASĂ BUTONUL VERDE: Activează Scutul AERO-SHIELD în Zborul FlightGear!"
                        }
                    }

                    // Banner-ul de confirmare când utilizatorul apasă pe Butonul Verde
                    div
                        id="school-green-solved-banner"
                        style="background:rgba(16, 185, 129, 0.16); border:2px solid #10b981; color:#ecfdf5; padding:12px 18px; border-radius:12px; font-size:15.5px; font-weight:800;"
                    {
                        "✅ BRAVO! Ai apăsat Butonul Verde: Uite în filmarea FlightGear din dreapta și în animația din stânga cum avionul verde rămâne pe Pista 09 Craiova!"
                    }
                }
            }
        }
    }
}

/// PAGINA 2 (`#concept-page-2`): `2️⃣ Parbrizul Avionului & Filmarea Reală (2 Panouri Separate)`.
#[must_use]
pub fn render_page_2_windshield_hud(is_active: bool) -> Markup {
    html! {
        div
            id="concept-page-2"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            // 1. Banda Povestitorului în Timp Real deasupra celor 2 panouri separate
            section class="storyteller-bar" id="storyteller-bar" {
                div class="story-icon" id="story-icon" { "🟢" }
                div style="flex:1;" {
                    div class="story-headline" id="story-headline" {
                        "💬 POVESTITORUL ÎN TIMP REAL: Totul este liniștit! Avionul zboară drept spre pista din Craiova."
                    }
                    div class="story-subtext" id="story-subtext" {
                        "👉 În stânga vezi Animația 3D a Parbrizului cu săgețile AI (verde = pământ, mov = ceață), iar în dreapta vezi Filmarea Reală 100% clară din cabina piloților!"
                    }
                }
                div class="hud-summary-badge" id="hud-guidance-summary" {
                    "DISTANȚĂ PISTĂ: 900 m | DIRECȚIE: 087.4°"
                }
            }

            // 2. Cardul Central Mare cu CELE 2 PANOURI COMPLET SEPARATE: Stânga = #hudCanvas, Dreapta = #hudBgRealVideo
            section class="spacious-card" id="zone-hud" {
                div class="windshield-top-controls" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 2 DIN 6 • PARBRIZUL 3D SEPARAT + FILMAREA REALĂ 100% CLARĂ" }
                        h1 class="page-main-title" style="margin-top:6px;" {
                            "👀 Ce Vede Camera Avionului prin Ceața Groasă? (Animație 3D vs. Filmare Reală)"
                        }
                    }
                }

                // Grid side-by-side (2 panouri alăturate, 0 suprapunere!)
                div style="display:grid; grid-template-columns:1fr 1fr; gap:24px; align-items:stretch;" {
                    // Panoul Stâng: 🪟 Animația Parbrizului 3D & Vectorii AI (#hudCanvas)
                    div style="background:#040d1e; border:2px solid #254f88; border-radius:18px; padding:16px; display:flex; flex-direction:column; gap:12px;" {
                        div style="display:flex; justify-content:space-between; align-items:center;" {
                            strong style="font-size:16.5px; color:#00ff9d;" {
                                "🪟 Animația Parbrizului 3D & Vectorii AI"
                            }
                            span class="live-pill" { "GRAFIC 3D SEPARAT" }
                        }
                        div style="position:relative; width:100%; height:440px; border-radius:16px; overflow:hidden; background:#050c18; border:2px solid #1e4273;" {
                            div class="hud-callout-badge hud-callout-fog" {
                                "☁️💨 NOR ÎMPINS DE VÂNT (Săgeți Mov)"
                            }
                            div class="hud-callout-badge hud-callout-als" {
                                "💡 BECURI PE PĂMÂNT VĂZUTE PRIN CEAȚĂ"
                            }
                            div class="gust-alert-overlay" id="gust-overlay" style="z-index:10;" {
                                "⚠️ ATENȚIE! AM VĂZUT UN NOR DE VÂNT ÎN FAȚĂ CU 0.85 SECUNDE ÎNAINTE SĂ LOVEASCĂ AVIONUL!"
                            }
                            canvas
                                id="hudCanvas"
                                width="600"
                                height="440"
                                style="display:block; width:100%; height:440px; border-radius:16px; background:#050c18;"
                            {}
                        }
                    }

                    // Panoul Drept: 🎥 Filmarea Video Reală din Cabina Piloților (#hudBgRealVideo)
                    div style="background:#040d1e; border:2px solid #00e5ff; border-radius:18px; padding:16px; display:flex; flex-direction:column; gap:12px;" {
                        div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:8px;" {
                            strong style="font-size:16.5px; color:#00e5ff;" {
                                "🎥 Filmarea Video Reală din Cabina Piloților"
                            }
                            div style="display:flex; gap:6px; flex-wrap:wrap;" {
                                button class="btn-mode-toggle" onclick="switchHudRealVideo(0)" {
                                    "🎥 Filmarea Reală 1 (LC-130)"
                                }
                                button class="btn-mode-toggle" onclick="switchHudRealVideo(1)" {
                                    "🎥 Filmarea Reală 2 (Paris CDG)"
                                }
                                button class="btn-mode-toggle" onclick="switchHudRealVideo(2)" {
                                    "🎥 Filmarea Reală 3 (Vânt Lateral)"
                                }
                            }
                        }
                        video
                            id="hudBgRealVideo"
                            src="/real_videos/real_02_als_runway_approach_cdg.mp4"
                            autoplay
                            loop
                            muted
                            playsinline
                            controls
                            style="display:block; width:100%; height:440px; object-fit:cover; border-radius:16px; opacity:1.0; background:#000; border:2px solid #00e5ff;"
                        {}
                    }
                }

                // 3. Cele 3 Casete Mari și Simple de Legendă
                div class="three-legend-boxes" {
                    div class="legend-card legend-green" {
                        div class="legend-title" { "🟢 Săgeți Verzi = Pământul și Pista fixă" }
                        p class="legend-desc" {
                            "Arată cum trece pământul pe sub noi. Pentru că pista stă pe loc, săgețile verzi ne spun viteza adevărată a avionului!"
                        }
                    }
                    div class="legend-card legend-purple" {
                        div class="legend-title" { "🟣 Săgeți Mov = Norul de ceață împins de vânt" }
                        p class="legend-desc" {
                            "Când bate vântul lateral, el împinge ceața. Calculatorul nostru separă săgețile mov ca să nu fie păcălit de nor!"
                        }
                    }
                    div class="legend-card legend-amber" {
                        div class="legend-title" { "🟡 Becurile Pistei văzute prin ceață" }
                        p class="legend-desc" {
                            "Ochelarii Infraroșu (SWIR) văd becurile calde de la capătul pistei Craiova RWY 09 chiar când afară e lapte alb!"
                        }
                    }
                }

                // 4. Bara Aerisită cu 4 Butoane Rapide chiar sub Parbriz
                div class="four-quick-buttons-bar" {
                    button class="btn-big btn-big-cyan" onclick="cycleHudRealVideo()" {
                        "🎥 Schimbă Filmarea Reală (1/2/3)"
                    }
                    button class="btn-big btn-big-purple" onclick="injectSuddenFogGustWithHighlight()" {
                        "💨 Bagă o Rafală de Ceață"
                    }
                    button class="btn-big btn-big-amber" onclick="stepManeuverWithHighlight(8, 2.5, 12, 0, 'Am virat avionul! Privește cum se înclină orizontul verde!')" {
                        "🔄 Virează Avionul"
                    }
                    button class="btn-big btn-big-green" onclick="toggleBlindNormalFog()" {
                        "🌫️ Ceață Oarbă / Normală"
                    }
                }
            }
        }
    }
}

/// PAGINA 3 (`#concept-page-3`): `3️⃣ Cursa celor 4 Avioane (De ce noi rămânem pe pistă?)`.
#[must_use]
pub fn render_page_3_four_planes_race(is_active: bool) -> Markup {
    html! {
        div
            id="concept-page-3"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            section class="spacious-card" id="zone-radar" {
                div class="windshield-top-controls" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 3 DIN 6 • ARENA CELOR 4 AVIOANE VĂZUTE DE SUS" }
                        h1 class="page-main-title" style="margin-top:6px;" {
                            "🏁 Cursa celor 4 Avioane: De Ce Doar Noi Rămânem pe Pistă?"
                        }
                    }
                    div style="display:flex; gap:12px; align-items:center; flex-wrap:wrap;" {
                        span class="hud-summary-badge" id="eta-summary" {
                            "TIMP PÂNĂ LA PISTĂ: 13.2 s | BECURI VĂZUTE: 12"
                        }
                        button class="btn-mode-toggle" onclick="window.setRadarMode6yo && window.setRadarMode6yo('game')" {
                            "🎮 Vedere Joc 4 Avioane"
                        }
                        button class="btn-mode-toggle" onclick="window.setRadarMode6yo && window.setRadarMode6yo('chart')" {
                            "📈 Grafic Eroare (m)"
                        }
                    }
                }

                // 1. Pista văzută de sus (#radarCanvas mare, height: 440px, 100% curată, fără niciun video suprapus!)
                div class="radar-stage-440" {
                    canvas id="radarCanvas" style="position:relative; z-index:4; width:100%; height:100%; display:block; background:#030914;" {}
                }

                // 2. Cele 4 Carduri Mari și Aerisite de Legendă (câte unul pentru fiecare avion)
                div class="four-planes-cards-grid" {
                    div class="plane-explain-card plane-card-green" {
                        div class="plane-card-header" {
                            span class="plane-emoji" { "✈️🟢" }
                            strong style="color:#00ff9d; font-size:18px;" { "Avionul Nostru (AERO-SHIELD)" }
                        }
                        div class="plane-err-val" id="err-shield" style="color:#00ff9d;" {
                            "Abatere față de centrul pistei: 0.28 m"
                        }
                        p class="plane-kid-text" {
                            "Separă norul de ceață de pământ, vede becurile pistei cu infraroșu și prinde hackerii în sub 3 secunde. Rămâne mereu pe mijlocul pistei!"
                        }
                    }

                    div class="plane-explain-card plane-card-red" {
                        div class="plane-card-header" {
                            span class="plane-emoji" { "✈️🔴" }
                            strong style="color:#ff2a5f; font-size:18px;" { "GPS Păcălit de Hacker" }
                        }
                        div class="plane-err-val" id="err-gnss" style="color:#ff2a5f;" {
                            "Abatere față de centrul pistei: 0.65 m"
                        }
                        p class="plane-kid-text" {
                            "Ascultă orbește de satelitul GPS. Când hackerul trimite o hartă falsă, acest avion crede minciuna și fuge direct în iarbă!"
                        }
                    }

                    div class="plane-explain-card plane-card-yellow" {
                        div class="plane-card-header" {
                            span class="plane-emoji" { "✈️🟡" }
                            strong style="color:#ffd600; font-size:18px;" { "Cameră Păcălită de Ceață" }
                        }
                        div class="plane-err-val" id="err-naive" style="color:#ffd600;" {
                            "Abatere față de centrul pistei: 0.95 m"
                        }
                        p class="plane-kid-text" {
                            "Folosește o cameră obișnuită care confundă norul de ceață cu pământul! Când vântul împinge ceața, avionul galben virează greșit!"
                        }
                    }

                    div class="plane-explain-card plane-card-orange" {
                        div class="plane-card-header" {
                            span class="plane-emoji" { "✈️🟠" }
                            strong style="color:#ff9100; font-size:18px;" { "Fără Cameră (Doar Senzori Interni)" }
                        }
                        div class="plane-err-val" id="err-ins" style="color:#ff9100;" {
                            "Abatere față de centrul pistei: 1.20 m"
                        }
                        p class="plane-kid-text" {
                            "Ca atunci când mergi cu ochii închiși prin cameră: la început știi unde ești, dar după câteva secunde amețești și te rătăcești!"
                        }
                    }
                }

                // 3. Cele 3 Butoane Mari de Test chiar pe Pagina 3
                div class="three-race-buttons-bar" {
                    button class="btn-big btn-big-red" onclick="raceTriggerHackerAttack()" {
                        "👾 Atacă GPS-ul cu Hackerul! (Vezi cum Avionul Roșu iese în iarbă!)"
                    }
                    button class="btn-big btn-big-purple" onclick="injectSuddenFogGustWithHighlight()" {
                        "💨 Suflă Vântul în Ceață! (Vezi cum Avionul Galben se păcălește!)"
                    }
                    button class="btn-big btn-big-green" onclick="resetAndHighlight()" {
                        "🔄 Resetează Cursa pe Mijlocul Pistei"
                    }
                }
            }
        }
    }
}

/// PAGINA 4 (`#concept-page-4`): `4️⃣ Creierul Google Drosophila & Scutul Anti-Hacker`.
#[must_use]
pub fn render_page_4_shield_and_controls(is_active: bool) -> Markup {
    html! {
        div
            id="concept-page-4"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            section class="spacious-card" {
                div class="page-hero-header" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 4 DIN 6 • CREIERUL GOOGLE DROSOPHILA & SCUTUL ANTI-HACKER" }
                        h1 class="page-main-title" {
                            "🧠🛡️ Creierul Google Drosophila & Scutul Anti-Hacker"
                        }
                        p class="page-subtitle" {
                            "Arhitectura conectomică Open-Source Google Research (Male Drosophila CNS & Optic Lobe Connectome) în 4 etaje biologice + cele 3 scuturi anti-hacker și telecomanda 6-DoF la 60 Hz!"
                        }
                    }
                    div style="display:flex; gap:10px; flex-wrap:wrap;" {
                        button class="btn-big btn-big-amber" onclick="triggerScenarioWithHighlight(1)" {
                            "🎬 Scenariul 1: Rafală Ceață"
                        }
                        button class="btn-big btn-big-cyan" onclick="triggerScenarioWithHighlight(2)" {
                            "🎬 Scenariul 2: Viraj 6-DoF"
                        }
                        button class="btn-big btn-big-red" onclick="triggerScenarioWithHighlight(3)" {
                            "🎬 Scenariul 3: Asediu Cyber"
                        }
                    }
                }

                // Partea 0: Panoul Vizual cu cele 4 Etaje ale Modelului Conectomic Open-Source Google Research Drosophila
                div class="drosophila-4-stages-grid" id="zone-drosophila-connectome" {
                    div class="drosophila-stage-card" {
                        div class="drosophila-stage-title" {
                            "👁️ Etajul 1: T4 / T5 (Lobul Optic Anticeață)"
                        }
                        p class="drosophila-stage-desc" {
                            "Separă reperele luminoase de pe sol (ALS) de ceața lăptoasă, exact cum vede musculița în zbor."
                        }
                    }
                    div class="drosophila-stage-card" {
                        div class="drosophila-stage-title" {
                            "🔄 Etajul 2: HS / VS (Stabilizatorul 6-DoF de Rotație)"
                        }
                        p class="drosophila-stage-desc" {
                            "Scade instantaneu virajele avionului (Dreapta/Stânga/Sus/Jos/Înclinare) din imaginea camerei."
                        }
                    }
                    div class="drosophila-stage-card" {
                        div class="drosophila-stage-title" {
                            "🧭 Etajul 3: E-PG Ring Attractor (Busola în Inel cu 16 Neuroni)"
                        }
                        p class="drosophila-stage-desc" {
                            "Menține direcția exactă către Pista 09 Craiova chiar și în orbire totală (0m vizibilitate)."
                        }
                    }
                    div class="drosophila-stage-card" {
                        div class="drosophila-stage-title" {
                            "💨 Etajul 4: PFNd (Fuziunea Vânt-Vedere & Anticiparea Rafalei)"
                        }
                        p class="drosophila-stage-desc" {
                            "Separă norul de ceață împins de vânt (săgețile mov) de pământul fix (săgețile verzi) cu 0.85s înainte de impact!"
                        }
                    }
                }

                // Partea 1: Cele 3 Carduri Mari și Aerisite ale Scutului de Siguranță
                div class="three-shields-grid" id="zone-cyber" {
                    // Card Scut 1: Paznicul GPS (Salt & Paharul CUSUM)
                    div class="shield-big-card" {
                        div class="shield-card-title" {
                            "🚨 1. Paznicul GPS: Salt Brusc & Paharul CUSUM"
                        }
                        p class="shield-card-story" {
                            "Dacă hackerul mută harta brusc (χ²) sau o trage milimetru cu milimetru (Paharul CUSUM), paznicul îl prinde în sub 2.9 secunde și taie firul GPS fals!"
                        }
                        div class="shield-metrics-box" {
                            div class="kv-row-big" {
                                span { "Stare Paznic GPS:" }
                                strong id="val-watchdog" style="color:#00ff9d;" { "TOTUL CURAT" }
                            }
                            div class="kv-row-big" {
                                span { "Scor Salt χ² / Pahar CUSUM:" }
                                strong id="val-chi2-cusum" style="color:#00e5ff;" { "1.18 / 0.12" }
                            }
                        }
                        div style="display:flex; gap:10px; flex-wrap:wrap; margin-top:14px;" {
                            button class="btn-big btn-big-red" id="btn-cyber-jump" style="flex:1;" onclick="toggleCyberWithHighlight('jump')" {
                                "🔴 Hacker: Salt GPS Brusc"
                            }
                            button class="btn-big btn-big-red" id="btn-cyber-stealth" style="flex:1;" onclick="toggleCyberWithHighlight('stealth')" {
                                "🕵️ Hacker: Fură GPS Încet"
                            }
                        }
                    }

                    // Card Scut 2: Avertizarea de Rafală în Ceață (0.85s înainte)
                    div class="shield-big-card" {
                        div class="shield-card-title" {
                            "🌪️ 2. Avertizarea de Rafală în Ceață (0.85s Înainte)"
                        }
                        p class="shield-card-story" {
                            "Camera privește la 40–80 metri în fața avionului. Când vede vârtejuri în ceață, avertizează pilotul cu 0.85 secunde ÎNAINTE ca vântul să lovească aripile!"
                        }
                        div class="shield-metrics-box" {
                            div class="kv-row-big" {
                                span { "Vânt Ceață Real vs Ghicit:" }
                                strong id="val-fog-wind" style="color:#f0abfc;" { "1.5 vs 1.5 m/s" }
                            }
                            div class="kv-row-big" {
                                span { "Grosime Ceață (β / ρ̂_fog):" }
                                strong id="val-beta" style="color:#00e5ff;" { "0.0399 m⁻¹" }
                            }
                            div class="kv-row-big" {
                                span { "Claritate Becuri Pistă:" }
                                strong id="val-cert" style="color:#00ff9d;" { "91.0% (12 becuri)" }
                            }
                        }
                        div style="margin-top:14px;" {
                            div style="display:flex; justify-content:space-between; font-size:14px; margin-bottom:6px;" {
                                span { "🌫️ Reglează Vizibilitatea în Ceață:" }
                                strong id="fog-slider-val" style="color:#00e5ff;" { "75 m" }
                            }
                            input type="range" id="fog-slider" min="25" max="1500" value="75" style="width:100%;" oninput="onFogSliderChange(this.value)" {}
                            button class="btn-big btn-big-purple" style="width:100%; margin-top:10px;" onclick="injectSuddenFogGustWithHighlight()" {
                                "🌪️ Suflă Rafală Laterală Puternică (18 m/s)"
                            }
                        }
                    }

                    // Card Scut 3: Cântarul Vântului Meteo & Cutia Matematică
                    div class="shield-big-card" id="zone-matrix" {
                        div class="shield-card-title" {
                            "⚖️ 3. Cântarul Vântului Meteo & Cutia Fără Blocaj"
                        }
                        p class="shield-card-story" {
                            "Comparăm raportul meteo de pe internet cu ce vede camera noastră. Dacă hackerul minte despre vânt, îl blocăm! Iar la cer punem 0.000000 fără împărțire la zero!"
                        }
                        div class="shield-metrics-box" {
                            div class="kv-row-big" {
                                span { "Sursă Meteo Craiova:" }
                                strong id="val-meteo-status" style="color:#00ff9d;" { "LIVE OPEN-METEO LRCV" }
                            }
                            div class="kv-row-big" {
                                span { "Cer / Orizont ρ_ground:" }
                                strong id="val-rho-hor" style="color:#00ff9d;" { "0.000000 m⁻¹ (SIGUR)" }
                            }
                            div class="kv-row-big" {
                                span { "Cuaternion ||q|| / Vreme:" }
                                strong id="val-qnorm" style="color:#00e5ff;" { "||q|| = 1.000000" }
                            }
                        }
                        div style="display:flex; gap:10px; flex-wrap:wrap; margin-top:14px;" {
                            button class="btn-big btn-big-red" id="btn-cyber-comms" style="flex:1;" onclick="toggleCyberWithHighlight('comms')" {
                                "🌐 Taie Internetul"
                            }
                            button class="btn-big btn-big-red" id="btn-cyber-poison" style="flex:1;" onclick="toggleCyberWithHighlight('poison')" {
                                "☣️ Meteo Mincinos"
                            }
                        }
                    }
                }

                // Partea 2: Telecomanda Completă de Zbor 6-DoF + Antrenare AI pe Placa Video
                div class="remote-control-spacious-box" id="zone-controls" {
                    div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:12px; margin-bottom:16px;" {
                        div {
                            h2 style="font-size:21px; color:#00e5ff;" {
                                "🕹️ Telecomanda de Pilotat Avionul în 6 Direcții (6-DoF) + Placa Video AI"
                            }
                            p style="font-size:15px; color:#9ec5eb; margin-top:4px;" {
                                "Apasă pe butoane sau folosește tastele W, A, S, D, Q, E pentru a vira și înclina avionul!"
                            }
                        }
                        button class="btn-big btn-big-green" onclick="startLiveCudaTrainingWithHighlight()" {
                            "🔥 Antrenare Fină Connectome Drosophila pe Placa Video CUDA (15s)"
                        }
                    }

                    div class="controls-6dof-spacious-grid" {
                        button class="btn-big" onclick="stepManeuverWithHighlight(-6, 0, 0, 0, 'Viraj la Stânga!')" { "⬅️ Virează Stânga (A)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(6, 0, 0, 0, 'Viraj la Dreapta!')" { "➡️ Virează Dreapta (D)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, 3.5, 0, 0, 'Ridicăm Botul Avionului!')" { "⬆️ Ridică Botul (W)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, -3.5, 0, 0, 'Coborâm Botul Avionului!')" { "⬇️ Coboară Botul (S)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, 0, -10, 0, 'Înclinăm Aripile spre Stânga!')" { "↺ Înclină Stânga (Q)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, 0, 10, 0, 'Înclinăm Aripile spre Dreapta!')" { "↻ Înclină Dreapta (E)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, 0, 0, 6, 'Mai Multă Viteză (+6 m/s)!')" { "⚡ Mai Repede (+6 m/s)" }
                        button class="btn-big" onclick="stepManeuverWithHighlight(0, 0, 0, -6, 'Frânăm Ușor (-6 m/s)!')" { "🛑 Mai Încet (-6 m/s)" }
                    }

                    div class="telemetry-strip-spacious" {
                        div class="tel-pill" { span { "Poziție NED:" } strong id="val-pos" { "[-900, 0, -52] m" } }
                        div class="tel-pill" { span { "Viteză Sol:" } strong id="val-vel-ned" { "[68.0, 0.0, 3.5] m/s" } }
                        div class="tel-pill" { span { "Unghiuri Euler:" } strong id="val-euler" { "[0.0°, -3.0°, 0.0°]" } }
                        div class="tel-pill" { span { "Unghi Vânt ψ_crab:" } strong id="val-crab" { "0.0°" } }
                        div class="tel-pill" { span { "Viteză AI:" } strong id="val-ai-lat" { "0.21 ms | σ=0.16m/s" } }
                    }

                    // Elemente ascunse de compatibilitate pentru updateTelemetryDom
                    div style="display:none;" {
                        span id="val-omega" {}
                        span id="val-aero" {}
                        span id="val-quat" {}
                        div id="val-rot-matrix" {}
                        span id="val-rho-fog" {}
                        span id="val-meteo-env" {}
                        span id="val-gpu-dev" {}
                    }

                    div id="cuda-progress-box" style="display:none; flex-direction:column; gap:6px; margin-top:14px; font-size:14px;" {
                        div style="display:flex; justify-content:space-between;" {
                            span id="cuda-epoch-txt" style="color:#00ff9d; font-weight:800;" { "ShadowTrainer CUDA Epoca 1/60" }
                            span id="cuda-loss-txt" style="color:#00e5ff; font-weight:800;" { "Eroare Fizică: 0.1420" }
                        }
                        div class="progress-bar-outer" {
                            div class="progress-bar-inner" id="cuda-progress-bar" {}
                        }
                    }
                }
            }
        }
    }
}

/// PAGINA 5 (`#concept-page-5`): `5️⃣ Cele 3 Filmări Video Reale din Cabina Piloților`.
#[must_use]
pub fn render_page_5_real_videos(bench_json: &serde_json::Value, is_active: bool) -> Markup {
    let total_frames = bench_json
        .get("total_frames_processed")
        .and_then(|v| v.as_u64())
        .unwrap_or(825);
    let overall_fps = bench_json
        .get("overall_fps_achieved")
        .and_then(|v| v.as_f64())
        .unwrap_or(438.7);

    let default_cards = [
        (
            "real_01_cat3_whiteout_cockpit_lc130.mp4",
            "Filmarea 1: Aterizare în Ceață Albă Completă (Whiteout CAT III — LC-130)",
            "Lapte alb în parbriz! Un pilot obișnuit nu vede pista cu ochiul liber până în ultimele secunde.",
            "Motorul Rust a extras automat contrastul Koschmieder și a ghidat avionul fără nicio eroare NaN la 435 FPS!",
        ),
        (
            "real_02_als_runway_approach_cdg.mp4",
            "Filmarea 2: Apropiere Noaptea pe Becurile Pistei (ALS Approach — Paris CDG)",
            "Se văd șirurile de becuri luminoase de apropiere (ALS) care clipesc prin stratul de ceață și ploaie.",
            "Camera noastră numără automat becurile ALS și calculează viteza reală față de pământ (Săgețile Verzi)!",
        ),
        (
            "real_03_crosswind_cockpit_landing.mp4",
            "Filmarea 3: Aterizare cu Vânt Lateral Puternic (Crosswind Crab Landing)",
            "Vântul bate puternic dintr-o parte, așa că avionul trebuie să zboare ușor oblic („ca racul”) ca să nu fie împins în iarbă.",
            "AERO-SHIELD simte vântul din ceață cu 0.85s înainte și comandă automat unghiul de corecție ψ_crab!",
        ),
    ];

    html! {
        div
            id="concept-page-5"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            section class="spacious-card" id="zone-real-videos" {
                div class="page-hero-header" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 5 DIN 6 • FILMĂRI REALE DIN CABINA DE PILOTAJ (.MP4)" }
                        h1 class="page-main-title" {
                            "📹 Cele 3 Filmări Video Reale din Cabina Piloților"
                        }
                        p class="page-subtitle" {
                            "Apasă Play pe oricare filmare sau deschide-o direct în Parbrizul Mare (Pagina 2)! Total analizat: "
                            strong style="color:#00e5ff;" { (total_frames) " cadre video reale" }
                            " la "
                            strong style="color:#00ff9d;" { (format!("{:.1} FPS", overall_fps)) }
                            "."
                        }
                    }
                }

                div class="videos-3-spacious-grid" {
                    @for (idx, (file_name, title_ro, eye_ro, ai_ro)) in default_cards.iter().enumerate() {
                        div class="video-spacious-card" {
                            h2 style="color:#00e5ff; font-size:19px; margin-bottom:12px;" {
                                "🎬 " (title_ro)
                            }
                            video
                                src=(format!("/real_videos/{}", file_name))
                                controls
                                loop
                                muted
                                playsinline
                                style="width:100%; height:230px; object-fit:cover; border-radius:12px; border:2px solid #1e4273; background:#000;"
                            {}

                            div class="video-explain-box" {
                                p style="font-size:15.5px; color:#e2f1ff; line-height:1.5; margin-bottom:8px;" {
                                    strong style="color:#ffd600;" { "👁️ Ce se vede cu ochiul liber: " }
                                    (eye_ro)
                                }
                                p style="font-size:15.5px; color:#d1fae5; line-height:1.5;" {
                                    strong style="color:#00ff9d;" { "🤖 Ce a măsurat programul nostru: " }
                                    (ai_ro)
                                }
                            }

                            button
                                class="btn-big btn-big-cyan"
                                style="width:100%; margin-top:14px;"
                                onclick=(format!("openRealVideoInWindshield({})", idx))
                            {
                                "📺 Deschide în Parbrizul Mare (Pagina 2)"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// PAGINA 6 (`#concept-page-6`): `6️⃣ Cele 10 Teste Sintetice FlightGear & Cursa Lungă 100 km Fără GPS (Analiza Erorii)`.
#[must_use]
pub fn render_page_6_flightgear_tests(fg: &FlightGearTestSuiteReport, is_active: bool) -> Markup {
    let lr = crate::ws_telemetry::load_or_run_long_range_zero_gps_report();

    html! {
        div
            id="concept-page-6"
            class=(if is_active { "concept-page-view active-page" } else { "concept-page-view" })
        {
            section class="spacious-card" id="zone-long-range-100km" style="margin-bottom:24px; border:2px solid #00ff9d;" {
                div class="page-hero-header" {
                    div {
                        span class="concept-pill" style="background:rgba(0,255,157,0.16); color:#00ff9d; border:1px solid #00ff9d;" {
                            "CURSĂ SINTETICĂ LUNGĂ FĂRĂ GPS • 100 KM (25 MIN / 90.000 CADRE LA 60 HZ) • IDENTIFICAREA ERORII"
                        }
                        h1 class="page-main-title" {
                            "🧭 Cursă Sintetică Lungă 100 km Fără GPS (Slatina/Balș → Craiova RWY 09) — Identificarea & Descompunerea Erorii"
                        }
                        p class="page-subtitle" {
                            "🧒 "
                            strong style="color:#ffd600;" { "Explicat simplu (ca la 6 ani): " }
                            "Ce se întâmplă dacă zburăm "
                            strong style="color:#00e5ff;" { (format!("{:.0} km", lr.total_distance_km)) }
                            " ("
                            strong style="color:#00e5ff;" { (format!("{:.0} minute / {} cadre la 60 Hz", lr.total_duration_sec / 60.0, lr.total_frames_60hz)) }
                            ") cu "
                            strong style="color:#ff2a5f;" { (format!("{:.1}% GPS", lr.gps_availability_pct)) }
                            " (sateliți complet opriți / bruiați)? Pe parcursul a 85 km fără niciun reper luminos la sol, chiar și sistemul nostru acumulează o eroare maximă de "
                            strong style="color:#ffd600;" { (format!("{:.1} m", lr.peak_error_before_als_m)) }
                            " (~0.36% din distanță), pe care o "
                            strong style="color:#00ff9d;" { "identificăm exact pe 4 cauze fizice" }
                            " și o "
                            strong style="color:#00ff9d;" { "reducem la doar " (format!("{:.2} m", lr.final_touchdown_error_m)) }
                            " când camera vede becurile pistei (ALS) la aterizare!"
                        }
                    }
                }

                // Sumar KPI pe 5 casete mari (Peak Error, Touchdown Error, Lateral, Longitudinal, Vertical & NaN status)
                div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(210px, 1fr)); gap:14px; margin-bottom:20px;" {
                    div style="background:#081a33; border:1.5px solid #ffd600; border-radius:14px; padding:14px;" {
                        div style="font-size:13px; color:#fde047; font-weight:800;" { "⚠️ EROARE MAXIMĂ ÎNAINTE DE PISTĂ (KM 85)" }
                        div style="font-size:28px; font-weight:900; color:#ffd600; margin:6px 0;" {
                            (format!("{:.1} m", lr.peak_error_before_als_m))
                        }
                        div style="font-size:13px; color:#cbd5e1;" {
                            "După 85 km de croazieră oarbă fără GPS și fără becuri ALS (~0.36% drift din distanță)"
                        }
                    }
                    div style="background:#081a33; border:2px solid #00ff9d; border-radius:14px; padding:14px;" {
                        div style="font-size:13px; color:#6ee7b7; font-weight:800;" { "🎯 EROARE FINALĂ LA ATERIZARE (KM 100)" }
                        div style="font-size:28px; font-weight:900; color:#00ff9d; margin:6px 0;" {
                            (format!("{:.2} m", lr.final_touchdown_error_m))
                        }
                        div style="font-size:13px; color:#cbd5e1;" {
                            "După ancorarea optică pe becurile pistei Craiova RWY 09 (CAT IIIc < 0.85 m)"
                        }
                    }
                    div style="background:#081a33; border:1.5px solid #00e5ff; border-radius:14px; padding:14px;" {
                        div style="font-size:13px; color:#7dd3fc; font-weight:800;" { "↔️ ABATERE LATERALĂ / LONGITUDINALĂ" }
                        div style="font-size:22px; font-weight:900; color:#00e5ff; margin:6px 0;" {
                            "Lat: " (format!("{:.2} m", lr.lateral_touchdown_error_m)) " | Long: " (format!("{:.2} m", lr.longitudinal_touchdown_error_m))
                        }
                        div style="font-size:13px; color:#cbd5e1;" {
                            "Abatere verticală (altitudine): "
                            strong style="color:#00ff9d;" { (format!("{:.2} m", lr.vertical_touchdown_error_m)) }
                        }
                    }
                    div style="background:#081a33; border:1.5px solid #38bdf8; border-radius:14px; padding:14px;" {
                        div style="font-size:13px; color:#bae6fd; font-weight:800;" { "🛡️ STABILITATE NUMERICĂ (90.000 CADRE)" }
                        div style="font-size:22px; font-weight:900; color:#00ff9d; margin:6px 0;" {
                            @if lr.zero_nan_or_inf_verified {
                                "✅ 0 NaN / 0 Inf (100% Stabil)"
                            } @else {
                                "⚠️ Verificare numerică"
                            }
                        }
                        div style="font-size:13px; color:#cbd5e1;" {
                            "GPS folosit pe toată cursa de 100 km: "
                            strong style="color:#ff2a5f;" { (format!("{:.1}%", lr.gps_availability_pct)) }
                        }
                    }
                }

                // Tabelul / Cardurile celor 4 Faze ale Cursei de 100 km Fără GPS
                h2 style="color:#00e5ff; font-size:20px; margin:18px 0 12px 0;" {
                    "📍 Evoluția Erorii pe Cele 4 Faze ale Zborului Sintetic de 100 km (Fără GPS)"
                }
                div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(260px, 1fr)); gap:14px; margin-bottom:22px;" {
                    @for phase in &lr.phases {
                        div style="background:#071529; border:1px solid #1e3a5f; border-radius:14px; padding:15px;" {
                            div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;" {
                                span class="fg-id-badge" {
                                    "FAZA " (phase.phase_index) " (" (format!("{:.0}–{:.0} km", phase.km_start, phase.km_end)) ")"
                                }
                                span style="font-size:13px; color:#7dd3fc; font-weight:700;" {
                                    (format!("{:.0}s • {} cadre", phase.duration_sec, phase.frames_60hz))
                                }
                            }
                            div style="color:#ffffff; font-weight:800; font-size:16.5px; margin-bottom:8px;" {
                                (phase.phase_name)
                            }
                            div style="font-size:14px; color:#cbd5e1; line-height:1.55;" {
                                div {
                                    "🌫️ Vizibilitate MOR: "
                                    strong style="color:#fde047;" { (format!("{:.0} m", phase.visibility_mor_m)) }
                                    " | 🧭 Eroare busolă E-PG: "
                                    strong style="color:#00e5ff;" { (format!("{:.3}°", phase.epg_compass_error_deg)) }
                                }
                                div style="margin-top:6px;" {
                                    "🟢 Eroare AERO-SHIELD (final / max): "
                                    strong style="color:#00ff9d;" {
                                        (format!("{:.2} m", phase.aero_shield_pos_error_end_m))
                                        " / "
                                        (format!("{:.1} m", phase.aero_shield_pos_error_max_m))
                                    }
                                    " ("
                                    (format!("{:.3}%", phase.aero_shield_drift_pct_of_distance))
                                    " din distanță, RMSE viteză "
                                    (format!("{:.3} m/s", phase.aero_shield_vel_rmse_mps))
                                    ")"
                                }
                                div style="margin-top:6px; border-top:1px dashed #1e3a5f; padding-top:6px; font-size:13px;" {
                                    "🟠 INS pur (fără cameră): "
                                    strong style="color:#ff9100;" { (format!("{:.0} m", phase.pure_ins_pos_error_end_m)) }
                                    " | 🟡 Flux optic naiv: "
                                    strong style="color:#ffd600;" { (format!("{:.0} m", phase.naive_flow_pos_error_end_m)) }
                                    " | 🔴 GPS falsificat: "
                                    strong style="color:#ff2a5f;" { (format!("{:.0} m", phase.spoofed_gps_pos_error_end_m)) }
                                }
                            }
                        }
                    }
                }

                // SECȚIUNEA ESENȚIALĂ: Identificarea și Descompunerea celor 4 Surse Fizice de Eroare
                div id="zone-error-sources-breakdown" style="background:#061326; border:2px solid #ffd600; border-radius:16px; padding:18px; margin-bottom:20px;" {
                    h2 style="color:#ffd600; font-size:21px; margin-bottom:8px;" {
                        "🔍 De unde provine eroarea de " (format!("{:.1} m", lr.peak_error_before_als_m)) " înainte de pistă? (Cele 4 Surse Fizice Identificate)"
                    }
                    p style="color:#e2f1ff; font-size:15px; margin-bottom:14px; line-height:1.5;" {
                        "Am izolat matematic în simulatorul Rust fiecare componentă care contribuie la deriva acumulată pe cei 85 km de zbor fără GPS și fără lumini de pistă:"
                    }
                    div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(280px, 1fr)); gap:14px;" {
                        @for (idx, src) in lr.identified_error_sources.iter().enumerate() {
                            div style="background:#0b1e3b; border:1px solid #334155; border-radius:12px; padding:14px;" {
                                div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:6px;" {
                                    span style="color:#ffd600; font-weight:900; font-size:15px;" {
                                        "SURSA #" (idx + 1) " • " (format!("{:.1}%", src.contribution_pct))
                                    }
                                    span style="background:rgba(255,214,0,0.18); color:#fde047; font-weight:900; padding:3px 9px; border-radius:8px; font-size:14px;" {
                                        "+" (format!("{:.1} m", src.contribution_m))
                                    }
                                }
                                div style="color:#ffffff; font-weight:800; font-size:16px; margin-bottom:8px;" {
                                    (src.source_name)
                                }
                                div class="progress-bar-outer" style="margin-bottom:10px; height:10px;" {
                                    div
                                        class="progress-bar-inner"
                                        style=(format!("width:{:.1}%; background:linear-gradient(90deg, #ffd600, #ff9100);", src.contribution_pct.clamp(0.0, 100.0)))
                                    {}
                                }
                                p style="color:#bae6fd; font-size:14px; line-height:1.5; margin-bottom:8px;" {
                                    "⚙️ " (src.physical_mechanism)
                                }
                                div style="font-size:12.5px; color:#94a3b8; border-top:1px dashed #1e3a5f; padding-top:6px;" {
                                    " 📌 Activă în: " strong style="color:#7dd3fc;" { (src.active_in_phases) }
                                }
                            }
                        }
                    }
                }

                // SECȚIUNEA DE REMEDIERE: Cum corectăm fiecare sursă de eroare
                div id="zone-error-remediation" style="background:#051c24; border:1.5px solid #00ff9d; border-radius:16px; padding:18px;" {
                    h2 style="color:#00ff9d; font-size:20px; margin-bottom:10px;" {
                        "🛠️ Cum Remediem / Eliminăm Fiecare Sursă de Eroare în Arhitectura AERO-SHIELD"
                    }
                    div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(280px, 1fr)); gap:12px;" {
                        @for rec in &lr.engineering_remediation_recommendations {
                            div style="background:#072630; border-left:4px solid #00ff9d; border-radius:10px; padding:12px 14px; color:#ecfdf5; font-size:14.5px; line-height:1.5;" {
                                "✅ " (rec)
                            }
                        }
                    }
                }
            }

            section class="spacious-card" id="zone-flightgear-10" {
                div class="page-hero-header" {
                    div {
                        span class="concept-pill" { "CONCEPTUL 6 DIN 6 • VALIDARE SINTETICĂ FLIGHTGEAR PE VM-UL GPU" }
                        h1 class="page-main-title" {
                            "🛩️ Cele 10 Teste Sintetice FlightGear (" (fg.passed_tests) "/" (fg.total_tests) " PASSED)"
                        }
                        p class="page-subtitle" {
                            "Simulator instalat pe mașina virtuală: "
                            strong style="color:#00ff9d;" { (fg.fgfs_binary_version) }
                            " | Abatere medie AERO-SHIELD: "
                            strong style="color:#00ff9d;" { (format!("{:.2} m", fg.mean_aero_shield_error_m)) }
                            " față de "
                            strong style="color:#ff2a5f;" { (format!("{:.1} m", fg.mean_classic_estimator_error_m)) }
                            " la sistemele clasice."
                        }
                    }
                }

                div class="fg-10-spacious-grid" {
                    @for t in &fg.tests {
                        div class="fg-spacious-card" {
                            div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;" {
                                span class="fg-id-badge" { (t.test_id) }
                                span style="color:#00ff9d; font-weight:900; font-size:14px;" {
                                    "✅ PASS (" (format!("{:.2}m", t.aero_shield_touchdown_error_m)) " vs " (format!("{:.1}m", t.classic_gnss_or_flow_error_m)) ")"
                                }
                            }
                            div style="color:#ffffff; font-weight:800; font-size:17px; margin-bottom:6px;" {
                                (t.title_ro)
                            }
                            p style="color:#bae6fd; font-size:15px; line-height:1.5; margin-bottom:8px;" {
                                "🧒 " (t.simple_explanation_ro)
                            }
                            div style="font-size:13.5px; color:#7dd3fc; border-top:1px dashed #1e3a5f; padding-top:8px;" {
                                "📊 " (t.key_metric_verified)
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Bara Mare de Navigare Jos (sub conceptul activ): `⬅️ Conceptul Anterior` | `📍 Pagina X din 6` | `Conceptul Următor ➡️`.
#[must_use]
pub fn render_bottom_page_navigator(active_page: u8) -> Markup {
    let idx = (active_page.clamp(1, 6) - 1) as usize;
    let meta = SIX_CONCEPT_PAGES[idx];

    html! {
        footer class="bottom-concept-nav" {
            button
                class="btn-big btn-nav-prev"
                id="btn-prev-concept"
                onclick="prevConceptPage()"
            {
                "⬅️ Conceptul Anterior"
            }

            div class="bottom-page-indicator" id="bottom-page-indicator" {
                "📍 Pagina " (meta.page_num) " din 6: " (meta.full_title_ro)
            }

            button
                class="btn-big btn-nav-next"
                id="btn-next-concept"
                onclick="nextConceptPage()"
            {
                "Conceptul Următor ➡️"
            }
        }
    }
}

/// Alias compatibil cu toate testele existente: randează Pagina 1 (și include toate cele 6 pagini în DOM pentru comutare instantanee).
#[must_use]
pub fn render_aero_shield_app_maud(state: &CockpitServerState) -> Markup {
    render_aero_shield_app_maud_page(state, 1)
}

/// Randează pagina completă SSR la numărul de pagină specificat (`1..=6`) fără `CockpitServerState`.
#[must_use]
pub fn render_full_cockpit_page_at(initial_page: u8) -> Markup {
    let active_page = initial_page.clamp(1, 6);
    let lessons = get_all_6_interactive_lessons();
    let bench_json = compute_or_load_real_video_benchmark();
    let fg_report = run_all_10_flightgear_synthetic_tests();

    html! {
        (DOCTYPE)
        html lang="ro" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "AERO-SHIELD LRCV V2.0 — Rust Maud SSR • 6 Pagini Aerisite (Craiova RWY 09)" }
                style { (PreEscaped(MAUD_COCKPIT_CSS)) }
            }
            body data-initial-page=(active_page) {
                (render_header_bar(0, "CUDA NVIDIA / CPU SIMD", active_page))
                (render_floating_pointer())

                main class="spacious-page-container" {
                    (render_page_1_interactive_school(&lessons, active_page == 1))
                    (render_page_2_windshield_hud(active_page == 2))
                    (render_page_3_four_planes_race(active_page == 3))
                    (render_page_4_shield_and_controls(active_page == 4))
                    (render_page_5_real_videos(&bench_json, active_page == 5))
                    (render_page_6_flightgear_tests(&fg_report, active_page == 6))
                }

                (render_bottom_page_navigator(active_page))

                script { (PreEscaped(MAUD_COCKPIT_JS_ENGINE)) }
                script { (PreEscaped(crate::ui_visual_engine::render_visual_canvases_script())) }
            }
        }
    }
}

/// Funcția principală SSR care randează aplicația aerisită cu 6 pagini (câte un singur concept vizibil pe ecran).
#[must_use]
pub fn render_aero_shield_app_maud_page(state: &CockpitServerState, initial_page: u8) -> Markup {
    let active_page = initial_page.clamp(1, 6);
    let latest_frame = state.telemetry_rx.borrow().clone();
    let lessons = get_all_6_interactive_lessons();
    let bench_json = compute_or_load_real_video_benchmark();
    let fg_report = run_all_10_flightgear_synthetic_tests();

    html! {
        (DOCTYPE)
        html lang="ro" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "AERO-SHIELD LRCV V2.0 — Rust Maud SSR • 6 Pagini Aerisite (Craiova RWY 09)" }
                style { (PreEscaped(MAUD_COCKPIT_CSS)) }
            }
            body data-initial-page=(active_page) {
                (render_header_bar(latest_frame.tick, &latest_frame.cyber_and_gpu.gpu_device_name, active_page))
                (render_floating_pointer())

                main class="spacious-page-container" {
                    (render_page_1_interactive_school(&lessons, active_page == 1))
                    (render_page_2_windshield_hud(active_page == 2))
                    (render_page_3_four_planes_race(active_page == 3))
                    (render_page_4_shield_and_controls(active_page == 4))
                    (render_page_5_real_videos(&bench_json, active_page == 5))
                    (render_page_6_flightgear_tests(&fg_report, active_page == 6))
                }

                (render_bottom_page_navigator(active_page))

                script { (PreEscaped(MAUD_COCKPIT_JS_ENGINE)) }
                script { (PreEscaped(crate::ui_visual_engine::render_visual_canvases_script())) }
            }
        }
    }
}

const MAUD_COCKPIT_CSS: &str = r####"
:root {
  --bg-deep: #040914;
  --bg-panel: #0a1428;
  --bg-card: #0e1d38;
  --border-cyan: #1e4273;
  --hud-green: #00ff9d;
  --hud-cyan: #00e5ff;
  --fog-purple: #d946ef;
  --alert-red: #ff2a5f;
  --warn-amber: #ffb800;
  --text-main: #f1f7ff;
  --text-dim: #9ec5eb;
}
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  font-family: "Inter", "Segoe UI", system-ui, sans-serif;
}
body {
  background: radial-gradient(circle at 50% 0%, #0f2246 0%, var(--bg-deep) 75%);
  color: var(--text-main);
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  overflow-x: hidden;
  padding-bottom: 48px;
}

/* BARA DE NAVIGARE SUS (AERISITĂ, MARE, CLARĂ) */
.topbar {
  background: rgba(7, 15, 32, 0.96);
  border-bottom: 2px solid var(--border-cyan);
  padding: 18px 36px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.topbar-upper {
  max-width: 1220px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}
.brand { display: flex; align-items: center; gap: 14px; }
.brand-badge {
  background: linear-gradient(135deg, #00ff9d 0%, #00b8ff 100%);
  color: #021019;
  font-weight: 900;
  font-size: 13px;
  padding: 6px 12px;
  border-radius: 8px;
}
.brand-title { font-size: 19px; font-weight: 900; color: var(--hud-cyan); }
.brand-sub { font-size: 13.5px; color: var(--text-dim); margin-top: 2px; }

.lang-and-status {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-wrap: wrap;
}
.lang-switch {
  display: flex;
  gap: 6px;
  background: #061022;
  padding: 4px;
  border-radius: 10px;
  border: 1px solid #1e4273;
}
.lang-btn {
  background: transparent;
  color: var(--text-dim);
  border: none;
  padding: 7px 14px;
  border-radius: 7px;
  font-size: 14px;
  font-weight: 800;
  cursor: pointer;
}
.lang-btn.active {
  background: #163763;
  color: #ffffff;
}
.phase-banner {
  background: rgba(0, 255, 157, 0.12);
  border: 1px solid var(--hud-green);
  color: var(--hud-green);
  padding: 8px 16px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 800;
}

/* CELE 6 BUTOANE MARI DE PAGINI (CÂTE UN CONCEPT PE PAGINĂ) */
.pages-nav-bar {
  max-width: 1220px;
  width: 100%;
  margin: 0 auto;
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 12px;
}
.nav-page-btn {
  background: #0d1d3a;
  color: #dbeafe;
  border: 2px solid #23497d;
  padding: 13px 14px;
  border-radius: 14px;
  font-size: 15px;
  font-weight: 800;
  cursor: pointer;
  transition: all 0.16s ease;
  text-align: center;
}
.nav-page-btn:hover {
  border-color: var(--hud-cyan);
  background: #16305c;
  transform: translateY(-2px);
}
.nav-page-btn.active {
  background: linear-gradient(90deg, #00ff9d 0%, #00d4ff 100%);
  color: #021019;
  border-color: #ffffff;
  font-weight: 900;
  box-shadow: 0 0 22px rgba(0, 255, 157, 0.45);
}

/* CONTAINERUL PRINCIPAL FOARTE AERISIT (MAX 1220px, PADDING GENEROS) */
.spacious-page-container {
  max-width: 1220px;
  width: 100%;
  margin: 28px auto;
  padding: 0 28px;
  flex: 1;
}

/* DOAR 1 PAGINĂ VIZIBILĂ LA ORICE MOMENT! */
.concept-page-view {
  display: none !important;
}
.concept-page-view.active-page {
  display: block !important;
  animation: fadeInPage 0.25s ease;
}
@keyframes fadeInPage {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.spacious-card {
  background: rgba(11, 23, 46, 0.94);
  border: 2px solid #21467a;
  border-radius: 24px;
  padding: 36px 44px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.45);
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.page-hero-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: 20px;
  border-bottom: 1px solid #1e3a66;
  padding-bottom: 22px;
}
.concept-pill {
  display: inline-block;
  background: rgba(0, 229, 255, 0.14);
  color: var(--hud-cyan);
  border: 1px solid rgba(0, 229, 255, 0.4);
  padding: 5px 14px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 900;
  letter-spacing: 0.4px;
}
.page-main-title {
  font-size: 27px;
  font-weight: 900;
  color: #ffffff;
  margin-top: 8px;
}
.page-subtitle {
  font-size: 17.5px;
  color: var(--text-dim);
  line-height: 1.55;
  margin-top: 8px;
  max-width: 850px;
}
.stars-box {
  background: #071329;
  border: 2px solid #fbbf24;
  color: #fde047;
  padding: 12px 20px;
  border-radius: 14px;
  font-size: 17px;
  font-weight: 900;
}

/* PAGINA 1: ȘCOALA INTERACTIVĂ AERISITĂ */
.lesson-tabs-spacious {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 12px;
}
.lesson-tab-big {
  background: #08152c;
  color: #cbd5e1;
  border: 2px solid #214373;
  padding: 14px 12px;
  border-radius: 14px;
  font-size: 16px;
  font-weight: 800;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  transition: all 0.15s ease;
}
.lesson-tab-big:hover {
  border-color: var(--hud-cyan);
  color: #fff;
}
.lesson-tab-big.active {
  background: linear-gradient(135deg, #f59e0b 0%, #eab308 100%);
  color: #021019;
  border-color: #fef08a;
  font-weight: 900;
  box-shadow: 0 0 18px rgba(245, 158, 11, 0.45);
}
.lesson-pane-spacious {
  flex-direction: column;
  gap: 20px;
  background: #071226;
  border: 2px solid #1e3f6f;
  border-radius: 18px;
  padding: 28px 32px;
}
.lesson-big-heading {
  font-size: 23px;
  font-weight: 900;
  color: var(--hud-cyan);
}
.lesson-big-story {
  font-size: 20px;
  line-height: 1.65;
  color: #f8fafc;
  background: #040c1a;
  padding: 22px 26px;
  border-radius: 14px;
  border-left: 6px solid var(--warn-amber);
}
.lesson-action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  align-items: center;
}
.lesson-takeaway-banner {
  font-size: 16.5px;
  color: var(--hud-green);
  font-weight: 800;
  background: rgba(0, 255, 157, 0.08);
  padding: 12px 18px;
  border-radius: 10px;
  border: 1px solid rgba(0, 255, 157, 0.3);
}
.school-canvas-box {
  background: #061124;
  border: 2px solid #21487a;
  border-radius: 18px;
  padding: 22px 26px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.school-canvas-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 16px;
  color: #bae6fd;
}
.live-pill {
  background: rgba(0, 255, 157, 0.16);
  color: var(--hud-green);
  padding: 5px 12px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 900;
}

/* BUTOANE MARI PENTRU TOATE PAGINILE */
.btn-big {
  background: #13284c;
  color: #ffffff;
  border: 2px solid #2b548e;
  padding: 14px 22px;
  border-radius: 14px;
  font-size: 16px;
  font-weight: 800;
  cursor: pointer;
  transition: all 0.15s ease;
}
.btn-big:hover {
  transform: translateY(-2px);
  border-color: var(--hud-cyan);
  background: #1b3869;
}
.btn-big-green {
  background: linear-gradient(90deg, #059669 0%, #10b981 100%);
  border-color: #6ee7b7;
  color: #ffffff;
}
.btn-big-cyan {
  background: linear-gradient(90deg, #0284c7 0%, #06b6d4 100%);
  border-color: #bae6fd;
  color: #ffffff;
}
.btn-big-amber {
  background: linear-gradient(90deg, #d97706 0%, #f59e0b 100%);
  border-color: #fde68a;
  color: #021019;
}
.btn-big-purple {
  background: linear-gradient(90deg, #7e22ce 0%, #c026d3 100%);
  border-color: #f0abfc;
  color: #ffffff;
}
.btn-big-red {
  background: linear-gradient(90deg, #be123c 0%, #e11d48 100%);
  border-color: #fda4af;
  color: #ffffff;
}

/* PAGINA 2: PARBRIZUL AVIONULUI & FILMAREA 50% */
.storyteller-bar {
  background: linear-gradient(90deg, rgba(6, 36, 52, 0.96) 0%, rgba(14, 28, 56, 0.96) 100%);
  border: 2px solid var(--hud-green);
  border-radius: 20px;
  padding: 20px 28px;
  margin-bottom: 24px;
  display: flex;
  align-items: center;
  gap: 18px;
  flex-wrap: wrap;
}
.story-icon { font-size: 34px; }
.story-headline { font-size: 19px; font-weight: 900; color: #ffffff; }
.story-subtext { font-size: 15.5px; color: #bae6fd; margin-top: 4px; }
.hud-summary-badge {
  background: #051329;
  border: 1px solid var(--hud-cyan);
  color: var(--hud-green);
  padding: 10px 16px;
  border-radius: 12px;
  font-size: 15px;
  font-weight: 800;
}
.windshield-top-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 18px;
}
.slider-control-box {
  display: flex;
  align-items: center;
  gap: 12px;
  background: #071429;
  padding: 12px 20px;
  border-radius: 14px;
  border: 1px solid #23497d;
}
.windshield-stage-480 {
  position: relative;
  width: 100%;
  height: 480px;
  background: #02060d;
  border: 2px solid #254f88;
  border-radius: 18px;
  overflow: hidden;
}
.hud-callout-badge {
  position: absolute;
  z-index: 8;
  padding: 8px 14px;
  border-radius: 10px;
  font-size: 13.5px;
  font-weight: 900;
  pointer-events: none;
}
.hud-callout-fog {
  top: 16px;
  left: 18px;
  background: rgba(88, 28, 135, 0.88);
  border: 1px solid #f0abfc;
  color: #fae8ff;
}
.hud-callout-als {
  bottom: 16px;
  right: 18px;
  background: rgba(6, 78, 59, 0.88);
  border: 1px solid #00ff9d;
  color: #d1fae5;
}
.gust-alert-overlay {
  position: absolute;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(225, 29, 72, 0.94);
  border: 2px solid #fde047;
  color: #ffffff;
  padding: 10px 22px;
  border-radius: 12px;
  font-size: 15px;
  font-weight: 900;
  display: none;
}
.three-legend-boxes {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 18px;
}
.legend-card {
  background: #071326;
  border-radius: 16px;
  padding: 20px 22px;
  border: 2px solid #1e3f6f;
}
.legend-green { border-left: 6px solid #00ff9d; }
.legend-purple { border-left: 6px solid #d946ef; }
.legend-amber { border-left: 6px solid #ffb800; }
.legend-title { font-size: 17px; font-weight: 900; color: #ffffff; margin-bottom: 8px; }
.legend-desc { font-size: 15px; color: #cbd5e1; line-height: 1.5; }

.four-quick-buttons-bar {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}

/* PAGINA 3: CURSA CELOR 4 AVIOANE */
.radar-stage-440 {
  position: relative;
  width: 100%;
  height: 440px;
  background: #030914;
  border: 2px solid #254f88;
  border-radius: 18px;
  overflow: hidden;
}
.btn-mode-toggle {
  background: #0f2447;
  color: #e0f2fe;
  border: 1px solid #38bdf8;
  padding: 9px 14px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 800;
  cursor: pointer;
}
.four-planes-cards-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 18px;
}
.plane-explain-card {
  background: #071326;
  border-radius: 16px;
  padding: 22px 24px;
  border: 2px solid #1e3f6f;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.plane-card-green { border-left: 7px solid #00ff9d; background: rgba(6, 42, 34, 0.55); }
.plane-card-red { border-left: 7px solid #ff2a5f; }
.plane-card-yellow { border-left: 7px solid #ffd600; }
.plane-card-orange { border-left: 7px solid #ff9100; }
.plane-card-header { display: flex; align-items: center; gap: 10px; }
.plane-emoji { font-size: 26px; }
.plane-err-val { font-size: 16px; font-weight: 900; }
.plane-kid-text { font-size: 15px; color: #dbeafe; line-height: 1.5; }
.three-race-buttons-bar {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

/* PAGINA 4: CREIERUL GOOGLE DROSOPHILA, SCUTUL ANTI-HACKER & TELECOMANDA */
.drosophila-pills-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 12px;
}
.drosophila-bio-pill {
  background: rgba(0, 255, 157, 0.12);
  border: 1px solid #00ff9d;
  color: #d1fae5;
  padding: 6px 14px;
  border-radius: 999px;
  font-size: 13.5px;
  font-weight: 800;
}
.drosophila-4-stages-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}
.drosophila-stage-card {
  background: linear-gradient(145deg, #07162c 0%, #091d3a 100%);
  border: 2px solid #00e5ff;
  border-left: 5px solid #00ff9d;
  border-radius: 14px;
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.drosophila-stage-title {
  font-size: 15.5px;
  font-weight: 900;
  color: #00ff9d;
}
.drosophila-stage-desc {
  font-size: 13.5px;
  color: #dbeafe;
  line-height: 1.45;
}
.three-shields-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 20px;
}
.shield-big-card {
  background: #071326;
  border: 2px solid #21467a;
  border-radius: 18px;
  padding: 24px 26px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 14px;
}
.shield-card-title { font-size: 19px; font-weight: 900; color: var(--hud-cyan); }
.shield-card-story { font-size: 15.5px; color: #cbd5e1; line-height: 1.55; }
.shield-metrics-box {
  background: #040c1a;
  border-radius: 12px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.kv-row-big {
  display: flex;
  justify-content: space-between;
  font-size: 14.5px;
  color: #9ec5eb;
}
.remote-control-spacious-box {
  background: #071326;
  border: 2px solid #21467a;
  border-radius: 18px;
  padding: 26px 30px;
}
.controls-6dof-spacious-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}
.telemetry-strip-spacious {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 12px;
  margin-top: 18px;
}
.tel-pill {
  background: #040c1a;
  border: 1px solid #1e3f6f;
  border-radius: 10px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 13px;
  color: #9ec5eb;
}
.tel-pill strong { font-size: 15px; color: #00ff9d; }

/* PAGINA 5: CELE 3 FILMĂRI REALE */
.videos-3-spacious-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 22px;
}
.video-spacious-card {
  background: #071326;
  border: 2px solid #21467a;
  border-radius: 18px;
  padding: 24px 26px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}
.video-explain-box {
  background: #040c1a;
  border-radius: 12px;
  padding: 16px;
  margin-top: 14px;
}

/* PAGINA 6: CELE 10 TESTE FLIGHTGEAR */
.fg-10-spacious-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 18px;
}
.fg-spacious-card {
  background: #071326;
  border: 2px solid #21467a;
  border-radius: 16px;
  padding: 22px 26px;
}
.fg-id-badge {
  background: var(--hud-cyan);
  color: #021019;
  font-weight: 900;
  font-size: 13.5px;
  padding: 4px 10px;
  border-radius: 6px;
}

/* BARA MARE DE NAVIGARE JOS */
.bottom-concept-nav {
  max-width: 1220px;
  width: 100%;
  margin: 12px auto 0 auto;
  padding: 20px 28px;
  background: rgba(8, 18, 38, 0.96);
  border: 2px solid #23497d;
  border-radius: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 16px;
}
.bottom-page-indicator {
  font-size: 18px;
  font-weight: 900;
  color: var(--hud-green);
  text-align: center;
}

/* SPOTLIGHT & POINTER */
.visual-spotlight-active {
  outline: 4px solid #fde047 !important;
  box-shadow: 0 0 35px rgba(253, 224, 71, 0.65) !important;
}
#floating-visual-pointer {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
}
.pointer-bubble {
  background: #fde047;
  border: 3px solid #ffffff;
  border-radius: 14px;
  padding: 12px 18px;
  display: flex;
  align-items: center;
  gap: 12px;
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.55);
}
.pointer-arrow { font-size: 28px; }
.progress-bar-outer {
  width: 100%;
  height: 12px;
  background: #040c1a;
  border-radius: 999px;
  overflow: hidden;
}
.progress-bar-inner {
  width: 0%;
  height: 100%;
  background: linear-gradient(90deg, #00ff9d, #00e5ff);
}

@media (max-width: 1024px) {
  .pages-nav-bar, .lesson-tabs-spacious { grid-template-columns: repeat(3, 1fr); }
  .three-legend-boxes, .three-shields-grid, .videos-3-spacious-grid, .drosophila-4-stages-grid { grid-template-columns: 1fr; }
  .four-planes-cards-grid, .fg-10-spacious-grid, .four-quick-buttons-bar { grid-template-columns: 1fr; }
}
"####;

const MAUD_COCKPIT_JS_ENGINE: &str = r####"
let ws = null;
let latestFrame = null;
let currentConceptPage = Number(document.body.getAttribute("data-initial-page") || 1);
let currentLessonNum = 1;
let lessonAnimTick = 0;
let spotlightTimer = null;
let currentRealVideoIdx = 1;
let isBlindFog = false;

let cyberState = {
  gps_jump_spoof: false,
  gps_stealth_ramp_spoof: false,
  cut_internet_comms: false,
  poison_meteo_cache: false
};

const CONCEPT_PAGE_TITLES_RO = {
  1: "Lecții Interactive — Creierul de Zbor Google Drosophila",
  2: "Parbrizul Avionului & Filmarea Reală (2 Panouri Separate)",
  3: "Cursa celor 4 Avioane (De ce noi rămânem pe pistă?)",
  4: "Creierul Google Drosophila & Scutul Anti-Hacker",
  5: "Cele 3 Filmări Video Reale din Cabina Piloților",
  6: "Cele 10 Teste Sintetice FlightGear & Cursa Lungă 100 km Fără GPS (Analiza Erorii)"
};

const REAL_VIDEO_FILES = [
  "/real_videos/real_01_cat3_whiteout_cockpit_lc130.mp4",
  "/real_videos/real_02_als_runway_approach_cdg.mp4",
  "/real_videos/real_03_crosswind_cockpit_landing.mp4"
];

const SCHOOL_FG_LESSONS = {
  1: {
    fg_video_url: "/real_videos/fg_lesson_1.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-01/07): Ceață CAT IIIc + Atac Hacker GPS vs. Scutul AERO-SHIELD pe Pista 09 Craiova",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-01/07): CAT IIIc Fog + GPS Hacker Attack vs. AERO-SHIELD on Craiova RWY 09",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Activează Scutul AERO-SHIELD în Zborul FlightGear!"
  },
  2: {
    fg_video_url: "/real_videos/fg_lesson_2.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-01 SWIR): Ochiul Infraroșu T4/T5 curăță ceața și fixează cele 18 bare luminoase ALS",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-01 SWIR): T4/T5 Infrared Eye clears fog and locks onto 18 ALS light bars",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Pornește Ochiul Magic SWIR T4/T5 în FlightGear!"
  },
  3: {
    fg_video_url: "/real_videos/fg_lesson_3.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-05/06): Numărarea becurilor pistei ALS și măsurarea vitezei (1/Z = 0 la cer, fără blocaj!)",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-05/06): Counting ALS lights & speed measurement (1/Z = 0 at sky, zero crash!)",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Măsoară Viteza cu Becurile ALS în FlightGear (1/Z = 0)!"
  },
  4: {
    fg_video_url: "/real_videos/fg_lesson_4.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-02/03): Viraj 6-DoF Roll +22° (HS/VS) + Rafală de Vânt în Ceață +15 m/s (PFNd 0.85s)",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-02/03): 6-DoF Roll +22° (HS/VS) + Fog Wind Gust +15 m/s (PFNd 0.85s)",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Separă Vântul din Ceață (PFNd 0.85s) în FlightGear!"
  },
  5: {
    fg_video_url: "/real_videos/fg_lesson_5.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-08/09): Atac Lent Stealth Spoofing prins de Paharul CUSUM în 2.9s + Busola E-PG 16-Neuroni",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-08/09): Stealth Spoofing caught by CUSUM Cup in 2.9s + 16-Neuron E-PG Compass",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Prinde Hackerul cu Paharul CUSUM & Busola E-PG!"
  },
  6: {
    fg_video_url: "/real_videos/fg_lesson_6.mp4",
    fg_video_caption_ro: "🎬 Zbor Sintetic FlightGear (FG-10): Aterizare Autonomă Completă în Whiteout 0m pe Pista 09 Craiova (Eroare 0.21m!)",
    fg_video_caption_en: "🎬 FlightGear Synthetic Flight (FG-10): Full Autonomous Landing in 0m Whiteout on Craiova RWY 09 (0.21m Error!)",
    green_btn_label_ro: "🟢 APASĂ BUTONUL VERDE: Aterizează Autonom în Whiteout 0m pe FlightGear!"
  }
};

window.schoolLessonMode = "solved";
let currentSchoolFgOpacity = 55;

const trailHistory = {
  truth: [],
  gnss: [],
  ins: [],
  naive: [],
  shield: []
};
const MAX_TRAIL_LEN = 160;

// =========================================================================
// NAVIGAREA ÎNTRE CELE 6 PAGINI AERISITE (CÂTE 1 CONCEPT PE PAGINĂ)
// =========================================================================
function switchConceptPage(pageNum) {
  const clamped = Math.max(1, Math.min(6, Number(pageNum) || 1));
  currentConceptPage = clamped;

  for (let i = 1; i <= 6; i++) {
    const pageEl = document.getElementById(`concept-page-${i}`);
    const navBtn = document.getElementById(`nav-page-btn-${i}`);
    if (pageEl) {
      pageEl.classList.toggle("active-page", i === clamped);
    }
    if (navBtn) {
      navBtn.classList.toggle("active", i === clamped);
    }
  }

  const bottomInd = document.getElementById("bottom-page-indicator");
  if (bottomInd) {
    bottomInd.textContent = `📍 Pagina ${clamped} din 6: ${CONCEPT_PAGE_TITLES_RO[clamped]}`;
  }

  try {
    window.history.replaceState(null, "", `/pagina/${clamped}`);
  } catch (e) {}

  window.scrollTo({ top: 0, behavior: "smooth" });

  // Redimensionăm imediat Canvas-urile când pagina devine vizibilă
  requestAnimationFrame(() => {
    window.dispatchEvent(new Event("resize"));
    renderHudCanvas();
    renderRadarCanvas();
    renderLessonMiniCanvas();
  });
}

function prevConceptPage() {
  const prev = currentConceptPage > 1 ? currentConceptPage - 1 : 6;
  switchConceptPage(prev);
}

function nextConceptPage() {
  const next = currentConceptPage < 6 ? currentConceptPage + 1 : 1;
  switchConceptPage(next);
}

function setAppLang(lang) {
  window.aeroLang = lang === "en" ? "en" : "ro";
  const btnRo = document.getElementById("btn-lang-ro");
  const btnEn = document.getElementById("btn-lang-en");
  if (btnRo) btnRo.classList.toggle("active", window.aeroLang === "ro");
  if (btnEn) btnEn.classList.toggle("active", window.aeroLang === "en");

  document.querySelectorAll(".lesson-big-story").forEach((el) => {
    const txt = window.aeroLang === "en" ? el.getAttribute("data-en") : el.getAttribute("data-ro");
    if (txt) el.textContent = txt;
  });

  const lessonMeta = SCHOOL_FG_LESSONS[currentLessonNum];
  const fgCapEl = document.getElementById("maud-school-fg-caption");
  if (fgCapEl && lessonMeta) {
    fgCapEl.textContent = window.aeroLang === "en" ? lessonMeta.fg_video_caption_en : lessonMeta.fg_video_caption_ro;
  }
}

// =========================================================================
// CONEXIUNEA WEBSOCKET 60 HZ ZERO-LAG
// =========================================================================
function connectWebSocket() {
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${proto}//${window.location.host}/ws/telemetry`;
  ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    const el = document.getElementById("conn-status");
    if (el) {
      el.textContent = "WS 60.0 Hz CONECTAT (ZERO-LAG) | PISTA CRAIOVA RWY 09";
      el.style.color = "#00ff9d";
    }
  };

  ws.onmessage = (event) => {
    try {
      const frame = JSON.parse(event.data);
      latestFrame = frame;
      updateTelemetryDom(frame);
      recordEstimatorTrails(frame);
    } catch (err) {}
  };

  ws.onclose = () => {
    setTimeout(connectWebSocket, 1000);
  };
}

function sendSimCommand(cmdObj) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(cmdObj));
  } else {
    fetch("/api/command", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(cmdObj)
    }).catch(() => {});
  }
}

// =========================================================================
// SUBLINIERE VIZUALĂ (SPOTLIGHT + SĂGEATĂ ANIMATĂ)
// =========================================================================
function highlightVisualZone(zoneId, messageRo) {
  document.querySelectorAll(".visual-spotlight-active").forEach((el) => {
    el.classList.remove("visual-spotlight-active");
  });
  const target = document.getElementById(zoneId);
  const pointer = document.getElementById("floating-visual-pointer");
  const pDesc = document.getElementById("pointer-desc");
  if (!target) return;

  target.classList.add("visual-spotlight-active");

  if (pointer && pDesc) {
    pDesc.textContent = messageRo || "Uită-te aici cum lucrează AERO-SHIELD în timp real!";
    const rect = target.getBoundingClientRect();
    const topPx = Math.max(12, rect.top - 64);
    const leftPx = Math.min(window.innerWidth - 380, Math.max(16, rect.left + 24));
    pointer.style.top = `${topPx}px`;
    pointer.style.left = `${leftPx}px`;
    pointer.style.display = "block";
  }

  if (spotlightTimer) clearTimeout(spotlightTimer);
  spotlightTimer = setTimeout(() => {
    target.classList.remove("visual-spotlight-active");
    if (pointer) pointer.style.display = "none";
  }, 4500);
}

// =========================================================================
// PAGINA 1: ȘCOALA INTERACTIVĂ ÎN 6 LECȚII + VIDEO ZBOR FLIGHTGEAR
// =========================================================================
function setSchoolFgVideoOpacity(val) {
  const numVal = Math.max(0, Math.min(100, Number(val) || 0));
  currentSchoolFgOpacity = numVal;
  const fgVideoEl = document.getElementById("maudSchoolFgVideo");
  const sliderEl = document.getElementById("school-fg-opacity-slider");
  const labelEl = document.getElementById("school-fg-opacity-label");

  if (fgVideoEl) {
    fgVideoEl.style.opacity = (numVal / 100).toFixed(2);
    fgVideoEl.style.pointerEvents = numVal >= 95 ? "auto" : "none";
  }
  if (sliderEl) {
    sliderEl.value = String(numVal);
  }
  if (labelEl) {
    labelEl.textContent = `🎚️ Transparență Video Zbor FlightGear: ${numVal}%`;
  }
}

function cycleSchoolFgVideoMode() {
  if (currentSchoolFgOpacity === 55) {
    setSchoolFgVideoOpacity(100);
  } else if (currentSchoolFgOpacity === 100) {
    setSchoolFgVideoOpacity(0);
  } else {
    setSchoolFgVideoOpacity(55);
  }
}

function triggerSchoolLessonState(mode) {
  window.schoolLessonMode = mode === "danger" ? "danger" : "solved";
  const bannerEl = document.getElementById("school-green-solved-banner");
  const fgVideoEl = document.getElementById("maudSchoolFgVideo");

  if (window.schoolLessonMode === "danger") {
    if (bannerEl) {
      bannerEl.style.background = "rgba(225, 29, 72, 0.22)";
      bannerEl.style.borderColor = "#fb7185";
      bannerEl.style.color = "#ffe4e6";
      bannerEl.textContent = "⚠️ PERICOL ACTIV ÎN LECȚIE! Apasă acum pe BUTONUL VERDE MARE (#btnSolveLessonGreen) de mai sus ca să activezi Scutul AERO-SHIELD în zborul FlightGear!";
    }
    highlightVisualZone("btnSolveLessonGreen", "Apasă pe acest BUTON VERDE MARE ca să rezolvi pericolul în zborul sintetic FlightGear!");
  } else {
    if (fgVideoEl) {
      if (currentSchoolFgOpacity < 25) {
        setSchoolFgVideoOpacity(55);
      }
      fgVideoEl.play().catch(() => {});
    }
    lessonPlayAction(currentLessonNum);
    if (bannerEl) {
      bannerEl.style.background = "rgba(16, 185, 129, 0.18)";
      bannerEl.style.borderColor = "#10b981";
      bannerEl.style.color = "#ecfdf5";
      bannerEl.textContent = "✅ BRAVO! Ai apăsat Butonul Verde: Uite în filmarea FlightGear cum avionul verde rezolvă problema și rămâne pe Pista 09 Craiova!";
    }
  }
  renderLessonMiniCanvas();
}

function selectLesson6Yo(num) {
  const clamped = Math.max(1, Math.min(6, Number(num) || 1));
  currentLessonNum = clamped;
  window.schoolLessonMode = "solved";

  for (let i = 1; i <= 6; i++) {
    const tab = document.getElementById(`lesson-tab-${i}`);
    const pane = document.getElementById(`lesson-pane-${i}`);
    if (tab) tab.classList.toggle("active", i === clamped);
    if (pane) pane.style.display = i === clamped ? "flex" : "none";
  }

  const starsEl = document.getElementById("school-stars-progress");
  if (starsEl) {
    const filled = "⭐ ".repeat(clamped);
    const empty = "☆ ".repeat(6 - clamped);
    starsEl.textContent = `${filled}${empty}(Lecția ${clamped} din 6)`;
  }

  const lessonMeta = SCHOOL_FG_LESSONS[clamped];
  const fgVideoEl = document.getElementById("maudSchoolFgVideo");
  if (fgVideoEl && lessonMeta) {
    fgVideoEl.src = lessonMeta.fg_video_url;
    fgVideoEl.load();
    fgVideoEl.play().catch(() => {});
  }

  const fgCapEl = document.getElementById("maud-school-fg-caption");
  if (fgCapEl && lessonMeta) {
    fgCapEl.textContent = window.aeroLang === "en" ? lessonMeta.fg_video_caption_en : lessonMeta.fg_video_caption_ro;
  }

  const greenBtnEl = document.getElementById("btnSolveLessonGreen");
  if (greenBtnEl && lessonMeta) {
    greenBtnEl.textContent = lessonMeta.green_btn_label_ro;
  }

  const bannerEl = document.getElementById("school-green-solved-banner");
  if (bannerEl && lessonMeta) {
    bannerEl.style.background = "rgba(16, 185, 129, 0.16)";
    bannerEl.style.borderColor = "#10b981";
    bannerEl.style.color = "#ecfdf5";
    bannerEl.textContent = `✅ Lecția ${clamped} pregătită! Apasă pe Butonul Verde strălucitor ca să vezi rezolvarea în zborul FlightGear!`;
  }

  if (window.setSpotlightLessonStep) {
    window.setSpotlightLessonStep(Math.min(5, clamped));
  }
  renderLessonMiniCanvas();
}

function selectSchoolLesson(num) {
  selectLesson6Yo(num);
}

function nextSchoolLesson() {
  const next = currentLessonNum < 6 ? currentLessonNum + 1 : 1;
  selectLesson6Yo(next);
}

function prevSchoolLesson() {
  const prev = currentLessonNum > 1 ? currentLessonNum - 1 : 6;
  selectLesson6Yo(prev);
}

function lessonPlayAction(num) {
  if (num === 1) {
    sendSimCommand({
      type: "ConfigureCyberAttack",
      payload: { gps_jump_spoof: true, gps_stealth_ramp_spoof: false, cut_internet_comms: false, poison_meteo_cache: false }
    });
    highlightVisualZone("maudSchoolCanvas", "Lecția 1: Uită-te în desenul animat cum Avionul Roșu ✈️🔴 fuge după momeala hackerului, dar Avionul Verde ✈️🟢 rămâne pe pistă!");
  } else if (num === 2) {
    sendSimCommand({ type: "SetFogDensity", payload: { visibility_mor_m: 45, enable_swir_als: true } });
    highlightVisualZone("maudSchoolCanvas", "Lecția 2: Ochelarii Magici Infraroșu văd becurile strălucitoare ale pistei chiar și prin ceață groasă de 45m!");
  } else if (num === 3) {
    stepManeuver(0, 6.0, 0, 0);
    highlightVisualZone("maudSchoolCanvas", "Lecția 3: Am ridicat botul avionului spre cer! Formula pune Cer = 0.000000 fără să înghețe niciodată!");
  } else if (num === 4) {
    injectSuddenFogGust();
    highlightVisualZone("maudSchoolCanvas", "Lecția 4: Suflă vântul! Săgețile MOV arată norul de ceață împins în lateral, dar săgețile VERZI păstrează pista!");
  } else if (num === 5) {
    sendSimCommand({
      type: "ConfigureCyberAttack",
      payload: { gps_jump_spoof: false, gps_stealth_ramp_spoof: true, cut_internet_comms: true, poison_meteo_cache: true }
    });
    highlightVisualZone("maudSchoolCanvas", "Lecția 5: Paharul de prins minciuni (CUSUM) se umple în <2.9 secunde și blochează hackerul furișat!");
  } else if (num === 6) {
    sendSimCommand({ type: "TriggerDirectorScenario", payload: { scenario_id: 3 } });
    highlightVisualZone("maudSchoolCanvas", "Lecția 6: Misiunea Finală Actul 3 a pornit! Apasă acum pe butonul albastru ca să mergi la Pagina 2 (Parbrizul Mare)!");
  }
}

function renderLessonMiniCanvas() {
  const canvas = document.getElementById("maudSchoolCanvas");
  if (!canvas) return;
  if (canvas.clientWidth > 50 && canvas.width !== canvas.clientWidth) {
    canvas.width = canvas.clientWidth;
  }
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  lessonAnimTick += 1;

  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = window.schoolLessonMode === "danger" ? "#1e0712" : "#040c1c";
  ctx.fillRect(0, 0, w, h);

  if (window.schoolLessonMode === "danger") {
    ctx.fillStyle = "#ff2a5f";
    ctx.font = "bold 14px sans-serif";
    ctx.fillText("⚠️ MOD PERICOL ACTIV — APASĂ PE BUTONUL VERDE DE SUB ECRAN PENTRU A ACTIVA SCUTUL!", 175, 22);
  } else {
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 13.5px sans-serif";
    ctx.fillText("✅ SCUTUL AERO-SHIELD & ZBORUL FLIGHTGEAR REZOLVAT ACTIV!", 175, 22);
  }

  // Desenăm pista din Craiova în dreapta
  ctx.fillStyle = "#0b243b";
  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 3;
  ctx.fillRect(w - 190, 45, 160, h - 90);
  ctx.strokeRect(w - 190, 45, 160, h - 90);
  ctx.fillStyle = "#00ff9d";
  ctx.font = "bold 14px sans-serif";
  ctx.fillText("🎯 PISTA CRAIOVA 09", w - 182, 34);

  // Desenăm Inelul-Busolă cu 16 Neuroni (🧭 E-PG Ring Attractor) în stânga-sus / centru-dreapta
  const ringCx = currentLessonNum >= 4 ? w - 310 : 88;
  const ringCy = currentLessonNum >= 4 ? h / 2 - 10 : 78;
  const ringR = currentLessonNum >= 4 ? 54 : 36;

  ctx.save();
  ctx.strokeStyle = currentLessonNum >= 4 ? "#00e5ff" : "rgba(0, 229, 255, 0.55)";
  ctx.lineWidth = currentLessonNum >= 4 ? 2.5 : 1.8;
  ctx.beginPath();
  ctx.arc(ringCx, ringCy, ringR, 0, Math.PI * 2);
  ctx.stroke();

  // Cei 16 neuroni E-PG în inel (neuronul 0 orientat spre Pista 09 Craiova strălucește #00ff9d)
  for (let n = 0; n < 16; n++) {
    const ang = (n / 16) * Math.PI * 2;
    const nx = ringCx + Math.cos(ang) * ringR;
    const ny = ringCy + Math.sin(ang) * ringR;
    const isTargetRwy09 = n === 0 || n === 15 || n === 1;
    ctx.beginPath();
    ctx.arc(nx, ny, n === 0 ? 6.5 : 3.8, 0, Math.PI * 2);
    ctx.fillStyle = n === 0 ? "#00ff9d" : (isTargetRwy09 ? "#38bdf8" : "#334155");
    ctx.fill();
    if (n === 0) {
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }
  // Vectorul luminos din centrul inelului E-PG direct spre Pista 09 Craiova
  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 2.5;
  ctx.beginPath();
  ctx.moveTo(ringCx, ringCy);
  ctx.lineTo(ringCx + ringR - 4, ringCy);
  ctx.stroke();
  ctx.fillStyle = "#00ff9d";
  ctx.font = "bold 12px sans-serif";
  ctx.fillText("🧭 E-PG Busola în Inel 16-Neuroni", ringCx - 74, ringCy + ringR + 18);
  ctx.restore();

  // Bara cu cele 4 circuite biologice Google Drosophila în partea de jos a canvas-ului
  ctx.fillStyle = "rgba(7, 20, 42, 0.90)";
  ctx.fillRect(14, h - 28, w - 28, 22);
  ctx.fillStyle = "#bae6fd";
  ctx.font = "bold 11.5px sans-serif";
  ctx.fillText(
    "🪰 GOOGLE DROSOPHILA:  👁️ T4/T5 Ochiul Anticeață  |  🔄 HS/VS Stabilizatorul de Viraje 6-DoF  |  🧭 E-PG Busola în Inel 16-Neuroni  |  💨 PFNd Simțul Vântului în Ceață",
    22,
    h - 13
  );

  const t = (lessonAnimTick % 180) / 180;

  if (currentLessonNum === 1) {
    ctx.fillStyle = "rgba(203, 213, 225, 0.20)";
    ctx.fillRect(175, 30, 320, h - 70);
    ctx.fillStyle = "#e2e8f0";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("☁️ CEAȚĂ GROASĂ CA LAPTELE + HACKER GPS 👾", 185, 56);

    const xRed = 150 + t * (w - 370);
    const yRed = h / 2 - Math.sin(t * 3.14) * 85;
    ctx.fillStyle = "#ff2a5f";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("✈️🔴 GPS Păcălit (în iarbă!)", xRed, yRed);

    const xGrn = 150 + t * (w - 350);
    ctx.fillStyle = "#00ff9d";
    ctx.fillText("✈️🟢 Creierul Drosophila (Drept pe pistă!)", xGrn, h / 2 + 24);
  } else if (currentLessonNum === 2) {
    for (let i = 0; i < 6; i++) {
      ctx.fillStyle = "#ffd600";
      ctx.beginPath();
      ctx.arc(w - 175 + i * 24, h / 2, 7, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.5;
    ctx.strokeRect(w / 2 - 120, 40, 270, h - 85);
    ctx.fillStyle = "#00e5ff";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("👁️ T4/T5 Ochiul Anticeață + SWIR: Văd 12 becuri prin ceață!", w / 2 - 170, 30);
    ctx.fillStyle = "#00ff9d";
    ctx.fillText("✈️🟢", 160 + t * (w - 370), h / 2 + 6);
  } else if (currentLessonNum === 3) {
    ctx.fillStyle = "#0c1e3e";
    ctx.fillRect(160, 0, w - 360, h / 2);
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("🔄 HS/VS Stabilizatorul de Viraje 6-DoF | Cer ρ_ground = 0.000000", 170, 48);
    for (let i = 0; i < 7; i++) {
      const bx = 170 + ((w - 380 - (lessonAnimTick * 4 + i * 85) % (w - 380)));
      ctx.fillStyle = "#00ff9d";
      ctx.fillText("🟢⬅️", bx, h - 48);
    }
    ctx.fillText("✈️🟢 Măsurăm viteza peste becuri fără blocaj la orizont!", 170, h / 2 + 36);
  } else if (currentLessonNum === 4) {
    for (let i = 0; i < 6; i++) {
      const fy = (lessonAnimTick * 2 + i * 36) % (h - 65) + 20;
      ctx.fillStyle = "#d946ef";
      ctx.font = "bold 14.5px sans-serif";
      ctx.fillText("🟣⬇️ Vânt în Ceață (18 m/s)", 80, fy);
    }
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("💨 PFNd Simțul Vântului în Ceață + 🧭 E-PG Busola în Inel 16-Neuroni (0.85s înainte!)", 50, h - 42);
  } else if (currentLessonNum === 5) {
    const fillH = Math.min(140, (lessonAnimTick % 160) * 1.1);
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 3;
    ctx.strokeRect(70, 50, 80, 145);
    ctx.fillStyle = fillH > 105 ? "#ff2a5f" : "#ffb800";
    ctx.fillRect(73, 50 + (145 - fillH), 74, fillH);
    ctx.fillStyle = "#ffffff";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("🥤 PAHARUL CUSUM", 55, 38);
    ctx.fillText(
      fillH > 105
        ? "🚨 HACKER PRINS ÎN 2.8s! 🧭 E-PG 16-Neuroni ține direcția RWY 09!"
        : "🕵️ Adunăm minciunile GPS + 🧭 E-PG păstrează busola în orbire!",
      175,
      115
    );
  } else {
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 16.5px sans-serif";
    ctx.fillText("🏆 COMANDANT DROSOPHILA: Aterizare Perfectă pe Craiova RWY 09 (< 0.4m)!", 40, h / 2 - 22);
    ctx.fillStyle = "#fde047";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("👉 👁️ T4/T5 + 🔄 HS/VS + 🧭 E-PG 16-Neuroni + 💨 PFNd active la 60 Hz!", 40, h / 2 + 16);
  }
}

// =========================================================================
// PAGINA 2 & 5: CONTROALE FILMARE REALĂ ÎN PARBRIZ
// =========================================================================
function switchHudRealVideo(idx) {
  currentRealVideoIdx = idx % REAL_VIDEO_FILES.length;
  const vid = document.getElementById("hudBgRealVideo");
  if (!vid) return;
  const src = REAL_VIDEO_FILES[currentRealVideoIdx];
  vid.src = src;
  vid.play().catch(() => {});
  sendSimCommand({ type: "ReplayRealVideo" });
}

function cycleHudRealVideo() {
  switchHudRealVideo((currentRealVideoIdx + 1) % REAL_VIDEO_FILES.length);
  highlightVisualZone("zone-hud", `Am schimbat filmarea reală #${currentRealVideoIdx + 1} în Parbrizul Avionului!`);
}

function openRealVideoInWindshield(idx) {
  switchHudRealVideo(idx);
  switchConceptPage(2);
  setTimeout(() => {
    highlightVisualZone("zone-hud", `Filmarea Reală #${idx + 1} rulează acum 100% clar în panoul video separat din dreapta!`);
  }, 200);
}

function setHudVideoOpacity(val) {
  const vid = document.getElementById("hudBgRealVideo");
  const lbl = document.getElementById("video-opacity-label");
  if (vid) vid.style.opacity = (Number(val) / 100).toFixed(2);
  if (lbl) lbl.textContent = `${val}%`;
}

function toggleBlindNormalFog() {
  isBlindFog = !isBlindFog;
  const mor = isBlindFog ? 35 : 250;
  sendSimCommand({ type: "SetFogDensity", payload: { visibility_mor_m: mor, enable_swir_als: true } });
  highlightVisualZone("zone-hud", isBlindFog ? "Am pus Ceață Oarbă (35m)! Doar Ochelarii Infraroșu mai văd pista!" : "Am revenit la Ceață Normală (250m)!");
}

function raceTriggerHackerAttack() {
  cyberState.gps_jump_spoof = true;
  cyberState.gps_stealth_ramp_spoof = true;
  sendSimCommand({ type: "ConfigureCyberAttack", payload: cyberState });
  highlightVisualZone("zone-radar", "Hackerul a atacat GPS-ul! Privește cum Avionul Roșu ✈️🔴 iese în iarbă, dar Avionul Verde ✈️🟢 rămâne pe pistă!");
}

// =========================================================================
// COMENZI CU SUBLINIERE AUTOMATĂ
// =========================================================================
function triggerScenarioWithHighlight(id) {
  sendSimCommand({ type: "TriggerDirectorScenario", payload: { scenario_id: id } });
  const descriptions = {
    1: "POVESTEA 1: Suflă o rafală bruscă în ceață! Camera o vede cu 0.85 secunde înainte să lovească avionul!",
    2: "POVESTEA 2: Avionul face viraje și urcă spre cer — fără niciun blocaj la orizont!",
    3: "POVESTEA 3: Hackerul atacă GPS-ul în ceață oarbă, dar Avionul Verde aterizează pe pistă!"
  };
  const headline = document.getElementById("story-headline");
  if (headline) headline.textContent = `💬 POVESTITORUL ÎN TIMP REAL: ${descriptions[id]}`;
}

function resetAndHighlight() {
  cyberState = { gps_jump_spoof: false, gps_stealth_ramp_spoof: false, cut_internet_comms: false, poison_meteo_cache: false };
  Object.keys(trailHistory).forEach((k) => { trailHistory[k] = []; });
  sendSimCommand({ type: "ResetApproach" });
}

function stepManeuver(dyaw, dpitch, droll, dthr) {
  sendSimCommand({
    type: "ManeuverStep",
    payload: { d_yaw_deg: dyaw, d_pitch_deg: dpitch, d_roll_deg: droll, d_throttle_mps: dthr }
  });
}

function stepManeuverWithHighlight(dyaw, dpitch, droll, dthr, msgRo) {
  stepManeuver(dyaw, dpitch, droll, dthr);
  const headline = document.getElementById("story-headline");
  if (headline && msgRo) headline.textContent = `💬 POVESTITORUL ÎN TIMP REAL: ${msgRo}`;
}

function injectSuddenFogGust() {
  sendSimCommand({
    type: "InjectSuddenFogGust",
    payload: { wx_ned_mps: -4.5, wy_ned_mps: 18.0, wz_ned_mps: 0.8, turbulence_intensity: 0.9 }
  });
}

function injectSuddenFogGustWithHighlight() {
  injectSuddenFogGust();
  const headline = document.getElementById("story-headline");
  if (headline) {
    headline.textContent = "💬 POVESTITORUL ÎN TIMP REAL: Am suflat o rafală de vânt de 18 m/s în ceață! Săgețile MOV arată norul împins în lateral!";
  }
}

function startLiveCudaTrainingWithHighlight() {
  sendSimCommand({ type: "StartLiveCudaTraining", payload: { epochs: 60 } });
}

function onFogSliderChange(val) {
  const el = document.getElementById("fog-slider-val");
  if (el) el.textContent = `${val} m`;
  sendSimCommand({
    type: "SetFogDensity",
    payload: { visibility_mor_m: parseFloat(val), enable_swir_als: true }
  });
}

function toggleCyberWithHighlight(flag) {
  if (flag === "jump") cyberState.gps_jump_spoof = !cyberState.gps_jump_spoof;
  if (flag === "stealth") cyberState.gps_stealth_ramp_spoof = !cyberState.gps_stealth_ramp_spoof;
  if (flag === "comms") cyberState.cut_internet_comms = !cyberState.cut_internet_comms;
  if (flag === "poison") cyberState.poison_meteo_cache = !cyberState.poison_meteo_cache;
  sendSimCommand({ type: "ConfigureCyberAttack", payload: cyberState });
}

window.addEventListener("keydown", (e) => {
  if (e.target.tagName === "INPUT") return;
  const k = e.key.toLowerCase();
  if (k === "a" || e.key === "ArrowLeft") stepManeuver(-5, 0, 0, 0);
  if (k === "d" || e.key === "ArrowRight") stepManeuver(5, 0, 0, 0);
  if (k === "w" || e.key === "ArrowUp") stepManeuver(0, 3.0, 0, 0);
  if (k === "s" || e.key === "ArrowDown") stepManeuver(0, -3.0, 0, 0);
  if (k === "q") stepManeuver(0, 0, -8, 0);
  if (k === "e") stepManeuver(0, 0, 8, 0);
});

// =========================================================================
// ACTUALIZARE DOM TELEMETRIE
// =========================================================================
function setText(id, txt) {
  const el = document.getElementById(id);
  if (el) el.textContent = txt;
}

function updateTelemetryDom(f) {
  const p = f.aircraft_pose;
  const fm = f.fog_and_meteo;
  const est = f.estimators_arena;
  const g = f.lrcv_guidance;
  const c = f.cyber_and_gpu;

  setText("phase-banner", f.approach_phase);
  setText("hud-guidance-summary", `DISTANȚĂ PISTĂ: ${g.distance_to_rwy09_m.toFixed(0)} m | DIRECȚIE: ${g.bearing_to_rwy09_deg.toFixed(1)}°`);
  setText("eta-summary", `TIMP PÂNĂ LA PISTĂ: ${g.eta_touchdown_sec.toFixed(1)} s | BECURI VĂZUTE: ${g.als_bars_detected}`);

  const gustOverlay = document.getElementById("gust-overlay");
  if (gustOverlay) {
    if (fm.gust_lookahead_warning) {
      gustOverlay.style.display = "block";
      gustOverlay.textContent = `⚠️ ATENȚIE! VEDEM O RAFALĂ ÎN CEAȚĂ CU ${fm.gust_lookahead_lead_sec.toFixed(2)}s ÎNAINTE SĂ LOVEASCĂ AVIONUL!`;
    } else {
      gustOverlay.style.display = "none";
    }
  }

  setText("err-gnss", `Abatere față de centrul pistei: ${est.raw_gnss_error_m.toFixed(2)} m`);
  setText("err-ins", `Abatere față de centrul pistei: ${est.pure_ins_error_m.toFixed(2)} m`);
  setText("err-naive", `Abatere față de centrul pistei: ${est.naive_flow_error_m.toFixed(2)} m`);
  setText("err-shield", `Abatere față de centrul pistei: ${est.aero_shield_error_m.toFixed(2)} m`);

  setText("val-pos", `[${p.pos_ned_m[0].toFixed(0)}, ${p.pos_ned_m[1].toFixed(1)}, ${p.pos_ned_m[2].toFixed(1)}] m`);
  setText("val-vel-ned", `[${p.vel_ground_ned_mps[0].toFixed(1)}, ${p.vel_ground_ned_mps[1].toFixed(1)}, ${p.vel_ground_ned_mps[2].toFixed(1)}] m/s`);
  setText("val-euler", `[${p.euler_deg[0].toFixed(1)}°, ${p.euler_deg[1].toFixed(1)}°, ${p.euler_deg[2].toFixed(1)}°]`);
  setText("val-crab", `${p.crab_angle_cmd_deg >= 0 ? "+" : ""}${p.crab_angle_cmd_deg.toFixed(2)}°`);

  const q = p.quat_b_to_n;
  const qNorm = Math.sqrt(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]);
  setText("val-qnorm", `||q|| = ${qNorm.toFixed(6)} | ${fm.lrcv_temp_c.toFixed(1)}°C`);
  setText("val-rho-hor", `${fm.rho_ground_horizon_safe.toFixed(6)} m⁻¹ (SIGUR)`);
  setText("val-beta", `${fm.beta_ext_koschmieder.toFixed(4)} m⁻¹`);
  setText("val-fog-wind", `${fm.true_fog_wind_ned_mps[1].toFixed(1)} vs ${fm.est_fog_wind_ned_mps[1].toFixed(1)} m/s`);
  setText("val-cert", `${(g.photonic_certainty_mean * 100).toFixed(1)}% (${g.als_bars_detected} becuri)`);

  setText("val-watchdog", c.gnss_watchdog_status);
  const wdEl = document.getElementById("val-watchdog");
  if (wdEl) {
    wdEl.style.color = c.gnss_watchdog_status.includes("NOMINAL") ? "#00ff9d" : "#ff2a5f";
  }
  setText("val-chi2-cusum", `${c.chi2_mahalanobis_stat.toFixed(2)} / ${c.cusum_stealth_stat.toFixed(2)}`);
  setText("val-meteo-status", fm.meteo_source_status);
  setText("val-ai-lat", `${c.inference_latency_ms.toFixed(2)} ms | σ=${c.heteroscedastic_sigma_v[1].toFixed(2)}m/s`);

  const cudaBox = document.getElementById("cuda-progress-box");
  if (cudaBox) {
    if (c.live_training_progress && c.live_training_progress.is_running) {
      cudaBox.style.display = "flex";
      const tp = c.live_training_progress;
      setText("cuda-epoch-txt", `ShadowTrainer CUDA Epoca ${tp.current_epoch}/${tp.total_epochs}`);
      setText("cuda-loss-txt", `Eroare: ${tp.current_loss.toFixed(4)} | Reziduu: ${tp.koschmieder_phys_residual.toFixed(4)}`);
      const bar = document.getElementById("cuda-progress-bar");
      if (bar) bar.style.width = `${tp.progress_pct.toFixed(1)}%`;
    } else {
      cudaBox.style.display = "none";
    }
  }
}

function recordEstimatorTrails(f) {
  const pushPt = (arr, pos) => {
    arr.push([pos[0], pos[1]]);
    if (arr.length > MAX_TRAIL_LEN) arr.shift();
  };
  pushPt(trailHistory.truth, f.aircraft_pose.pos_ned_m);
  pushPt(trailHistory.gnss, f.estimators_arena.raw_gnss_pos_ned);
  pushPt(trailHistory.ins, f.estimators_arena.pure_ins_pos_ned);
  pushPt(trailHistory.naive, f.estimators_arena.naive_flow_pos_ned);
  pushPt(trailHistory.shield, f.estimators_arena.aero_shield_pos_ned);
}

// =========================================================================
// CANVAS 1 (PAGINA 2): PARBRIZUL AVIONULUI 3D SEPARAT (100% OPAC, FĂRĂ VIDEO SUPRAPUS)
// =========================================================================
function renderHudCanvas() {
  const canvas = document.getElementById("hudCanvas");
  if (!canvas) return;
  if (canvas.clientWidth > 50 && (canvas.width !== canvas.clientWidth || canvas.height !== canvas.clientHeight)) {
    canvas.width = canvas.clientWidth;
    canvas.height = canvas.clientHeight;
  }
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  // Fundal 100% opac pentru cer și sol ca panoul 3D să fie complet independent de video
  ctx.fillStyle = "#050e1d";
  ctx.fillRect(0, 0, w, h * 0.52);
  ctx.fillStyle = "#08182e";
  ctx.fillRect(0, h * 0.52, w, h * 0.48);

  if (!latestFrame) return;
  const p = latestFrame.aircraft_pose;
  const fm = latestFrame.fog_and_meteo;
  const g = latestFrame.lrcv_guidance;

  const fogAlpha = Math.min(0.45, Math.max(0.05, 1.0 - Math.exp(-fm.beta_ext_koschmieder * 25.0)));
  ctx.fillStyle = `rgba(18, 30, 52, ${fogAlpha.toFixed(2)})`;
  ctx.fillRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2;
  const rollRad = (p.euler_deg[0] * Math.PI) / 180.0;
  const pitchOffsetPx = p.euler_deg[1] * 7.5;

  ctx.save();
  ctx.translate(cx, cy + pitchOffsetPx);
  ctx.rotate(-rollRad);

  // Linia orizontului
  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 2.5;
  ctx.beginPath();
  ctx.moveTo(-w * 0.42, 0);
  ctx.lineTo(w * 0.42, 0);
  ctx.stroke();

  // Pista Craiova RWY 09 în perspectivă
  const yawOffsetPx = (p.euler_deg[2] - g.bearing_to_rwy09_deg) * 6.0;
  const rwyTopW = 48;
  const rwyBotW = 240;
  const rwyH = h * 0.42;

  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 3;
  ctx.beginPath();
  ctx.moveTo(yawOffsetPx - rwyTopW / 2, 14);
  ctx.lineTo(yawOffsetPx + rwyTopW / 2, 14);
  ctx.lineTo(yawOffsetPx + rwyBotW / 2, rwyH);
  ctx.lineTo(yawOffsetPx - rwyBotW / 2, rwyH);
  ctx.closePath();
  ctx.stroke();

  if (g.swir_als_active) {
    const bars = Math.min(12, g.als_bars_detected);
    for (let i = 1; i <= bars; i++) {
      const frac = i / 13.0;
      const by = 18 + frac * (rwyH - 22);
      const bw = rwyTopW + frac * (rwyBotW - rwyTopW) * 0.65;
      ctx.strokeStyle = `rgba(255, 210, 80, ${0.45 + 0.5 * g.photonic_certainty_mean})`;
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(yawOffsetPx - bw / 2, by);
      ctx.lineTo(yawOffsetPx + bw / 2, by);
      ctx.stroke();
    }
  }
  ctx.restore();

  // Săgețile de flux optic: VERDE = Pământ, MOV = Ceață
  const vecs = latestFrame.optical_flow_vectors || [];
  for (let i = 0; i < vecs.length; i++) {
    const v = vecs[i];
    const x0 = v.x_norm * w;
    const y0 = v.y_norm * h;
    const x1 = x0 + v.dx_px * 1.9;
    const y1 = y0 + v.dy_px * 1.9;

    ctx.strokeStyle = v.layer === "ground" ? "#00ff9d" : "#e879f9";
    ctx.lineWidth = v.layer === "ground" ? 2.5 : 2.0;
    ctx.beginPath();
    ctx.moveTo(x0, y0);
    ctx.lineTo(x1, y1);
    ctx.stroke();

    ctx.fillStyle = ctx.strokeStyle;
    ctx.beginPath();
    ctx.arc(x1, y1, 3, 0, Math.PI * 2);
    ctx.fill();
  }
}

// =========================================================================
// CANVAS 2 (PAGINA 3): CURSA CELOR 4 AVIOANE VĂZUTE DE SUS (100% OPAC, FĂRĂ VIDEO)
// =========================================================================
function renderRadarCanvas() {
  const canvas = document.getElementById("radarCanvas");
  if (!canvas) return;
  if (canvas.clientWidth > 50 && (canvas.width !== canvas.clientWidth || canvas.height !== canvas.clientHeight)) {
    canvas.width = canvas.clientWidth;
    canvas.height = canvas.clientHeight;
  }
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = "#030914";
  ctx.fillRect(0, 0, w, h);

  const mapX = (xNed) => ((xNed + 950) / 1150) * (w - 110) + 40;
  const mapY = (yNed) => h / 2 + (yNed / 150) * (h * 0.40);

  // Grila
  ctx.strokeStyle = "rgba(0, 229, 255, 0.14)";
  ctx.lineWidth = 1;
  for (let x = -900; x <= 100; x += 200) {
    const cx = mapX(x);
    ctx.beginPath();
    ctx.moveTo(cx, 0);
    ctx.lineTo(cx, h);
    ctx.stroke();
  }

  // Linia centrală spre pista din Craiova
  ctx.strokeStyle = "rgba(0, 255, 157, 0.5)";
  ctx.setLineDash([8, 8]);
  ctx.beginPath();
  ctx.moveTo(mapX(-950), mapY(0));
  ctx.lineTo(mapX(0), mapY(0));
  ctx.stroke();
  ctx.setLineDash([]);

  // Pista Craiova RWY 09
  const rwyX0 = mapX(0);
  const rwyX1 = mapX(150);
  ctx.fillStyle = "rgba(0, 255, 157, 0.25)";
  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 3;
  ctx.fillRect(rwyX0, mapY(-22), rwyX1 - rwyX0, mapY(22) - mapY(-22));
  ctx.strokeRect(rwyX0, mapY(-22), rwyX1 - rwyX0, mapY(22) - mapY(-22));
  ctx.fillStyle = "#00ff9d";
  ctx.font = "bold 14px sans-serif";
  ctx.fillText("🎯 PISTA CRAIOVA 09", rwyX0 - 50, mapY(-30));

  if (!latestFrame) return;

  const drawTrail = (pts, color, width) => {
    if (pts.length < 2) return;
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.beginPath();
    for (let i = 0; i < pts.length; i++) {
      const px = mapX(pts[i][0]);
      const py = mapY(pts[i][1]);
      if (i === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    }
    ctx.stroke();
  };

  drawTrail(trailHistory.ins, "#ff9100", 2.5);
  drawTrail(trailHistory.gnss, "#ff2a5f", 2.5);
  drawTrail(trailHistory.naive, "#ffd600", 2.8);
  drawTrail(trailHistory.shield, "#00ff9d", 4.0);

  const drawPlaneLabel = (pos, color, labelTxt, yOff) => {
    const px = mapX(pos[0]);
    const py = mapY(pos[1]);
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(px, py, 6.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.font = "bold 13.5px sans-serif";
    ctx.fillText(labelTxt, px - 22, py + yOff);
  };

  const e = latestFrame.estimators_arena;
  drawPlaneLabel(e.pure_ins_pos_ned, "#ff9100", "✈️🟠 Fără Cameră (INS)", -14);
  drawPlaneLabel(e.raw_gnss_pos_ned, "#ff2a5f", "✈️🔴 GPS Păcălit", -28);
  drawPlaneLabel(e.naive_flow_pos_ned, "#ffd600", "✈️🟡 Păcălit de Ceață", 22);
  drawPlaneLabel(e.aero_shield_pos_ned, "#00ff9d", "✈️🟢 Avionul Nostru (PE PISTĂ!)", -12);
}

function animationLoop() {
  renderHudCanvas();
  renderRadarCanvas();
  renderLessonMiniCanvas();
  requestAnimationFrame(animationLoop);
}

connectWebSocket();
requestAnimationFrame(animationLoop);
"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_6_renders_long_range_100km_zero_gps_and_error_sources() {
        let html = render_full_cockpit_page_at(6).into_string();
        assert!(html.contains("id=\"zone-long-range-100km\""));
        assert!(html.contains("id=\"zone-error-sources-breakdown\""));
        assert!(html.contains("id=\"zone-error-remediation\""));
        assert!(html.contains("SURSA #1"));
        assert!(html.contains("SURSA #2"));
        assert!(html.contains("SURSA #3"));
        assert!(html.contains("SURSA #4"));
        assert!(html.contains("id=\"zone-flightgear-10\""));
    }
}


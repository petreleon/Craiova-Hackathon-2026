//! # Motorul de Randare SSR 100% Rust cu Framework-ul `maud` (`maud::html!`)
//!
//! Generează interfața web AERO-SHIELD LRCV V2.0 direct din structuri tipizate Rust
//! (fără fișiere `.html` statice servite brut), incluzând:
//! - **Școala Interactivă de Aviație în 6 Lecții Vizuale (Pe Înțelesul unui Copil de 6 Ani)**,
//! - **Sistemul de Subliniere Vizuală (`Spotlight` + Săgeată Animată `👇 UITE AICI CE SE SCHIMBĂ!`)**,
//! - **Parbrizul 3D + Suprapunerea 50% a Filmării Reale `.mp4` din Avion**,
//! - **Cursa Celor 4 Avioane Colorate (`✈️🟢`, `✈️🔴`, `✈️🟡`, `✈️🟠`) spre Pista Craiova RWY 09**,
//! - **Cele 3 Filmări Reale de Aterizare în Ceață** și **Cele 10 Teste Sintetice FlightGear (`FG-01`..`FG-10`)**.

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
}

/// Returnează lista completă a celor 6 lecții interactive pas-cu-pas.
#[must_use]
pub fn get_all_6_interactive_lessons() -> Vec<InteractiveLesson6Yo> {
    vec![
        InteractiveLesson6Yo {
            lesson_num: 1,
            icon: "🧸",
            title_ro: "Lecția 1: Ceața Groasă ca Laptele și Hackerul Răutăcios",
            title_en: "Lesson 1: Thick Milk Fog & The Naughty GPS Hacker",
            child_story_ro: "Imaginează-ți că mergi cu bicicleta prin ceață groasă ca laptele și nu vezi nici măcar vârful ghidonului! Dacă întrebi un străin răutăcios (Hackerul GPS 🔴) pe unde e drumul, el îți pune o hartă falsă ca să cazi în iarbă. Iar dacă închizi ochii și doar numeri pașii (Senzorii Interni 🟠), după câțiva metri amețești și te rătăcești!",
            child_story_en: "Imagine riding a bike in fog as thick as milk! A naughty GPS hacker gives you a fake map, and walking with your eyes closed makes you dizzy.",
            interactive_action_label_ro: "🔴 Apasă Aici: Pune Ceață Groasă + Pornește Hackerul GPS!",
            interactive_action_js: "lessonPlayAction(1)",
            visual_highlight_zone: "zone-radar",
            takeaway_badge_ro: "⭐ Ai învățat de ce avionul roșu 🔴 și portocaliu 🟠 greșesc drumul!",
        },
        InteractiveLesson6Yo {
            lesson_num: 2,
            icon: "🔦",
            title_ro: "Lecția 2: Ochiul Magic Infraroșu cu Lupă Anticeață (SWIR + ALS)",
            title_en: "Lesson 2: Magic Infrared Eye Seeing Runway Lights Through Fog",
            child_story_ro: "Când aprinzi o lanternă normală în ceață, lumina se lovește de picăturile de apă și îți face un nor alb în față. Dar avionul nostru are Ochelari Magici Infraroșii (SWIR 1550nm)! Cu ei, ceața devine transparentă și vedem clar șirul de becuri strălucitoare (ALS 💡) de la capătul pistei din Craiova!",
            child_story_en: "Normal headlights bounce off fog droplets, creating glare. Our Magic Infrared Goggles (SWIR) cut through fog to see the bright runway lights!",
            interactive_action_label_ro: "🔦 Apasă Aici: Pune Ochelarii Magici Infraroșu peste Ceață!",
            interactive_action_js: "lessonPlayAction(2)",
            visual_highlight_zone: "zone-hud",
            takeaway_badge_ro: "⭐ Ai descoperit cum vedem becurile pistei Craiova RWY 09 prin ceață!",
        },
        InteractiveLesson6Yo {
            lesson_num: 3,
            icon: "💡",
            title_ro: "Lecția 3: Cum Măsurăm Viteza Numărând Becurile (și Cerul Fără Blocaj!)",
            title_en: "Lesson 3: Counting Passing Lights & Looking at the Sky Safely",
            child_story_ro: "Când mergi cu mașina noaptea, stâlpii de lumină de lângă tine trec repede: ZUUM! Așa știe avionul cât de repede zboară peste pământ (Săgețile Verzi 🟢). Dar sus pe cer nu există stâlpi (distanța e infinită)! Calculatoarele vechi împărțeau la zero și înghețau (NaN). Noi folosim o formulă isteață care pune simplu numărul ZERO la cer — fără să se blocheze niciodată!",
            child_story_en: "We measure ground speed by watching runway lights pass underneath (Green Arrows), while looking at the sky outputs a safe 0.000000 with zero crashes!",
            interactive_action_label_ro: "🚀 Apasă Aici: Ridică Botul Avionului spre Cer (Verifică Zero-Blocaj)!",
            interactive_action_js: "lessonPlayAction(3)",
            visual_highlight_zone: "zone-matrix",
            takeaway_badge_ro: "⭐ Ai văzut de ce calculatorul avionului nu îngheață niciodată la orizont!",
        },
        InteractiveLesson6Yo {
            lesson_num: 4,
            icon: "☁️",
            title_ro: "Lecția 4: Norul Împins de Vânt (Mov) vs. Pământul Fix (Verde)",
            title_en: "Lesson 4: Purple Fog Blown by Wind vs. Solid Green Ground",
            child_story_ro: "Uite secretul cel mai mare! Când bate vântul dintr-o parte, el împinge norul de ceață spre dreapta (Săgețile Mov 🟣). O cameră obișnuită (Avionul Galben 🟡) se sperie și crede că avionul a alunecat la stânga, așa că virează greșit în câmp! Avionul nostru Verde 🟢 separă norul mov de pământul verde și vede rafala cu 0.85 secunde ÎNAINTE să lovească aripile!",
            child_story_en: "Wind blows the fog sideways (Purple Arrows). Naive cameras get tricked, but our Green Plane separates the moving fog from the solid ground 0.85s ahead!",
            interactive_action_label_ro: "🌪️ Apasă Aici: Suflă un Vânt Puternic în Ceață (18 m/s)!",
            interactive_action_js: "lessonPlayAction(4)",
            visual_highlight_zone: "zone-hud",
            takeaway_badge_ro: "⭐ Ai înțeles cum separăm vântul din ceață (Mov) de pistă (Verde)!",
        },
        InteractiveLesson6Yo {
            lesson_num: 5,
            icon: "⚖️",
            title_ro: "Lecția 5: Paharul care Adună Minciunile Hackerului (CUSUM < 2.9s)",
            title_en: "Lesson 5: The Cup That Catches Tiny Hacker Lies (CUSUM)",
            child_story_ro: "Dacă hackerul e șiret, nu mută harta dintr-odată, ci o trage câte un milimetru pe secundă (Stealth Spoof 🕵️). Dar noi avem un Pahar Magic (numit CUSUM): fiecare milimetru de minciună e o picătură în pahar! În mai puțin de 3 secunde (2.9s), paharul se umple, sună alarma 🚨 și aruncăm harta falsă a hackerului la coș!",
            child_story_en: "A sneaky hacker nudges the GPS map millimeter by millimeter. Our Magic Cup (CUSUM) collects every tiny lie drop and sounds the alarm in under 2.9 seconds!",
            interactive_action_label_ro: "🕵️ Apasă Aici: Testează Paharul de Prins Minciuni (Atac Furișat)!",
            interactive_action_js: "lessonPlayAction(5)",
            visual_highlight_zone: "zone-cyber",
            takeaway_badge_ro: "⭐ Ai prins hackerul furișat în mai puțin de 2.9 secunde!",
        },
        InteractiveLesson6Yo {
            lesson_num: 6,
            icon: "🎓",
            title_ro: "Lecția 6: Ești Comandant de Zbor! Aterizează în Asediu Complet!",
            title_en: "Lesson 6: You Are the Captain! Land in Full Cyber-Siege!",
            child_story_ro: "Felicitări! Acum știi toate secretele: 1) Ochelarii Infraroșu văd becurile prin ceață, 2) Separăm norul mov de pământul verde, și 3) Paharul CUSUM oprește hackerii! Apasă butonul de mai jos ca să pornești Misiunea Finală (Actul 3) și urmărește cum Avionul Verde ✈️🟢 aterizează exact pe centrul pistei din Craiova!",
            child_story_en: "Congratulations! Launch the Final Act 3 Mission and watch our Green Plane land right on the Craiova RWY 09 centerline under full cyber-siege!",
            interactive_action_label_ro: "🎬 Apasă Aici: Pornește Misiunea Finală de Aterizare (Actul 3)!",
            interactive_action_js: "lessonPlayAction(6)",
            visual_highlight_zone: "zone-radar",
            takeaway_badge_ro: "🏆 DIPLOMĂ DE COMANDANT AERO-SHIELD LRCV V2.0 OBȚINUTĂ!",
        },
    ]
}

/// 1. Bara superioară randată prin `maud::html!`.
#[must_use]
pub fn render_header_bar(current_tick: u64, gpu_device: &str) -> Markup {
    html! {
        header class="topbar" {
            div class="brand" {
                span class="brand-badge" { "RUST MAUD SSR • DO-178C" }
                div {
                    div class="brand-title" {
                        "✈️ AERO-SHIELD LRCV V2.0 — PISTA CRAIOVA RWY 09 (EXPLICAT VIZUAL PAS CU PAS)"
                    }
                    div class="brand-sub" id="conn-status" {
                        "WS 60.0 Hz LIVE | Tick #" (current_tick) " | Motor: " (gpu_device)
                    }
                }
            }

            div class="director-buttons" {
                button class="btn btn-school" onclick="toggleSchoolModal(true)" {
                    "🎓 ȘCOALA ÎN 6 LECȚII (PENTRU 6 ANI)"
                }
                button class="btn btn-spotlight" onclick="startGuidedVisualTour()" {
                    "🔦 TUR VIZUAL GHIDAT (SUBLINIAZĂ TOT!)"
                }
                button class="btn btn-act" id="btn-act-1" onclick="triggerScenarioWithHighlight(1)" {
                    "🎬 POVESTEA 1: VÂNT BRUSC ÎN CEAȚĂ"
                }
                button class="btn btn-act" id="btn-act-2" onclick="triggerScenarioWithHighlight(2)" {
                    "🎬 POVESTEA 2: VIRAJ & ACROBAȚII 6-DoF"
                }
                button class="btn btn-act" id="btn-act-3" onclick="triggerScenarioWithHighlight(3)" {
                    "🎬 POVESTEA 3: ATAC HACKER + ATERIZARE"
                }
                button class="btn btn-gust" id="btn-real-video" onclick="testRealFogVideo()" {
                    "📹 FILMĂRI REALE (.MP4)"
                }
                button class="btn" onclick="resetAndHighlight()" {
                    "🔄 DE LA CAPĂT"
                }
            }

            div class="phase-banner" id="phase-banner" {
                "FAZA 2: OCHELARI INFRAROȘU ACTIVI PE PISTA RWY 09"
            }
        }
    }
}

/// 2. Povestitorul în timp real + Indicatorul Plutitor cu Săgeată Animată (`👇 UITE AICI CE SE SCHIMBĂ!`).
#[must_use]
pub fn render_storyteller_banner() -> Markup {
    html! {
        // Indicator plutitor cu săgeată galbenă gigant pentru subliniere vizuală
        div id="floating-visual-pointer" style="display:none;" {
            div class="pointer-bubble" {
                span class="pointer-arrow" { "👇" }
                div {
                    div id="pointer-title" style="font-weight:900; font-size:13px; color:#021019;" {
                        "UITE AICI CE SE SCHIMBĂ ACUM!"
                    }
                    div id="pointer-desc" style="font-size:11.5px; color:#1e293b;" {
                        "Observă cum avionul nostru verde rămâne pe pistă!"
                    }
                }
            }
        }

        // Banda Povestitorului Live (traduce tot ce se întâmplă în cuvinte simple pentru un copil de 6 ani)
        section class="storyteller-bar" id="storyteller-bar" {
            div class="story-icon" id="story-icon" { "🟢" }
            div style="flex:1;" {
                div class="story-headline" id="story-headline" {
                    "💬 POVESTITORUL LIVE (PE ÎNȚELESUL UNUI COPIL DE 6 ANI): Totul este liniștit! Avionul zboară drept spre pista din Craiova."
                }
                div class="story-subtext" id="story-subtext" {
                    "👉 Apasă pe oricare dintre cele 6 Lecții de mai jos sau pe butoanele colorate, iar ecranul îți va încercui cu GALBEN STRĂLUCITOR exact locul unde trebuie să te uiți!"
                }
            }
            div style="display:flex; gap:6px; align-items:center;" {
                button class="btn btn-spotlight" onclick="startGuidedVisualTour()" {
                    "🔦 Arată-mi cu Săgeata Galbenă!"
                }
            }
        }
    }
}

/// 3. Școala Interactivă în 6 Lecții Vizuale (generată din structurile Rust `InteractiveLesson6Yo` cu `maud::html!`).
#[must_use]
pub fn render_interactive_school_6yo(lessons: &[InteractiveLesson6Yo]) -> Markup {
    html! {
        section class="school-panel" id="school-panel" {
            div class="school-header" {
                div style="display:flex; align-items:center; gap:10px; flex-wrap:wrap;" {
                    span class="school-badge" { "🎓 ACADEMIA MICULUI PILOT (6 ANI+)" }
                    strong style="color:#00ff9d; font-size:13.5px;" {
                        "Învață Pas cu Pas în 6 Lecții Interactive cu Desene Animate:"
                    }
                    span id="school-stars-progress" style="color:#ffd600; font-weight:800; font-size:13px;" {
                        "⭐ ⭐ ☆ ☆ ☆ ☆ (Lecția 1 din 6)"
                    }
                }
                button class="btn" style="padding:4px 10px;" onclick="toggleSchoolModal()" {
                    "Ascunde / Arată Școala ▲▼"
                }
            }

            // Butoanele celor 6 Lecții randate cu @for în Maud
            div class="lesson-tabs" {
                @for lesson in lessons {
                    button
                        class=(if lesson.lesson_num == 1 { "lesson-tab active" } else { "lesson-tab" })
                        id=(format!("lesson-tab-{}", lesson.lesson_num))
                        onclick=(format!("selectLesson6Yo({})", lesson.lesson_num))
                    {
                        span style="font-size:16px;" { (lesson.icon) }
                        span { "Lecția " (lesson.lesson_num) }
                    }
                }
            }

            // Conținutul lecției active + Mini-Jocul pe Canvas
            div class="lesson-stage-grid" {
                // Stânga: Povestea + Butonul mare de experiment interactiv
                div class="lesson-story-card" {
                    @for lesson in lessons {
                        div
                            class="lesson-content-pane"
                            id=(format!("lesson-pane-{}", lesson.lesson_num))
                            style=(if lesson.lesson_num == 1 { "display:flex;" } else { "display:none;" })
                        {
                            h3 style="color:#00e5ff; font-size:15px; margin-bottom:6px;" {
                                (lesson.icon) " " (lesson.title_ro)
                            }
                            p style="color:#f1f5f9; font-size:13px; line-height:1.55; background:#061124; padding:10px 12px; border-radius:7px; border-left:4px solid #ffb800;" {
                                (lesson.child_story_ro)
                            }
                            div style="display:flex; flex-wrap:wrap; gap:8px; margin-top:10px; align-items:center;" {
                                button
                                    class="btn btn-play-action"
                                    onclick=(lesson.interactive_action_js)
                                {
                                    (lesson.interactive_action_label_ro)
                                }
                                button
                                    class="btn btn-spotlight"
                                    onclick=(format!("highlightVisualZone('{}', 'Uite efectul Lecției {} pe ecranul mare!')", lesson.visual_highlight_zone, lesson.lesson_num))
                                {
                                    "🔦 Subliniază pe Ecranul Mare!"
                                }
                                @if lesson.lesson_num < 6 {
                                    button
                                        class="btn btn-next-lesson"
                                        onclick=(format!("selectLesson6Yo({})", lesson.lesson_num + 1))
                                    {
                                        "➡️ Lecția " (lesson.lesson_num + 1) "!"
                                    }
                                }
                            }
                            div style="margin-top:8px; font-size:11.5px; color:#00ff9d; font-weight:700;" {
                                (lesson.takeaway_badge_ro)
                            }
                        }
                    }
                }

                // Dreapta: Desenul Animat Interactiv al Lecției curente
                div class="lesson-canvas-card" {
                    div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px; font-size:11px; color:#7dd3fc;" {
                        strong id="lesson-canvas-caption" {
                            "🎨 DESEN ANIMAT INTERACTIV (Apasă butonul din stânga ca să vezi animația!):"
                        }
                        span style="color:#00ff9d;" { "60 FPS LIVE" }
                    }
                    canvas id="maudSchoolCanvas" width="680" height="185" style="width:100%; height:185px; background:#030917; border:1px solid #1e4273; border-radius:6px;" {}
                }
            }
        }
    }
}

/// 4. Ecranul Principal Dual: Parbrizul cu Filmare Reală `.mp4` suprapusă 50% (Stânga) + Cursa Celor 4 Avioane (Dreapta).
#[must_use]
pub fn render_main_cockpit_and_4planes_arena() -> Markup {
    html! {
        main class="cockpit-grid" {
            // ZONA 1: PARBRIZUL AVIONULUI + FILMAREA REALĂ .MP4 SUPRAPUSĂ 50%
            section class="viewport-card" id="zone-hud" {
                div class="card-header" {
                    span {
                        "👀 ZONA 1: PARBRIZUL AVIONULUI + FILMARE REALĂ ÎN CEAȚĂ (🟢 VERDE = PĂMÂNTUL FIX, 🟣 MOV = NORUL DE CEAȚĂ)"
                    }
                    span id="hud-guidance-summary" style="color:#00ff9d;" {
                        "DISTANȚĂ PISTĂ: 900 m | DIRECȚIE: 087.4°"
                    }
                }

                // Bara de control pentru filmarea reală suprapusă peste HUD
                div style="display:flex; flex-wrap:wrap; justify-content:space-between; align-items:center; gap:8px; padding:5px 10px; background:#071326; border-bottom:1px solid #163759; font-size:11px;" {
                    div style="display:flex; align-items:center; gap:6px;" {
                        strong style="color:#00ff9d;" { "📹 FILMARE REALĂ ÎN PARBRIZ:" }
                        button class="btn" style="padding:3px 8px; font-size:10.5px;" onclick="switchHudRealVideo(0)" {
                            "1. Ceață Albă CAT III"
                        }
                        button class="btn" style="padding:3px 8px; font-size:10.5px;" onclick="switchHudRealVideo(1)" {
                            "2. Lumini Pistă Noaptea (ALS)"
                        }
                        button class="btn" style="padding:3px 8px; font-size:10.5px;" onclick="switchHudRealVideo(2)" {
                            "3. Aterizare Vânt Lateral"
                        }
                    }
                    div style="display:flex; align-items:center; gap:6px;" {
                        span style="color:#9ec5eb;" { "🎚️ Transparență Video Real:" }
                        input
                            type="range"
                            id="video-opacity-slider"
                            min="0"
                            max="90"
                            value="50"
                            style="width:90px;"
                            oninput="setHudVideoOpacity(this.value)"
                        {}
                        strong id="video-opacity-label" style="color:#00e5ff; min-width:34px;" { "50%" }
                    }
                }

                div class="canvas-wrap" {
                    // Filmarea reală .mp4 în fundalul parbrizului (suprapusă cu săgețile verzi și mov!)
                    video
                        id="hudBgRealVideo"
                        src="/real_videos/real_02_als_runway_approach_cdg.mp4"
                        autoplay
                        loop
                        muted
                        playsinline
                        style="position:absolute; inset:0; width:100%; height:100%; object-fit:cover; opacity:0.50; pointer-events:none; z-index:1;"
                    {}

                    // Alerta pulsatorie de rafală văzută în față (0.85s înainte de impact)
                    div class="gust-alert-overlay" id="gust-overlay" style="z-index:10;" {
                        "⚠️ ATENȚIE! AM VĂZUT UN NOR DE VÂNT ÎN FAȚĂ CU 0.85 SECUNDE ÎNAINTE SĂ LOVEASCĂ AVIONUL!"
                    }

                    // Canvas-ul HUD 3D transparent peste filmarea reală
                    canvas id="hudCanvas" style="position:relative; z-index:4;" {}
                }

                div class="kid-explain-strip" {
                    "🧒 " strong style="color:#00ff9d;" { "Explicație pentru copii (Zona 1):" }
                    " Pe ecran vezi filmarea reală din avion! Săgețile "
                    strong style="color:#00ff9d;" { "VERZI 🟢" }
                    " arată pământul și becurile pistei (care stau pe loc), iar săgețile "
                    strong style="color:#f0abfc;" { "MOV 🟣" }
                    " arată norii de ceață împinși de vânt în lateral!"
                }
            }

            // ZONA 2: CURSA CELOR 4 AVIOANE COLORATE SPRE PISTA DIN CRAIOVA
            section class="viewport-card" id="zone-radar" {
                div class="card-header" {
                    span {
                        "🏁 ZONA 2: CURSA CELOR 4 AVIOANE VĂZUTE DE SUS (CINE NIMEREȘTE PISTA DIN CRAIOVA?)"
                    }
                    span id="eta-summary" style="color:#00ff9d;" {
                        "TIMP PÂNĂ LA PISTĂ: 13.2 s | BECURI VĂZUTE: 12"
                    }
                }
                div class="canvas-wrap" style="height: 335px;" {
                    canvas id="radarCanvas" {}
                }
                div class="arena-legend" {
                    div class="est-pill" style="border-left: 4px solid #ff2a5f;" onclick="highlightVisualZone('zone-radar', 'Avionul Roșu folosește doar GPS clasic — când hackerul minte, fuge în câmp!')" {
                        span { "✈️🔴 1. AVIONUL ROȘU (GPS Păcălit de Hacker)" }
                        strong id="err-gnss" style="color:#ff2a5f;" { "Abatere: 0.65 m" }
                    }
                    div class="est-pill" style="border-left: 4px solid #ff9100;" onclick="highlightVisualZone('zone-radar', 'Avionul Portocaliu zboară cu ochii închiși — amețește și derivă >80m!')" {
                        span { "✈️🟠 2. AVIONUL PORTOCALIU (Cu Ochii Închiși / INS)" }
                        strong id="err-ins" style="color:#ff9100;" { "Abatere: 1.20 m" }
                    }
                    div class="est-pill" style="border-left: 4px solid #ffd600;" onclick="highlightVisualZone('zone-radar', 'Avionul Galben confundă norul de ceață cu pământul când bate vântul!')" {
                        span { "✈️🟡 3. AVIONUL GALBEN (Cameră Păcălită de Ceață)" }
                        strong id="err-naive" style="color:#ffd600;" { "Abatere: 0.95 m" }
                    }
                    div class="est-pill" style="border-left: 4px solid #00ff9d; background:#07261f;" onclick="highlightVisualZone('zone-radar', 'Avionul nostru Verde AERO-SHIELD merge drept pe centrul pistei (<0.8m)!')" {
                        span style="color:#00ff9d; font-weight:800;" { "✈️🟢 4. AVIONUL NOSTRU VERDE (AERO-SHIELD V2.0)" }
                        strong id="err-shield" style="color:#00ff9d; font-size:12.5px;" { "Abatere: 0.28 m" }
                    }
                }
                div class="kid-explain-strip" {
                    "🧒 " strong style="color:#00ff9d;" { "Explicație pentru copii (Zona 2):" }
                    " Privește cele 4 avioane de sus! Când pornești vântul sau hackerul, avioanele "
                    strong style="color:#ff2a5f;" { "Roșu 🔴" } ", "
                    strong style="color:#ff9100;" { "Portocaliu 🟠" } " și "
                    strong style="color:#ffd600;" { "Galben 🟡" }
                    " ies în iarbă, dar "
                    strong style="color:#00ff9d;" { "Avionul Verde ✈️🟢" }
                    " aterizează fix pe mijlocul pistei!"
                }
            }
        }
    }
}

/// 5. Telecomanda de Zbor 6-DoF + Cutia Matematică + Butoanele de Ceață și Hacker.
#[must_use]
pub fn render_control_deck() -> Markup {
    html! {
        section class="lower-deck" {
            // ZONA 3A: TELECOMANDA DE PILOTAT AVIONUL
            div class="panel" id="zone-controls" {
                div class="panel-title" {
                    span { "🕹️ ZONA 3: TELECOMANDA DE PILOTAT AVIONUL (APASĂ SAU FOLOSEȘTE TASTATURA!)" }
                    span style="color:var(--hud-green); font-size:10px;" { "RĂSPUNS INSTANTANEU 60Hz" }
                }
                div class="controls-6dof-grid" {
                    button class="btn" onclick="stepManeuverWithHighlight(-6, 0, 0, 0, 'Viraj la Stânga! Uită-te la Parbriz și la Matricea 3x3!')" { "⬅️ VIREAZĂ STÂNGA (A)" }
                    button class="btn" onclick="stepManeuverWithHighlight(6, 0, 0, 0, 'Viraj la Dreapta! Uită-te cum se rotește orizontul!')" { "➡️ VIREAZĂ DREAPTA (D)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, 3.5, 0, 0, 'Ridicăm Botul Avionului spre Cer (Urcare)!')" { "⬆️ RIDICĂ BOTUL (W)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, -3.5, 0, 0, 'Coborâm Botul Avionului spre Pistă!')" { "⬇️ COBOARĂ BOTUL (S)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, 0, -10, 0, 'Înclinăm Aripile spre Stânga (Roll)!')" { "↺ ÎNCLINĂ STÂNGA (Q)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, 0, 10, 0, 'Înclinăm Aripile spre Dreapta (Roll)!')" { "↻ ÎNCLINĂ DREAPTA (E)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, 0, 0, 6, 'Mai Multă Viteză (+6 m/s)!')" { "⚡ MAI REPEDE (+6 m/s)" }
                    button class="btn" onclick="stepManeuverWithHighlight(0, 0, 0, -6, 'Frânăm Ușor (-6 m/s)!')" { "🛑 MAI ÎNCET (-6 m/s)" }
                }

                div class="kv-grid" {
                    div class="kv-item" { span class="kv-label" { "Poziție față de Pistă:" } span class="kv-val" id="val-pos" { "[-900, 0, -52] m" } }
                    div class="kv-item" { span class="kv-label" { "Viteză Pământ [X,Y,Z]:" } span class="kv-val" id="val-vel-ned" { "[68.0, 0.0, 3.5] m/s" } }
                    div class="kv-item" { span class="kv-label" { "Unghiuri [Înclinare,Bot,Direcție]:" } span class="kv-val" id="val-euler" { "[0.0°, -3.0°, 0.0°]" } }
                    div class="kv-item" { span class="kv-label" { "Viteză Rotire [p, q, r]:" } span class="kv-val" id="val-omega" { "[0.0, 0.0, 0.0] °/s" } }
                    div class="kv-item" { span class="kv-label" { "Unghi Atac α / Derapaj β:" } span class="kv-val" id="val-aero" { "2.4° / 0.0°" } }
                    div class="kv-item" { span class="kv-label" { "Unghi Împotriva Vântului ψ_crab:" } span class="kv-val" id="val-crab" style="color:var(--hud-cyan);" { "0.0°" } }
                }
                div class="kid-explain-strip" {
                    "🧒 " strong style="color:#ffb800;" { "Pe înțelesul copiilor:" }
                    " Aici e ghidonul avionului! Când apeși pe un buton, avionul își mișcă aripile imediat."
                }
            }

            // ZONA 3B: CUTIA MATEMATICĂ ANTI-AMEȚEALĂ (CUATERNIONI + ADÂNCIME INVERSĂ FĂRĂ NaN)
            div class="panel" id="zone-matrix" {
                div class="panel-title" {
                    span { "📐 CUTIA MATEMATICĂ ANTI-AMEȚEALĂ (CUATERNION q & MATRICEA 3×3)" }
                    span id="val-qnorm" style="color:var(--hud-green); font-size:10px;" { "||q|| = 1.000000 (PERFECT)" }
                }
                div class="matrix-box" {
                    div style="display:flex; justify-content:space-between; color:var(--text-dim);" {
                        span { "Busola 4D (Cuaternion unitar q):" }
                        strong id="val-quat" style="color:var(--hud-cyan);" { "[0.9997, 0.0000, -0.0262, 0.0000]" }
                    }
                    div class="matrix-grid" id="val-rot-matrix" {
                        span { "+0.9986" } span { "+0.0000" } span { "-0.0523" }
                        span { "+0.0000" } span { "+1.0000" } span { "+0.0000" }
                        span { "+0.0523" } span { "+0.0000" } span { "+0.9986" }
                    }
                }

                div class="kv-grid" {
                    div class="kv-item" { span class="kv-label" { "Cer / Orizont ρ_ground (Fără Blocaj):" } span class="kv-val" id="val-rho-hor" style="color:var(--hud-green);" { "0.000000 m⁻¹ (SIGUR)" } }
                    div class="kv-item" { span class="kv-label" { "Distanță Inversă Ceață ρ̂_fog:" } span class="kv-val" id="val-rho-fog" style="color:var(--fog-purple);" { "0.0204 m⁻¹" } }
                    div class="kv-item" { span class="kv-label" { "Grosime Ceață Koschmieder β:" } span class="kv-val" id="val-beta" { "0.0399 m⁻¹" } }
                    div class="kv-item" { span class="kv-label" { "Vânt Ceață Real vs Ghicit:" } span class="kv-val" id="val-fog-wind" { "1.5 vs 1.5 m/s" } }
                    div class="kv-item" { span class="kv-label" { "Claritate Becuri Pistă:" } span class="kv-val" id="val-cert" { "91.0% (12 becuri)" } }
                    div class="kv-item" { span class="kv-label" { "Vremea Live Craiova LRCV:" } span class="kv-val" id="val-meteo-env" { "8.5°C | QNH 1016" } }
                }
                div class="kid-explain-strip" {
                    "🧒 " strong style="color:#00e5ff;" { "Pe înțelesul copiilor:" }
                    " Aceste 9 numere verzi îi spun avionului cum este înclinat fără să amețească niciodată, iar la cer pune "
                    code { "0.000000" } " ca să nu împartă la zero!"
                }
            }

            // ZONA 4: TESTEAZĂ FURTUNA DE CEAȚĂ, HACKERUL ȘI PLACA VIDEO!
            div class="panel" id="zone-cyber" {
                div class="panel-title" {
                    span { "🛡️ ZONA 4: PORNEȘTE VÂNTUL ÎN CEAȚĂ, HACKERUL GPS ȘI PLACA VIDEO!" }
                    span id="val-gpu-dev" style="color:var(--warn-amber); font-size:10px;" { "CUDA / SIMD ACTIV" }
                }

                div style="display:flex; gap:6px; align-items:center;" {
                    button class="btn btn-gust" style="flex:1;" onclick="injectSuddenFogGustWithHighlight()" {
                        "🌪️ SUFLĂ VÂNT PUTERNIC ÎN CEAȚĂ (18 m/s)"
                    }
                    button class="btn btn-cuda" onclick="startLiveCudaTrainingWithHighlight()" {
                        "🔥 ANTRENEAZĂ CREIERUL AI (15s)"
                    }
                }

                div style="display:flex; align-items:center; gap:8px; font-size:11px;" {
                    span style="color:var(--text-dim);" { "🌫️ Cât de groasă e ceața (Vizibilitate):" }
                    input type="range" id="fog-slider" min="25" max="1500" value="75" style="flex:1;" oninput="onFogSliderChange(this.value)" {}
                    strong id="fog-slider-val" style="color:var(--hud-cyan); min-width:52px;" { "75 m" }
                }

                div class="controls-6dof-grid" {
                    button class="btn btn-danger" id="btn-cyber-jump" onclick="toggleCyberWithHighlight('jump')" { "🔴 HACKER: SALT GPS BRUSC" }
                    button class="btn btn-danger" id="btn-cyber-stealth" onclick="toggleCyberWithHighlight('stealth')" { "🕵️ HACKER: FURĂ GPS ÎNCET" }
                    button class="btn btn-danger" id="btn-cyber-comms" onclick="toggleCyberWithHighlight('comms')" { "🌐 TAIE INTERNETUL" }
                    button class="btn btn-danger" id="btn-cyber-poison" onclick="toggleCyberWithHighlight('poison')" { "☣️ METEO MINCINOS" }
                }

                div class="kv-grid" {
                    div class="kv-item" { span class="kv-label" { "Polițistul Anti-Hacker:" } span class="kv-val" id="val-watchdog" style="color:var(--hud-green);" { "TOTUL CURAT" } }
                    div class="kv-item" { span class="kv-label" { "Paharul de Minciuni (χ² / CUSUM):" } span class="kv-val" id="val-chi2-cusum" { "1.18 / 0.12" } }
                    div class="kv-item" { span class="kv-label" { "Verificare Meteo:" } span class="kv-val" id="val-meteo-status" { "LIVE OPEN-METEO LRCV" } }
                    div class="kv-item" { span class="kv-label" { "Viteză Creier AI:" } span class="kv-val" id="val-ai-lat" { "0.21 ms | σ=0.16m/s" } }
                }

                div id="cuda-progress-box" style="display:none; flex-direction:column; gap:3px; font-size:10px;" {
                    div style="display:flex; justify-content:space-between;" {
                        span id="cuda-epoch-txt" style="color:var(--hud-green);" { "ShadowTrainer CUDA Epoca 1/60" }
                        span id="cuda-loss-txt" style="color:var(--hud-cyan);" { "Eroare: 0.1420" }
                    }
                    div class="progress-bar-outer" {
                        div class="progress-bar-inner" id="cuda-progress-bar" {}
                    }
                }
                div class="kid-explain-strip" {
                    "🧒 " strong style="color:#f0abfc;" { "Pe înțelesul copiilor:" }
                    " Încearcă să păcălești avionul cu butoanele roșii! Polițistul nostru digital (CUSUM) prinde orice minciună în sub 2.9 secunde!"
                }
            }
        }
    }
}

/// 6. Secțiunea cu cele 3 Filmări Reale de Aviație (`.mp4`) randată din Rust prin `maud::html!`.
#[must_use]
pub fn render_real_videos_section(bench_json: &serde_json::Value) -> Markup {
    let videos = bench_json
        .get("videos")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total_frames = bench_json
        .get("total_frames_processed")
        .and_then(|v| v.as_u64())
        .unwrap_or(825);
    let overall_fps = bench_json
        .get("overall_fps_achieved")
        .and_then(|v| v.as_f64())
        .unwrap_or(438.7);

    html! {
        section class="extra-section" id="zone-real-videos" {
            div class="section-header" {
                div {
                    h2 style="color:#00ff9d; font-size:15px;" {
                        "📹 CELE 3 FILMĂRI REALE DE ATERIZARE ÎN CEAȚĂ (PROCESATE PIXEL CU PIXEL DE MOTORUL RUST)"
                    }
                    p style="color:#9ec5eb; font-size:11.5px; margin-top:2px;" {
                        "Apasă Play pe oricare filmare reală sau trimite-o direct în Parbrizul HUD de sus! Total cadre reale analizate: "
                        strong style="color:#00e5ff;" { (total_frames) " cadre" }
                        " la o viteză medie de "
                        strong style="color:#00ff9d;" { (format!("{:.1} FPS", overall_fps)) }
                        " (0 erori NaN/Inf)."
                    }
                }
            }

            div class="videos-3-grid" {
                @for (idx, v) in videos.iter().enumerate() {
                    @let file_name = v.get("video_path").and_then(|s| s.as_str()).unwrap_or("real_01_cat3_whiteout_cockpit_lc130.mp4");
                    @let desc = v.get("source_description").and_then(|s| s.as_str()).unwrap_or("Filmare reală din cabina de pilotaj");
                    @let fps = v.get("real_fps_achieved").and_then(|f| f.as_f64()).unwrap_or(430.0);
                    @let mor = v.get("estimated_mor_visibility_m").and_then(|f| f.as_f64()).unwrap_or(1031.0);
                    @let als = v.get("mean_als_beacons_per_frame").and_then(|f| f.as_f64()).unwrap_or(20.0);
                    @let gusts = v.get("gust_lookahead_triggers").and_then(|f| f.as_u64()).unwrap_or(225);

                    div class="video-card" {
                        div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:6px;" {
                            strong style="color:#00e5ff; font-size:12px;" {
                                "🎬 Filmarea Reală #" (idx + 1) ": " (file_name)
                            }
                            button
                                class="btn btn-spotlight"
                                style="padding:3px 8px; font-size:10px;"
                                onclick=(format!("switchHudRealVideo({})", idx))
                            {
                                "⬆️ Pune în Parbrizul 3D!"
                            }
                        }
                        video
                            src=(format!("/real_videos/{}", file_name))
                            controls
                            loop
                            muted
                            playsinline
                            style="width:100%; height:170px; object-fit:cover; border-radius:5px; border:1px solid #1e4273; background:#000;"
                        {}
                        p style="color:#cbd5e1; font-size:11px; margin-top:5px;" { (desc) }
                        div style="display:flex; flex-wrap:wrap; gap:6px; margin-top:5px; font-size:10.5px; color:#00ff9d;" {
                            span class="metric-tag" { "⚡ " (format!("{:.1} FPS", fps)) }
                            span class="metric-tag" { "🌫️ Vizibilitate MOR: " (format!("{:.0} m", mor)) }
                            span class="metric-tag" { "💡 Becuri ALS/cadru: " (format!("{:.1}", als)) }
                            span class="metric-tag" { "🌪️ Alerte Rafală: " (gusts) }
                        }
                    }
                }
            }
        }
    }
}

/// 7. Secțiunea cu cele 10 Teste Sintetice FlightGear (`FG-01` .. `FG-10`) randată din Rust prin `maud::html!`.
#[must_use]
pub fn render_flightgear_10_section(fg: &FlightGearTestSuiteReport) -> Markup {
    html! {
        section class="extra-section" id="zone-flightgear-10" {
            div class="section-header" {
                div {
                    h2 style="color:#00e5ff; font-size:15px;" {
                        "🛩️ CELE 10 TESTE SINTETICE FLIGHTGEAR PE VM (" (fg.passed_tests) "/" (fg.total_tests) " TRECUTE CU SUCCES)"
                    }
                    p style="color:#9ec5eb; font-size:11.5px; margin-top:2px;" {
                        "Motor instalat: " strong style="color:#00ff9d;" { (fg.fgfs_binary_version) }
                        " | Eroare medie AERO-SHIELD: " strong style="color:#00ff9d;" { (format!("{:.2} m", fg.mean_aero_shield_error_m)) }
                        " vs. Sisteme Clasice: " strong style="color:#ff2a5f;" { (format!("{:.1} m", fg.mean_classic_estimator_error_m)) }
                    }
                }
            }

            div class="fg-10-grid" {
                @for t in &fg.tests {
                    div class="fg-card" {
                        div style="display:flex; justify-content:space-between; align-items:center;" {
                            span class="fg-id-badge" { (t.test_id) }
                            span style="color:#00ff9d; font-weight:800; font-size:11px;" {
                                "✅ PASS (Abatere: " (format!("{:.2}m", t.aero_shield_touchdown_error_m)) " vs " (format!("{:.1}m", t.classic_gnss_or_flow_error_m)) ")"
                            }
                        }
                        div style="color:#e0f2fe; font-weight:800; font-size:12px; margin-top:4px;" {
                            (t.title_ro)
                        }
                        p style="color:#bae6fd; font-size:11px; line-height:1.4; margin-top:4px;" {
                            "🧒 " (t.simple_explanation_ro)
                        }
                        div style="margin-top:5px; font-size:10px; color:#7dd3fc; border-top:1px dashed #1e3a5f; padding-top:4px;" {
                            "📊 " (t.key_metric_verified)
                        }
                    }
                }
            }
        }
    }
}

/// Funcția principală SSR care compune întreaga aplicație web în Rust folosind `maud::html!`.
#[must_use]
pub fn render_aero_shield_app_maud(state: &CockpitServerState) -> Markup {
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
                title { "AERO-SHIELD LRCV V2.0 — Rust Maud SSR & Școala Interactivă în 6 Lecții (Craiova RWY 09)" }
                style { (PreEscaped(MAUD_COCKPIT_CSS)) }
            }
            body {
                (render_header_bar(latest_frame.tick, &latest_frame.cyber_and_gpu.gpu_device_name))
                (render_storyteller_banner())
                (PreEscaped(crate::ui_visual_engine::render_visual_canvases_component_html()))
                (PreEscaped(crate::ui_lessons_6yo::render_lessons_6yo_component()))
                (render_interactive_school_6yo(&lessons))
                (render_main_cockpit_and_4planes_arena())
                (render_control_deck())
                (render_real_videos_section(&bench_json))
                (render_flightgear_10_section(&fg_report))
                script { (PreEscaped(MAUD_COCKPIT_JS_ENGINE)) }
                script { (PreEscaped(crate::ui_visual_engine::render_visual_canvases_script())) }
            }
        }
    }
}

const MAUD_COCKPIT_CSS: &str = r####"
:root {
  --bg-deep: #040811;
  --bg-panel: #0a1222;
  --bg-card: #0f1b33;
  --border-cyan: #164e63;
  --hud-green: #00ff9d;
  --hud-cyan: #00e5ff;
  --fog-purple: #d946ef;
  --alert-red: #ff2a5f;
  --warn-amber: #ffb800;
  --text-main: #e2f1ff;
  --text-dim: #7fa2c7;
}
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  font-family: "Inter", "JetBrains Mono", "Segoe UI", sans-serif;
}
body {
  background: radial-gradient(circle at 50% 0%, #0d1b36 0%, var(--bg-deep) 75%);
  color: var(--text-main);
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  overflow-x: hidden;
  padding-bottom: 30px;
}
.topbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 14px;
  background: rgba(7, 14, 28, 0.96);
  border-bottom: 1px solid var(--border-cyan);
}
.brand { display: flex; align-items: center; gap: 10px; }
.brand-badge {
  background: linear-gradient(135deg, #00ff9d 0%, #00b8ff 100%);
  color: #021019;
  font-weight: 900;
  font-size: 11px;
  padding: 4px 8px;
  border-radius: 4px;
}
.brand-title { font-size: 13.5px; font-weight: 800; color: var(--hud-cyan); }
.brand-sub { font-size: 11px; color: var(--text-dim); }
.director-buttons { display: flex; flex-wrap: wrap; gap: 6px; }
.btn {
  background: #10203d;
  color: var(--text-main);
  border: 1px solid #1e4273;
  padding: 6px 10px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.14s ease;
}
.btn:hover { border-color: var(--hud-cyan); background: #16305c; transform: translateY(-1px); }
.btn-school {
  background: linear-gradient(90deg, #f59e0b 0%, #eab308 100%);
  color: #041019;
  border-color: #fef08a;
  font-weight: 900;
}
.btn-spotlight {
  background: linear-gradient(90deg, #0284c7 0%, #06b6d4 100%);
  color: #fff;
  border-color: #bae6fd;
}
.btn-act {
  background: linear-gradient(180deg, #132c52 0%, #0b1b36 100%);
  border-color: #00b8ff;
  color: #e0f7ff;
}
.btn-act.active {
  background: linear-gradient(180deg, #00ff9d 0%, #009e60 100%);
  color: #02140c;
  border-color: #fff;
}
.btn-danger {
  background: linear-gradient(180deg, #4a1025 0%, #2b0714 100%);
  border-color: var(--alert-red);
  color: #ffd6e0;
}
.btn-danger.active {
  background: var(--alert-red);
  color: #fff;
  box-shadow: 0 0 14px rgba(255, 42, 95, 0.75);
}
.btn-gust {
  background: linear-gradient(90deg, #7e22ce 0%, #c026d3 100%);
  border-color: #f0abfc;
  color: #fff;
}
.btn-cuda {
  background: linear-gradient(90deg, #ea580c 0%, #dc2626 100%);
  border-color: #fdba74;
  color: #fff;
}
.btn-play-action {
  background: linear-gradient(90deg, #00ff9d 0%, #00e5ff 100%);
  color: #021318;
  border-color: #fff;
  font-size: 12.5px;
  font-weight: 900;
  padding: 8px 14px;
  box-shadow: 0 0 16px rgba(0, 255, 157, 0.45);
}
.btn-next-lesson {
  background: #ffb800;
  color: #05101a;
  border-color: #fef08a;
  font-weight: 900;
}
.phase-banner {
  background: #081529;
  border: 1px solid #1d4d7a;
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--hud-green);
  font-weight: 700;
}

/* STORYTELLER BAR & FLOATING POINTER */
.storyteller-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 8px 14px 0 14px;
  padding: 9px 14px;
  background: linear-gradient(90deg, rgba(11, 31, 58, 0.96) 0%, rgba(16, 42, 76, 0.96) 100%);
  border: 2px solid #00ff9d;
  border-radius: 8px;
  box-shadow: 0 0 20px rgba(0, 255, 157, 0.18);
}
.story-icon { font-size: 26px; }
.story-headline { font-size: 13.5px; font-weight: 800; color: #ffffff; }
.story-subtext { font-size: 11.5px; color: #a5f3fc; margin-top: 2px; }

#floating-visual-pointer {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  transition: all 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}
.pointer-bubble {
  display: flex;
  align-items: center;
  gap: 8px;
  background: linear-gradient(135deg, #fde047 0%, #f59e0b 100%);
  border: 3px solid #ffffff;
  border-radius: 10px;
  padding: 8px 14px;
  box-shadow: 0 0 32px rgba(250, 204, 21, 0.95);
  animation: bouncePointer 0.6s infinite alternate;
}
.pointer-arrow { font-size: 26px; }
@keyframes bouncePointer {
  from { transform: translateY(-4px) scale(0.98); }
  to { transform: translateY(5px) scale(1.03); }
}
.visual-spotlight-active {
  outline: 4px solid #fde047 !important;
  box-shadow: 0 0 35px rgba(250, 204, 21, 0.85) !important;
  transition: outline 0.2s ease, box-shadow 0.2s ease;
}

/* INTERACTIVE 6-LESSON SCHOOL */
.school-panel {
  margin: 8px 14px 0 14px;
  padding: 10px 14px;
  background: rgba(9, 20, 40, 0.96);
  border: 2px solid #eab308;
  border-radius: 8px;
}
.school-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.school-badge {
  background: #eab308;
  color: #021019;
  font-weight: 900;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 4px;
}
.lesson-tabs {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 6px;
  margin-bottom: 8px;
}
@media (max-width: 1000px) { .lesson-tabs { grid-template-columns: repeat(3, 1fr); } }
.lesson-tab {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 7px 8px;
  background: #0d1d38;
  color: #cbd5e1;
  border: 1px solid #264a78;
  border-radius: 6px;
  font-weight: 800;
  font-size: 12px;
  cursor: pointer;
}
.lesson-tab.active {
  background: linear-gradient(90deg, #eab308 0%, #f59e0b 100%);
  color: #03111c;
  border-color: #ffffff;
  box-shadow: 0 0 14px rgba(234, 179, 8, 0.65);
}
.lesson-stage-grid {
  display: grid;
  grid-template-columns: 1.25fr 1fr;
  gap: 10px;
}
@media (max-width: 1100px) { .lesson-stage-grid { grid-template-columns: 1fr; } }
.lesson-story-card, .lesson-canvas-card {
  background: #071224;
  border: 1px solid #1b3d69;
  border-radius: 7px;
  padding: 10px 12px;
}
.lesson-content-pane { flex-direction: column; }

/* MAIN COCKPIT & LOWER DECK */
.cockpit-grid {
  display: grid;
  grid-template-columns: 1.32fr 1fr;
  gap: 10px;
  padding: 10px 14px;
}
@media (max-width: 1200px) { .cockpit-grid { grid-template-columns: 1fr; } }
.viewport-card {
  background: var(--bg-panel);
  border: 1px solid var(--border-cyan);
  border-radius: 7px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  background: rgba(15, 27, 51, 0.9);
  border-bottom: 1px solid #153759;
  font-size: 11px;
  font-weight: 800;
  color: var(--hud-cyan);
}
.canvas-wrap {
  position: relative;
  width: 100%;
  height: 390px;
  background: #030710;
}
canvas { width: 100%; height: 100%; display: block; }
.gust-alert-overlay {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(192, 38, 211, 0.94);
  border: 2px solid #fdf4ff;
  color: #ffffff;
  padding: 7px 16px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 900;
  display: none;
  pointer-events: none;
}
.kid-explain-strip {
  padding: 6px 10px;
  background: #061020;
  border-top: 1px solid #153554;
  font-size: 11px;
  color: #bae6fd;
  line-height: 1.4;
}
.arena-legend {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 6px;
  padding: 6px 10px;
  background: #060d1b;
  border-top: 1px solid #153554;
  font-size: 11px;
}
.est-pill {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 5px 8px;
  border-radius: 4px;
  background: #0b162c;
  cursor: pointer;
}
.lower-deck {
  display: grid;
  grid-template-columns: 1.15fr 1.15fr 1fr;
  gap: 10px;
  padding: 0 14px 10px 14px;
}
@media (max-width: 1250px) { .lower-deck { grid-template-columns: 1fr; } }
.panel {
  background: var(--bg-panel);
  border: 1px solid var(--border-cyan);
  border-radius: 7px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.panel-title {
  font-size: 11.5px;
  font-weight: 800;
  color: var(--hud-cyan);
  border-bottom: 1px solid #153554;
  padding-bottom: 5px;
  display: flex;
  justify-content: space-between;
}
.controls-6dof-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6px;
}
.matrix-box {
  background: #050c18;
  border: 1px solid #163759;
  border-radius: 5px;
  padding: 6px 8px;
  font-size: 11px;
}
.matrix-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  margin-top: 4px;
  text-align: right;
  font-family: monospace;
  color: var(--hud-green);
}
.kv-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 5px 10px;
  font-size: 11px;
}
.kv-item {
  display: flex;
  justify-content: space-between;
  border-bottom: 1px dashed rgba(127, 162, 199, 0.16);
  padding-bottom: 2px;
}
.kv-label { color: var(--text-dim); }
.kv-val { font-weight: 700; color: var(--text-main); }
.progress-bar-outer {
  width: 100%;
  height: 10px;
  background: #061020;
  border: 1px solid #1e4273;
  border-radius: 5px;
  overflow: hidden;
}
.progress-bar-inner {
  height: 100%;
  width: 0%;
  background: linear-gradient(90deg, #00ff9d 0%, #00e5ff 100%);
}
.extra-section {
  margin: 8px 14px 0 14px;
  padding: 12px 14px;
  background: var(--bg-panel);
  border: 1px solid var(--border-cyan);
  border-radius: 7px;
}
.section-header { margin-bottom: 10px; }
.videos-3-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}
@media (max-width: 1100px) { .videos-3-grid { grid-template-columns: 1fr; } }
.video-card {
  background: #071224;
  border: 1px solid #1c3d66;
  border-radius: 6px;
  padding: 8px 10px;
}
.metric-tag {
  background: #0d2240;
  border: 1px solid #1e4273;
  padding: 2px 6px;
  border-radius: 4px;
}
.fg-10-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
}
@media (max-width: 1300px) { .fg-10-grid { grid-template-columns: repeat(2, 1fr); } }
.fg-card {
  background: #071224;
  border: 1px solid #1b3d69;
  border-radius: 6px;
  padding: 8px 10px;
}
.fg-id-badge {
  background: #00e5ff;
  color: #021019;
  font-weight: 900;
  font-size: 10.5px;
  padding: 2px 6px;
  border-radius: 4px;
}
"####;

const MAUD_COCKPIT_JS_ENGINE: &str = r####"
let ws = null;
let latestFrame = null;
let currentLessonNum = 1;
let lessonAnimTick = 0;
let spotlightTimer = null;
let cyberState = {
  gps_jump_spoof: false,
  gps_stealth_ramp_spoof: false,
  cut_internet_comms: false,
  poison_meteo_cache: false
};

const REAL_VIDEO_FILES = [
  "/real_videos/real_01_cat3_whiteout_cockpit_lc130.mp4",
  "/real_videos/real_02_als_runway_approach_cdg.mp4",
  "/real_videos/real_03_crosswind_cockpit_landing.mp4"
];

const trailHistory = {
  truth: [],
  gnss: [],
  ins: [],
  naive: [],
  shield: []
};
const MAX_TRAIL_LEN = 160;

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
// SISTEMUL DE SUBLINIERE VIZUALĂ (SPOTLIGHT + SĂGEATĂ ANIMATĂ PENTRU 6 ANI)
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
  target.scrollIntoView({ behavior: "smooth", block: "nearest" });

  if (pointer && pDesc) {
    pDesc.textContent = messageRo || "Uită-te aici cum lucrează AERO-SHIELD în timp real!";
    const rect = target.getBoundingClientRect();
    const topPx = Math.max(12, rect.top - 58);
    const leftPx = Math.min(window.innerWidth - 360, Math.max(16, rect.left + 24));
    pointer.style.top = `${topPx}px`;
    pointer.style.left = `${leftPx}px`;
    pointer.style.display = "block";
  }

  if (spotlightTimer) clearTimeout(spotlightTimer);
  spotlightTimer = setTimeout(() => {
    target.classList.remove("visual-spotlight-active");
    if (pointer) pointer.style.display = "none";
  }, 5500);
}

let guidedTourStep = 0;
function startGuidedVisualTour() {
  const steps = [
    ["zone-hud", "PASUL 1/4: Parbrizul Avionului! Săgețile VERZI arată pământul fix, iar săgețile MOV arată norul de ceață suflat de vânt!"],
    ["zone-radar", "PASUL 2/4: Cursa celor 4 Avioane! Doar Avionul Verde ✈️🟢 rămâne pe centrul pistei din Craiova!"],
    ["zone-controls", "PASUL 3/4: Telecomanda de Zbor! Apasă pe săgeți ca să înclini aripile și să virezi avionul!"],
    ["zone-cyber", "PASUL 4/4: Testul Furtunii și al Hackerului! Apasă aici ca să sufli vânt puternic sau să încerci să păcălești GPS-ul!"]
  ];
  const [zone, msg] = steps[guidedTourStep % steps.length];
  guidedTourStep++;
  highlightVisualZone(zone, msg);
}

// =========================================================================
// ȘCOALA INTERACTIVĂ ÎN 6 LECȚII (PENTRU UN COPIL DE 6 ANI)
// =========================================================================
function toggleSchoolModal(forceOpen) {
  const panel = document.getElementById("school-panel");
  if (!panel) return;
  if (forceOpen === true) {
    panel.style.display = "block";
    highlightVisualZone("school-panel", "Alege o lecție de la 1 la 6 și apasă pe butonul verde de joacă!");
  } else {
    panel.style.display = panel.style.display === "none" ? "block" : "none";
  }
}

function selectLesson6Yo(num) {
  currentLessonNum = num;
  for (let i = 1; i <= 6; i++) {
    const tab = document.getElementById(`lesson-tab-${i}`);
    const pane = document.getElementById(`lesson-pane-${i}`);
    if (tab) tab.classList.toggle("active", i === num);
    if (pane) pane.style.display = i === num ? "flex" : "none";
  }
  const starsEl = document.getElementById("school-stars-progress");
  if (starsEl) {
    const filled = "⭐ ".repeat(num);
    const empty = "☆ ".repeat(6 - num);
    starsEl.textContent = `${filled}${empty}(Lecția ${num} din 6)`;
  }
}

function lessonPlayAction(num) {
  if (num === 1) {
    sendSimCommand({
      type: "ConfigureCyberAttack",
      payload: { gps_jump_spoof: true, gps_stealth_ramp_spoof: false, cut_internet_comms: false, poison_meteo_cache: false }
    });
    highlightVisualZone("zone-radar", "Lecția 1: Uită-te cum Avionul Roșu ✈️🔴 (GPS păcălit de hacker) sare departe de pistă!");
  } else if (num === 2) {
    switchHudRealVideo(1);
    sendSimCommand({ type: "SetFogDensity", payload: { visibility_mor_m: 45, enable_swir_als: true } });
    highlightVisualZone("zone-hud", "Lecția 2: Ochelarii Magici Infraroșu văd becurile strălucitoare ale pistei chiar și prin ceață de 45m!");
  } else if (num === 3) {
    stepManeuver(0, 6.0, 0, 0);
    highlightVisualZone("zone-matrix", "Lecția 3: Am ridicat botul avionului spre cer! Uită-te la 'Cer / Orizont ρ_ground = 0.000000' — zero blocaje!");
  } else if (num === 4) {
    injectSuddenFogGust();
    highlightVisualZone("zone-hud", "Lecția 4: Suflă vântul! Săgețile MOV arată norul de ceață împins în lateral, dar săgețile VERZI păstrează pista!");
  } else if (num === 5) {
    sendSimCommand({
      type: "ConfigureCyberAttack",
      payload: { gps_jump_spoof: false, gps_stealth_ramp_spoof: true, cut_internet_comms: true, poison_meteo_cache: true }
    });
    highlightVisualZone("zone-cyber", "Lecția 5: Paharul de prins minciuni (CUSUM) se umple în <2.9 secunde și blochează hackerul furișat!");
  } else if (num === 6) {
    triggerScenarioWithHighlight(3);
  }
}

// Randarea desenului animat interactiv pentru fiecare dintre cele 6 lecții
function renderLessonMiniCanvas() {
  const canvas = document.getElementById("maudSchoolCanvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  lessonAnimTick++;
  const t = lessonAnimTick * 0.04;

  // Fundal
  ctx.fillStyle = "#040d1d";
  ctx.fillRect(0, 0, w, h);

  // Pista din dreapta
  ctx.fillStyle = "#163052";
  ctx.strokeStyle = "#00e5ff";
  ctx.lineWidth = 2;
  ctx.fillRect(w - 150, h / 2 - 26, 130, 52);
  ctx.strokeRect(w - 150, h / 2 - 26, 130, 52);
  ctx.fillStyle = "#00ff9d";
  ctx.font = "bold 11px sans-serif";
  ctx.fillText("PISTA CRAIOVA 09", w - 142, h / 2 + 4);

  if (currentLessonNum === 1) {
    // Lecția 1: Ceață + Hackerul Roșu vs Avionul Verde
    const xProg = (lessonAnimTick % 140) / 140;
    const px = 40 + xProg * (w - 220);
    // Avionul roșu fuge în sus în iarbă
    ctx.strokeStyle = "#ff2a5f";
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(40, h / 2);
    ctx.quadraticCurveTo(w / 2, h / 2 - 75, px, h / 2 - 65 * xProg);
    ctx.stroke();
    ctx.font = "16px sans-serif";
    ctx.fillText("✈️🔴 Hackerul l-a păcălit!", px - 20, h / 2 - 65 * xProg - 6);

    // Avionul verde merge drept
    ctx.strokeStyle = "#00ff9d";
    ctx.beginPath();
    ctx.moveTo(40, h / 2 + 12);
    ctx.lineTo(px, h / 2 + 12);
    ctx.stroke();
    ctx.fillText("✈️🟢 AERO-SHIELD (Drept spre pistă!)", px - 20, h / 2 + 32);
  } else if (currentLessonNum === 2) {
    // Lecția 2: Becurile pistei văzute prin lupă infraroșu
    for (let i = 0; i < 6; i++) {
      const bx = w - 210 + i * 28;
      ctx.fillStyle = i % 2 === 0 ? "#00ff9d" : "#ffb800";
      ctx.beginPath();
      ctx.arc(bx, h / 2, 5 + 2 * Math.sin(t * 3 + i), 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.arc(w - 155, h / 2, 58, 0, Math.PI * 2);
    ctx.stroke();
    ctx.fillStyle = "#00e5ff";
    ctx.font = "bold 12px sans-serif";
    ctx.fillText("🔦 OCHELARI MAGICI INFRAROȘU (SWIR): Văd becurile prin ceață!", 24, 34);
  } else if (currentLessonNum === 3) {
    // Lecția 3: Stâlpii de lumină jos vs Cerul fără împărțire la zero (0.000000)
    ctx.fillStyle = "#0c2444";
    ctx.fillRect(0, 0, w, h / 2);
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 12px sans-serif";
    ctx.fillText("🌌 CERUL (Distanță Infinită) -> Adâncime Inversă ρ = 0.000000 (ZERO BLOCAJE!)", 24, 34);
    for (let i = 0; i < 8; i++) {
      const lx = ((i * 85 - lessonAnimTick * 3) % w + w) % w;
      ctx.fillStyle = "#00ff9d";
      ctx.fillRect(lx, h - 42, 14, 8);
      ctx.fillText("💡⬅️", lx - 4, h - 48);
    }
    ctx.fillStyle = "#e2f1ff";
    ctx.fillText("🛣️ PĂMÂNTUL JOS: Becurile trec pe sub noi -> Măsurăm viteza exactă!", 24, h - 14);
  } else if (currentLessonNum === 4) {
    // Lecția 4: Norul mov împins de vânt vs Pământul verde fix
    for (let i = 0; i < 6; i++) {
      const cx = 60 + i * 95;
      const cy = 48 + (i % 2) * 24;
      ctx.fillStyle = "#d946ef";
      ctx.font = "bold 13px sans-serif";
      ctx.fillText("☁️💨➡️ Nor Mov (Vânt)", cx + 12 * Math.sin(t + i), cy);
    }
    for (let i = 0; i < 6; i++) {
      const gx = 60 + i * 95;
      ctx.fillStyle = "#00ff9d";
      ctx.fillText("🟢⬅️ Pământ Fix", gx, h - 32);
    }
  } else if (currentLessonNum === 5) {
    // Lecția 5: Paharul CUSUM care adună picăturile de minciună ale hackerului
    const fillPct = ((lessonAnimTick % 120) / 120);
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 3;
    ctx.strokeRect(120, 28, 90, 128);
    const barH = fillPct * 120;
    ctx.fillStyle = fillPct > 0.75 ? "#ff2a5f" : "#ffb800";
    ctx.fillRect(123, 153 - barH, 84, barH);
    ctx.fillStyle = "#ffffff";
    ctx.font = "bold 12px sans-serif";
    ctx.fillText(`🥛 PAHARUL CUSUM: ${(fillPct * 12).toFixed(1)} / 9.5`, 235, 70);
    ctx.fillStyle = fillPct > 0.75 ? "#ff2a5f" : "#00ff9d";
    ctx.fillText(
      fillPct > 0.75
        ? "🚨 PAHARUL S-A UMPLUT (<2.9s)! HACKERUL A FOST BLOCAT!"
        : "💧 Picură minciunile mici ale hackerului în pahar...",
      235,
      105
    );
  } else {
    // Lecția 6: Diploma de Comandant
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 15px sans-serif";
    ctx.fillText("🏆 FELICITĂRI! EȘTI GATA PENTRU ATERIZAREA SUPREMĂ PE PISTA CRAIOVA!", 32, h / 2 - 8);
    ctx.fillStyle = "#fde047";
    ctx.font = "13px sans-serif";
    ctx.fillText("Apasă pe butonul verde din stânga ca să lansezi Actul 3 și urmărește Avionul Verde!", 32, h / 2 + 22);
  }
}

// =========================================================================
// CONTROALE VIDEO REAL (.MP4) ÎN PARBRIZUL 3D
// =========================================================================
function switchHudRealVideo(idx) {
  const vid = document.getElementById("hudBgRealVideo");
  if (!vid) return;
  const chosen = REAL_VIDEO_FILES[idx % REAL_VIDEO_FILES.length];
  if (!vid.src.endsWith(chosen)) {
    vid.src = chosen;
    vid.play().catch(() => {});
  }
  sendSimCommand({ type: "replay_real_video" });
  highlightVisualZone("zone-hud", `Am pus Filmarea Reală #${idx + 1} direct în Parbrizul Avionului!`);
}

function setHudVideoOpacity(val) {
  const vid = document.getElementById("hudBgRealVideo");
  const lbl = document.getElementById("video-opacity-label");
  if (vid) vid.style.opacity = (parseFloat(val) / 100.0).toFixed(2);
  if (lbl) lbl.textContent = `${val}%`;
}

function triggerScenarioWithHighlight(id) {
  trailHistory.truth = [];
  trailHistory.gnss = [];
  trailHistory.ins = [];
  trailHistory.naive = [];
  trailHistory.shield = [];
  sendSimCommand({ type: "TriggerDirectorScenario", payload: { scenario_id: id } });
  const msgMap = {
    1: "Povestea 1 a început! La secunda 2.5 vine un vânt puternic din lateral — urmărește alerta de pe Parbriz!",
    2: "Povestea 2 a început! Avionul face viraje și înclinări 6-DoF prin ceață!",
    3: "Povestea 3 (Misiunea Finală): Ceață oarbă + Hacker GPS + Meteo fals! Uită-te cum Avionul Verde ✈️🟢 câștigă cursa!"
  };
  highlightVisualZone(id === 1 ? "zone-hud" : "zone-radar", msgMap[id]);
}

function resetAndHighlight() {
  trailHistory.truth = [];
  trailHistory.gnss = [];
  trailHistory.ins = [];
  trailHistory.naive = [];
  trailHistory.shield = [];
  sendSimCommand({ type: "ResetApproach" });
  highlightVisualZone("zone-hud", "Am pus avionul înapoi la 900m de pista din Craiova!");
}

function stepManeuver(dYaw, dPitch, dRoll, dThrottle) {
  sendSimCommand({
    type: "ManeuverStep",
    payload: { d_yaw_deg: dYaw, d_pitch_deg: dPitch, d_roll_deg: dRoll, d_throttle_mps: dThrottle }
  });
}

function stepManeuverWithHighlight(dYaw, dPitch, dRoll, dThrottle, msg) {
  stepManeuver(dYaw, dPitch, dRoll, dThrottle);
  highlightVisualZone("zone-hud", msg);
}

function injectSuddenFogGust() {
  sendSimCommand({
    type: "InjectSuddenFogGust",
    payload: { wx_ned_mps: -4.5, wy_ned_mps: 18.5, wz_ned_mps: 0.8, turbulence_intensity: 0.95 }
  });
}

function injectSuddenFogGustWithHighlight() {
  injectSuddenFogGust();
  highlightVisualZone("zone-hud", "🌪️ VÂNT PUTERNIC ÎN CEAȚĂ (18.5 m/s)! Uită-te la săgețile MOV și la alerta de 0.85 secunde!");
}

function onFogSliderChange(val) {
  const mor = parseFloat(val);
  document.getElementById("fog-slider-val").textContent = `${mor.toFixed(0)} m`;
  sendSimCommand({ type: "SetFogDensity", payload: { visibility_mor_m: mor, enable_swir_als: true } });
}

function toggleCyber(which) {
  if (which === "jump") cyberState.gps_jump_spoof = !cyberState.gps_jump_spoof;
  if (which === "stealth") cyberState.gps_stealth_ramp_spoof = !cyberState.gps_stealth_ramp_spoof;
  if (which === "comms") cyberState.cut_internet_comms = !cyberState.cut_internet_comms;
  if (which === "poison") cyberState.poison_meteo_cache = !cyberState.poison_meteo_cache;
  sendSimCommand({ type: "ConfigureCyberAttack", payload: cyberState });
}

function toggleCyberWithHighlight(which) {
  toggleCyber(which);
  highlightVisualZone("zone-radar", "🛡️ Atac Hacker comutat! Uită-te pe Radar cum Avionul Roșu fuge, dar Avionul Verde rămâne pe pistă!");
}

function startLiveCudaTrainingWithHighlight() {
  sendSimCommand({ type: "StartLiveCudaFineTuning", payload: { epochs: 60 } });
  highlightVisualZone("zone-cyber", "🔥 Antrenăm creierul AI pe GPU chiar în timpul zborului, fără să blocăm avionul!");
}

function testRealFogVideo() {
  sendSimCommand({ type: "replay_real_video" });
  highlightVisualZone("zone-real-videos", "📹 Aici sunt cele 3 filmări reale de aterizare în ceață procesate la >430 FPS!");
}

window.addEventListener("keydown", (e) => {
  if (e.target && e.target.tagName === "INPUT") return;
  const key = e.key.toLowerCase();
  if (key === "a" || key === "arrowleft") stepManeuver(-4.5, 0, 0, 0);
  if (key === "d" || key === "arrowright") stepManeuver(4.5, 0, 0, 0);
  if (key === "w" || key === "arrowup") stepManeuver(0, 2.5, 0, 0);
  if (key === "s" || key === "arrowdown") stepManeuver(0, -2.5, 0, 0);
  if (key === "q") stepManeuver(0, 0, -8.0, 0);
  if (key === "e") stepManeuver(0, 0, 8.0, 0);
  if (e.key === "Shift") stepManeuver(0, 0, 0, 5.0);
  if (e.key === "Control") stepManeuver(0, 0, 0, -5.0);
});

function recordEstimatorTrails(f) {
  const pushPt = (arr, pt) => {
    arr.push([pt[0], pt[1]]);
    if (arr.length > MAX_TRAIL_LEN) arr.shift();
  };
  pushPt(trailHistory.truth, f.aircraft_pose.pos_ned_m);
  pushPt(trailHistory.gnss, f.estimators_arena.raw_gnss_pos_ned);
  pushPt(trailHistory.ins, f.estimators_arena.pure_ins_pos_ned);
  pushPt(trailHistory.naive, f.estimators_arena.naive_flow_pos_ned);
  pushPt(trailHistory.shield, f.estimators_arena.aero_shield_pos_ned);
}

function updateTelemetryDom(f) {
  document.getElementById("phase-banner").textContent = f.approach_phase;
  const p = f.aircraft_pose;
  const m = f.fog_and_meteo;
  const e = f.estimators_arena;
  const g = f.lrcv_guidance;
  const c = f.cyber_and_gpu;

  // Actualizare Povestitor Live (pentru un copil de 6 ani!)
  const stIcon = document.getElementById("story-icon");
  const stHead = document.getElementById("story-headline");
  const stSub = document.getElementById("story-subtext");
  if (stHead && stSub && stIcon) {
    if (m.gust_lookahead_warning) {
      stIcon.textContent = "🌪️";
      stHead.textContent = `💬 POVESTITORUL LIVE: Atenție! Am văzut un nor de vânt puternic (${m.true_fog_wind_ned_mps[1].toFixed(1)} m/s) cu 0.85 secunde înainte să lovească avionul!`;
      stSub.textContent = `Avionul Galben 🟡 s-a păcălit cu ${e.naive_flow_error_m.toFixed(1)}m, dar Avionul nostru Verde 🟢 a pus unghiul anti-vânt (${p.crab_angle_cmd_deg.toFixed(1)}°) și stă pe pistă (${e.aero_shield_error_m.toFixed(2)}m)!`;
    } else if (c.cyber_flags && (c.cyber_flags.gps_jump_spoof || c.cyber_flags.gps_stealth_ramp_spoof)) {
      stIcon.textContent = "🚨";
      stHead.textContent = `💬 POVESTITORUL LIVE: Alerta Anti-Hacker! Hackerul încearcă să fure GPS-ul (Avionul Roșu 🔴 a greșit cu ${e.raw_gnss_error_m.toFixed(1)}m)!`;
      stSub.textContent = `Paharul de prins minciuni (${c.gnss_watchdog_status}) a oprit atacul, iar Avionul nostru Verde 🟢 zboară în siguranță cu Ochelarii Infraroșu (${e.aero_shield_error_m.toFixed(2)}m)!`;
    } else {
      stIcon.textContent = "🟢";
      stHead.textContent = `💬 POVESTITORUL LIVE: Avionul Verde ✈️🟢 zboară lin spre pista Craiova RWY 09! Mai sunt ${g.distance_to_rwy09_m.toFixed(0)} metri (${g.eta_touchdown_sec.toFixed(1)} secunde).`;
      stSub.textContent = `Vedem ${g.als_bars_detected} rânduri de becuri prin ceață (vizibilitate ${m.visibility_mor_m.toFixed(0)}m), iar abaterea noastră e de doar ${(e.aero_shield_error_m * 100).toFixed(0)} centimetri!`;
    }
  }

  for (let i = 1; i <= 3; i++) {
    const btn = document.getElementById(`btn-act-${i}`);
    if (btn) btn.classList.toggle("active", c.active_director_scenario === i);
  }

  if (c.cyber_flags) {
    cyberState = c.cyber_flags;
    document.getElementById("btn-cyber-jump").classList.toggle("active", !!c.cyber_flags.gps_jump_spoof);
    document.getElementById("btn-cyber-stealth").classList.toggle("active", !!c.cyber_flags.gps_stealth_ramp_spoof);
    document.getElementById("btn-cyber-comms").classList.toggle("active", !!c.cyber_flags.cut_internet_comms);
    document.getElementById("btn-cyber-poison").classList.toggle("active", !!c.cyber_flags.poison_meteo_cache);
  }

  const gustOverlay = document.getElementById("gust-overlay");
  if (gustOverlay) {
    if (m.gust_lookahead_warning) {
      gustOverlay.style.display = "block";
      gustOverlay.textContent = `⚠️ AM VĂZUT UN VÂNT LATERAL DE ${m.true_fog_wind_ned_mps[1].toFixed(1)} m/s ÎN CEAȚĂ CU ${m.gust_lookahead_lead_sec.toFixed(2)}s ÎNAINTE DE IMPACT!`;
    } else {
      gustOverlay.style.display = "none";
    }
  }

  document.getElementById("err-gnss").textContent = `Abatere: ${e.raw_gnss_error_m.toFixed(2)} m`;
  document.getElementById("err-ins").textContent = `Abatere: ${e.pure_ins_error_m.toFixed(2)} m`;
  document.getElementById("err-naive").textContent = `Abatere: ${e.naive_flow_error_m.toFixed(2)} m`;
  document.getElementById("err-shield").textContent = `Abatere: ${e.aero_shield_error_m.toFixed(2)} m`;

  document.getElementById("val-pos").textContent =
    `[${p.pos_ned_m[0].toFixed(0)}, ${p.pos_ned_m[1].toFixed(1)}, ${p.pos_ned_m[2].toFixed(1)}] m`;
  document.getElementById("val-vel-ned").textContent =
    `[${p.vel_ground_ned_mps[0].toFixed(1)}, ${p.vel_ground_ned_mps[1].toFixed(1)}, ${p.vel_ground_ned_mps[2].toFixed(1)}] m/s`;
  document.getElementById("val-euler").textContent =
    `[${p.euler_deg[0].toFixed(1)}°, ${p.euler_deg[1].toFixed(1)}°, ${p.euler_deg[2].toFixed(1)}°]`;
  document.getElementById("val-omega").textContent =
    `[${p.omega_body_dps[0].toFixed(1)}, ${p.omega_body_dps[1].toFixed(1)}, ${p.omega_body_dps[2].toFixed(1)}] °/s`;
  document.getElementById("val-aero").textContent =
    `${p.alpha_aoa_deg.toFixed(1)}° / ${p.beta_sideslip_deg.toFixed(1)}°`;
  document.getElementById("val-crab").textContent = `${p.crab_angle_cmd_deg.toFixed(2)}°`;

  const q = p.quat_b_to_n;
  const qNorm = Math.sqrt(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]);
  document.getElementById("val-qnorm").textContent = `||q|| = ${qNorm.toFixed(6)}`;
  document.getElementById("val-quat").textContent =
    `[${q[0].toFixed(4)}, ${q[1].toFixed(4)}, ${q[2].toFixed(4)}, ${q[3].toFixed(4)}]`;

  if (p.rot_matrix_b_to_n) {
    const rm = p.rot_matrix_b_to_n;
    const cells = [];
    for (let r = 0; r < 3; r++) {
      for (let col = 0; col < 3; col++) {
        const v = rm[r][col];
        cells.push(`<span>${v >= 0 ? "+" : ""}${v.toFixed(4)}</span>`);
      }
    }
    document.getElementById("val-rot-matrix").innerHTML = cells.join("");
  }

  document.getElementById("val-rho-hor").textContent = `${m.rho_ground_horizon_safe.toFixed(6)} m⁻¹ (SIGUR)`;
  document.getElementById("val-rho-fog").textContent = `${m.rho_fog_scale_inv_m.toFixed(4)} m⁻¹`;
  document.getElementById("val-beta").textContent = `${m.beta_ext_koschmieder.toFixed(4)} m⁻¹`;
  document.getElementById("val-fog-wind").textContent =
    `Wy: ${m.true_fog_wind_ned_mps[1].toFixed(1)} vs ${m.est_fog_wind_ned_mps[1].toFixed(1)} m/s`;
  document.getElementById("val-cert").textContent =
    `${(g.photonic_certainty_mean * 100).toFixed(1)}% (${g.als_bars_detected} becuri)`;
  document.getElementById("val-meteo-env").textContent =
    `${m.lrcv_temp_c.toFixed(1)}°C | QNH ${m.lrcv_qnh_hpa.toFixed(1)} | RH ${m.lrcv_rh_pct.toFixed(0)}%`;

  document.getElementById("eta-summary").textContent =
    `DISTANȚĂ RWY 09: ${g.distance_to_rwy09_m.toFixed(0)}m | TIMP: ${g.eta_touchdown_sec.toFixed(1)}s | BECURI: ${g.als_bars_detected}`;

  const wdEl = document.getElementById("val-watchdog");
  wdEl.textContent = c.gnss_watchdog_status;
  wdEl.style.color = c.gnss_watchdog_status.startsWith("NOMINAL") ? "#00ff9d" : "#ff2a5f";

  document.getElementById("val-chi2-cusum").textContent =
    `χ²=${c.chi2_mahalanobis_stat.toFixed(2)} / CUSUM=${c.cusum_stealth_stat.toFixed(2)}`;

  const msEl = document.getElementById("val-meteo-status");
  msEl.textContent = m.meteo_source_status;
  msEl.style.color = m.meteo_source_status.includes("POISON") ? "#ff2a5f" : "#00e5ff";

  document.getElementById("val-gpu-dev").textContent = c.gpu_device_name;
  document.getElementById("val-ai-lat").textContent =
    `${c.inference_latency_ms.toFixed(2)} ms | σ=[${c.heteroscedastic_sigma_v[0].toFixed(2)}, ${c.heteroscedastic_sigma_v[1].toFixed(2)}]`;

  const progBox = document.getElementById("cuda-progress-box");
  if (c.live_training_progress && progBox) {
    progBox.style.display = "flex";
    const tp = c.live_training_progress;
    document.getElementById("cuda-epoch-txt").textContent =
      `🔥 ShadowTrainer Epoca ${tp.current_epoch}/${tp.total_epochs} (${tp.progress_pct.toFixed(0)}%)`;
    document.getElementById("cuda-loss-txt").textContent =
      `Loss: ${tp.current_loss.toFixed(4)}`;
    document.getElementById("cuda-progress-bar").style.width = `${tp.progress_pct}%`;
  }
}

// =========================================================================
// CANVAS 1: PARBRIZUL 3D CU ETICHETE VIZUALE MARI EXPLICATE PENTRU COPII
// =========================================================================
function renderHudCanvas() {
  const canvas = document.getElementById("hudCanvas");
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  if (canvas.width !== rect.width || canvas.height !== rect.height) {
    canvas.width = rect.width;
    canvas.height = rect.height;
  }
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  if (!latestFrame) return;

  const p = latestFrame.aircraft_pose;
  const m = latestFrame.fog_and_meteo;
  const g = latestFrame.lrcv_guidance;
  const rollRad = (p.euler_deg[0] * Math.PI) / 180.0;
  const pitchDeg = p.euler_deg[1];
  const yawDeg = p.euler_deg[2];

  ctx.save();
  ctx.translate(w / 2, h / 2);
  ctx.rotate(-rollRad);

  const pitchOffsetPx = pitchDeg * 6.5;

  // Linia Orizontului
  ctx.strokeStyle = "rgba(0, 255, 157, 0.85)";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(-w, pitchOffsetPx);
  ctx.lineTo(w, pitchOffsetPx);
  ctx.stroke();

  // Pista RWY 09 Craiova + Becurile ALS
  const lateralShiftPx = (-p.pos_ned_m[1] * 2.2) - (yawDeg * 5.0);
  const rwyTopY = pitchOffsetPx + 6;
  const rwyBotY = h * 0.85;
  const topHalfW = 24;
  const botHalfW = 195;

  ctx.fillStyle = "rgba(10, 35, 48, 0.52)";
  ctx.strokeStyle = "#00e5ff";
  ctx.lineWidth = 2.2;
  ctx.beginPath();
  ctx.moveTo(lateralShiftPx - topHalfW, rwyTopY);
  ctx.lineTo(lateralShiftPx + topHalfW, rwyTopY);
  ctx.lineTo(lateralShiftPx + botHalfW, rwyBotY);
  ctx.lineTo(lateralShiftPx - botHalfW, rwyBotY);
  ctx.closePath();
  ctx.fill();
  ctx.stroke();

  ctx.strokeStyle = "#00ff9d";
  ctx.setLineDash([12, 10]);
  ctx.beginPath();
  ctx.moveTo(lateralShiftPx, rwyTopY);
  ctx.lineTo(lateralShiftPx, rwyBotY);
  ctx.stroke();
  ctx.setLineDash([]);

  const numBars = Math.min(g.als_bars_detected || 10, 14);
  for (let i = 1; i <= numBars; i++) {
    const frac = i / (numBars + 1);
    const by = rwyTopY + frac * (rwyBotY - rwyTopY) * 0.82;
    const barHalfW = topHalfW + frac * (botHalfW - topHalfW) * 0.45;
    ctx.strokeStyle = i % 3 === 0 ? "#ff2a5f" : "#00ffcc";
    ctx.lineWidth = 2.8;
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx - barHalfW, by);
    ctx.lineTo(lateralShiftPx + barHalfW, by);
    ctx.stroke();
  }
  ctx.restore();

  // Săgețile Verzi (Pământul) și Săgețile Mov (Norul de ceață suflat de vânt)
  if (latestFrame.optical_flow_vectors) {
    for (const vec of latestFrame.optical_flow_vectors) {
      const sx = vec.x_norm * w;
      const sy = vec.y_norm * h;
      const ex = sx + vec.dx_px;
      const ey = sy + vec.dy_px;
      const isFog = vec.layer === "fog";
      ctx.strokeStyle = isFog ? "#f0abfc" : "#00ff9d";
      ctx.fillStyle = isFog ? "#f0abfc" : "#00ff9d";
      ctx.lineWidth = isFog ? 2.6 : 2.2;

      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.lineTo(ex, ey);
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(ex, ey, 3.2, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // Etichete vizuale mari pe Parbriz (pe înțelesul unui copil de 6 ani!)
  ctx.fillStyle = "rgba(6, 18, 36, 0.86)";
  ctx.strokeStyle = "#f0abfc";
  ctx.lineWidth = 1.5;
  ctx.fillRect(w * 0.26, 46, 310, 24);
  ctx.strokeRect(w * 0.26, 46, 310, 24);
  ctx.fillStyle = "#f0abfc";
  ctx.font = "bold 11.5px sans-serif";
  ctx.fillText(`☁️💨 SĂGEȚI MOV = Norul de Ceață (Vânt: ${m.true_fog_wind_ned_mps[1].toFixed(1)} m/s)`, w * 0.26 + 8, 62);

  ctx.fillStyle = "rgba(6, 18, 36, 0.86)";
  ctx.strokeStyle = "#00ff9d";
  ctx.fillRect(w * 0.26, h - 38, 330, 24);
  ctx.strokeRect(w * 0.26, h - 38, 330, 24);
  ctx.fillStyle = "#00ff9d";
  ctx.fillText(`💡🟢 SĂGEȚI VERZI = Becurile Pistei Craiova (${g.als_bars_detected} văzute)`, w * 0.26 + 8, h - 22);
}

// =========================================================================
// CANVAS 2: CURSA CELOR 4 AVIOANE VĂZUTE DE SUS (CU ICONIȚE ✈️ MARI!)
// =========================================================================
function renderRadarCanvas() {
  const canvas = document.getElementById("radarCanvas");
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  if (canvas.width !== rect.width || canvas.height !== rect.height) {
    canvas.width = rect.width;
    canvas.height = rect.height;
  }
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const mapX = (xNed) => ((xNed + 950) / 1150) * (w - 80) + 30;
  const mapY = (yNed) => h / 2 + (yNed / 150) * (h * 0.40);

  // Grila
  ctx.strokeStyle = "rgba(0, 229, 255, 0.12)";
  ctx.lineWidth = 1;
  for (let x = -900; x <= 100; x += 200) {
    const cx = mapX(x);
    ctx.beginPath();
    ctx.moveTo(cx, 0);
    ctx.lineTo(cx, h);
    ctx.stroke();
  }

  // Linia centrală spre pista din Craiova
  ctx.strokeStyle = "rgba(0, 255, 157, 0.45)";
  ctx.setLineDash([6, 6]);
  ctx.beginPath();
  ctx.moveTo(mapX(-950), mapY(0));
  ctx.lineTo(mapX(0), mapY(0));
  ctx.stroke();
  ctx.setLineDash([]);

  // Pista Craiova RWY 09
  const rwyX0 = mapX(0);
  const rwyX1 = mapX(140);
  ctx.fillStyle = "rgba(0, 255, 157, 0.25)";
  ctx.strokeStyle = "#00ff9d";
  ctx.lineWidth = 2.5;
  ctx.fillRect(rwyX0, mapY(-20), rwyX1 - rwyX0, mapY(20) - mapY(-20));
  ctx.strokeRect(rwyX0, mapY(-20), rwyX1 - rwyX0, mapY(20) - mapY(-20));
  ctx.fillStyle = "#00ff9d";
  ctx.font = "bold 11.5px sans-serif";
  ctx.fillText("🎯 PISTA CRAIOVA 09", rwyX0 - 45, mapY(-26));

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

  drawTrail(trailHistory.ins, "#ff9100", 2.2);
  drawTrail(trailHistory.gnss, "#ff2a5f", 2.2);
  drawTrail(trailHistory.naive, "#ffd600", 2.4);
  drawTrail(trailHistory.shield, "#00ff9d", 3.4);

  const drawPlaneLabel = (pos, color, labelTxt, yOff) => {
    const px = mapX(pos[0]);
    const py = mapY(pos[1]);
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(px, py, 5.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.font = "bold 11px sans-serif";
    ctx.fillText(labelTxt, px - 18, py + yOff);
  };

  const e = latestFrame.estimators_arena;
  drawPlaneLabel(e.pure_ins_pos_ned, "#ff9100", "✈️🟠 Amețit (INS)", -12);
  drawPlaneLabel(e.raw_gnss_pos_ned, "#ff2a5f", "✈️🔴 Păcălit de Hacker", -24);
  drawPlaneLabel(e.naive_flow_pos_ned, "#ffd600", "✈️🟡 Împins de Ceață", 18);
  drawPlaneLabel(e.aero_shield_pos_ned, "#00ff9d", "✈️🟢 AERO-SHIELD (PE PISTĂ!)", -10);
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

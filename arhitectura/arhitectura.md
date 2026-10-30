# Master Architecture Blueprint (100% RUST) — AERO-SHIELD LRCV V2.0 (`Bulletproof Edition`)
**Eveniment**: Craiova Hackathon V2.0 (30 octombrie – 1 noiembrie 2026, Aeroportul Internațional Craiova `LRCV`)  
**Tehnologie Principală**: **100% RUST (`Edition 2021 / 2024`) + NVIDIA CUDA (`candle-core` / `cudarc`)**  
**Ciclu de Validare Arhitecturală**: `idea-generator-v2` (Propunere Rust) $\rightleftarrows$ `idea-validator-v2` (Audit Sever Rust/CUDA: 6 Critici de Sistem) $\rightleftarrows$ `idea-generator-v2` (Arhitectură Finală Blindată)

---

## 1. De Ce 100% RUST pentru AERO-SHIELD LRCV V2.0? (Avantajul Competitiv la Jurizare)

În aviația modernă și în sistemele critice de apărare anti-jamming/anti-spoofing (Track-urile **`AUTOPILOT`** și **`SECURE THE GATE`** + provocarea **`CONTROL TOWER`**), un *Garbage Collector* imprevizibil (Python/JS/Java) sau o eroare de memorie în C++ este inacceptabilă în timpul aterizării de precizie în ceață CAT IIIc.

Implementarea **100% în Rust** a întregului sistem **AERO-SHIELD LRCV V2.0** aduce 4 garanții inginerești majore în fața mentorilor (Pre-Pitch Top 10) și a juriului (Final Pitch):
1. **Determinism Real-Time ($60\text{ Hz}$ Vision / $400\text{ Hz}$ IMU, *Zero-Heap Allocation*)**: Filtrul Kalman pe 22 de stări (`SMatrix<f64, 22, 22>`) și cinematica pe cuaternioni unitari (`UnitQuaternion<f64>`) din `nalgebra` sunt alocate **100% pe stivă (stack)**, executând un pas complet de fuziune în **$< 45\,\mu\text{s}$** fără nicio pauză de alocare dinamică.
2. **Siguranță la Compilare pe Sisteme de Coordonate (*Phantom-Typed Reference Frames*)**: Folosind *Zero-Sized Phantom Types* în Rust (`FramedVec3<BodyFrame>`, `FramedVec3<NedFrameLrcv>`, `FramedVec3<CameraFrame>`), este **imposibil la compilare** să aduni din greșeală viteza în reperul aeronavei $\mathcal{F}_b$ cu viteza ceții în reperul pistei `LRCV` $\mathcal{F}_n$ fără aplicarea explicită a cuaternionului de rotație $\mathbf{R}_b^n(\mathbf{q})$.
3. **Antrenare și Inferență GPU CUDA Nativă în Rust Pur (`candle-core` + `candle-nn` + `safetensors`)**: Fără dependențe greoaie de Python sau zeci de GB de `libtorch` C++, rețeaua `FogOpticalNavNetV2` rulează și se antrenează direct pe GPU-ul NVIDIA (L4/T4) din Google Cloud, cu încărcare instantanee prin memory-map (`mmap`) în **$< 15\text{ ms}$** și arhitectură *Double-Buffered Shadow Trainer* (`ArcSwap<FrozenFogNavModel>`).
4. **Concurență *Lock-Free* Garantată de Borrow Checker**: Bucla fizică de $60\text{ Hz}$ nu așteaptă niciodată după rețea (`Open-Meteo API`), după antrenarea pe GPU sau după un client WebSocket lent, folosind pointeri atomici `ArcSwap` și conflație de stare `tokio::sync::watch`.

---

## 2. Structura Cargo Workspace-ului și a Documentației din `arhitectura/`

```text
Craiova Hackathon 2026/
├── arhitectura/
│   ├── arhitectura.md                  # [Acest Document] Master System Architecture (100% Rust)
│   ├── 01_aero_math_6dof.md            # Crate 1: Cuaternioni, Inverse-Depth, Aero-Gust & ES-EKF 22-Stări
│   ├── 02_aero_ai_cuda.md              # Crate 2: FogOpticalNavNet V2 în Candle CUDA, ShadowTrainer & PINN Loss
│   ├── 03_aero_vision_fog.md           # Crate 3: Rust SIMD + Rayon CLAHE, Certitudine Fotonică (λ1,λ2) & SWIR ALS
│   ├── 04_aero_meteo_cyber.md          # Crate 4: Async Open-Meteo LRCV, ArcSwap, Dual-Layer χ²+CUSUM & Sanity Gate
│   └── 05_aero_cockpit_server.md       # Crate 5: Thread RT 60Hz, Axum Watch WebSockets, 4-Estimators & Director Mode
├── planificare/                        # Documentația de concept, model matematic și validare juriu
├── Cargo.toml                          # Workspace Root cu profiluri de compilare ultra-rapide
└── crates/
    ├── aero-math-6dof/                 # Nucleul matematic 6-DoF & 4-Estimator Arena (nalgebra, simba, statrs)
    ├── aero-ai-cuda/                   # Deep Learning pe GPU CUDA (candle-core, candle-nn, safetensors)
    ├── aero-vision-fog/                # Computer Vision 100% Rust pur (rayon, image, wide SIMD)
    ├── aero-meteo-cyber/               # Meteo LRCV & Cyber-Defense Sandbox (tokio, reqwest, arc-swap)
    └── aero-cockpit-server/            # Executabilul principal: Simulator RT 60Hz + Server Axum + Cockpit 3D/HUD
```

### Diagrama Arhitecturală de Concurență *Lock-Free* în Rust

```mermaid
flowchart TB
    subgraph OS_RT ["Thread OS Dedicat: aero-60hz-rt-loop (16.66 ms deterministic)"]
        CMD_RX["Drenare Non-Blocking: crossbeam_channel::try_recv()"]
        PHYS["📦 aero-math-6dof\n6-DoF Quaternion Kinematics + Aero-Gust Coupling\n+ Inverse-Depth ρ_ground (Zero NaN at Horizon)"]
        VIS["📦 aero-vision-fog\nPure Rust Rayon SIMD: SWIR+VIS CLAHE\n3-Frame Structure Tensor (λ1,λ2) & ALS Halo Mask"]
        INF["📦 aero-ai-cuda (Inference)\nLock-Free ArcSwap::load(&FrozenFogNavModel)\nCUDA Forward Pass (< 1.8 ms)"]
        EKF["📦 aero-math-6dof (4-Estimator Arena)\n1. Raw GNSS | 2. Pure INS | 3. Naive Flow\n4. AERO-SHIELD V2 (22-State Joseph ES-EKF + χ²/CUSUM)"]
        CMD_RX --> PHYS --> VIS --> INF --> EKF
    end

    subgraph SHADOW_GPU ["Thread OS Background: aero-cuda-shadow-trainer"]
        TRAIN["📦 aero-ai-cuda (ShadowTrainer)\nVarMap Clonat + AdamW + Heteroscedastic PINN Loss\nBatch=16 cu 2ms yield -> ArcSwap::store(NewFrozenModel)"]
    end

    subgraph TOKIO_ASYNC ["Runtime Async Tokio: I/O, Meteo & WebSockets"]
        METEO["📦 aero-meteo-cyber\nAsync Open-Meteo & METAR LRCV (1.5s timeout)\n+ Optical-Aerodynamic Meteo Sanity Gate"]
        AXUM["🚀 aero-cockpit-server (Axum HTTP/WS :8080)\ntokio::sync::watch (Zero-Lag Latest Frame Conflation)\n3D LRCV Cockpit HUD + 3 One-Click Director Modes"]
    end

    TRAIN -. "Atomic Pointer Swap (ArcSwap)" .-> INF
    METEO -. "Atomic Snapshot (ArcSwap)" .-> EKF
    EKF == "watch::Sender::send(Arc<TelemetryFrameV2>)" ==> AXUM
    AXUM == "crossbeam::Sender::try_send(SimCommand)" ==> CMD_RX
```

---

## 3. Stack-ul Oficial de Librării (Crates) Rust Selectat și Validat

| Subsistem / Crate Intern | Crate-uri Externe Oficiale (`Cargo.toml`) | Rol Tehnic și Motivul Selecției în Bucla Red-Team |
| :--- | :--- | :--- |
| **`aero-math-6dof`** | `nalgebra = "0.33"`, `simba = "0.9"`, `statrs = "0.17"`, `rand = "0.8"`, `rand_chacha = "0.3"` | Algebră liniară alocată pe stivă (`SMatrix<f64, 22, 22>`, `UnitQuaternion<f64>`), decompoziții `Cholesky` și `SVD` fără panică, distribuții statistice $\chi^2$ și generator determinist `ChaCha8Rng`. |
| **`aero-ai-cuda`** | `candle-core = "0.8"` (cu feature `cuda`), `candle-nn = "0.8"`, `safetensors = "0.4"`, `arc-swap = "1.7"` | Framework ML 100% Rust dezvoltat de HuggingFace: suportă kernel-uri CUDA native, `AdamW`, `GRU`, `Conv2d`, încărcare instantanee `mmap` din `.safetensors` și fallback automat pe CPU dacă nu există GPU. |
| **`aero-vision-fog`** | `rayon = "1.10"`, `image = "0.25"`, `bytemuck = "1.19"` | **Zero dependențe C/C++ externe (`kornia-rs`/`opencv` eliminate la auditul Red-Team!)**. Implementare 100% Rust pur paralelizată cu `rayon` pentru CLAHE, Sobel $3\times 3$, valorile proprii $(\lambda_1, \lambda_2)$ și mascarea haloului ALS în $< 0.15\text{ ms}$. |
| **`aero-meteo-cyber`** | `tokio = { version = "1.41", features = ["full"] }`, `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`, `serde = "1.0"`, `serde_json = "1.0"`, `arc-swap = "1.7"` | Ingestie asincronă non-blocking pentru Open-Meteo Aviation API pe coordonatele `LRCV` (`44.3181° N, 23.8886° E`) cu `rustls` (fără dependențe OpenSSL C), cache local și watchdog dublu $\chi^2$ + CUSUM. |
| **`aero-cockpit-server`** | `axum = { version = "0.7", features = ["ws"] }`, `tower-http = { version = "0.6", features = ["fs", "cors"] }`, `crossbeam-channel = "0.5"`, `tracing = "0.1"` | Server Web și WebSocket de înaltă frecvență ($60\text{ Hz}$) cu conflație de stare `tokio::sync::watch` (zero lag pe rețea) și servire directă a interfeței 3D/HUD. |

---

## 4. Cele 6 Remedieri Critice de Sistem Rust/CUDA (`idea-validator-v2` $\rightleftarrows$ `idea-generator-v2`)

Pentru a garanta că proiectul compilează din prima pe VM-ul Google Cloud GPU și rulează fără niciun blocaj în timpul jurizării, arhitectura impune următoarele **6 reguli de fier**:

1. **Separarea `ArcSwap<FrozenFogNavModel>` de `ShadowTrainer` (Zero Contenție CUDA la $60\text{ Hz}$)**:
   - În `candle-nn`, `VarMap` conține un `Mutex`. De aceea, firul de $60\text{ Hz}$ nu folosește niciodată un `VarMap` mutabil, ci citește *lock-free* un `FrozenFogNavModel` (tensori detașați). Când juriul apasă `LIVE CUDA FINE-TUNING`, un thread separat (`ShadowTrainer`) antrenează pe o copie cu mini-batch-uri `B = 16` și micro-pauze de $2\text{ ms}$, făcând swap atomic (`active_model.store(...)`) la finalul fiecărei epoci.
2. **Arhitectură `FogSpatialAttentionPool2d + candle_nn::GRU` Nativă + Thread OS Dedicat**:
   - Deoarece `candle-nn` oferă `candle_nn::rnn::GRU` (1D), dar nu `ConvGRU2d`, rețeaua comprimă hărțile spațiale `[B, 64, 32, 32]` prin **Attention-Weighted Spatial Pooling** în doi descriptori denși (`f_ground [B, 64]` și `f_fog [B, 64]`), pe care îi concatenează cu vectorul cinematic 6-DoF `[B, 18]` și îi trece prin `candle_nn::GRU` nativ (`146 -> 128`).
   - Bucla de $60\text{ Hz}$ rulează pe un `std::thread` nativ dedicat (`aero-60hz-rt-loop`), nu într-un task `tokio::spawn`, prevenind blocarea serverului web.
3. **Configurație Cargo *Zero-Friction* (Compilare Incrementală în $< 2\text{ s}$)**:
   - Eliminarea oricăror legături C++ (`bindgen`/`cmake`) prin scrierea kernel-urilor vizuale în Rust pur + `rayon`.
   - În `Cargo.toml` rădăcină, `[profile.dev.package."*"] opt-level = 3` compilează o singură dată dependențele la viteză maximă, iar `[profile.release] lto = "thin"` oferă compilare release rapidă fără epuizarea RAM-ului.
4. **Forma Stabilizată Joseph + Solver `Cholesky` cu Fallback `SVD` în `nalgebra`**:
   - În `es_ekf_22state.rs`, este interzis apelul `.try_inverse().unwrap()`. Matricea de inovație $\mathbf{S}$ primește regularizare Tikhonov ($\mathbf{S}_{reg} = \frac{1}{2}(\mathbf{S}+\mathbf{S}^T) + 10^{-6}\mathbf{I}$) și este rezolvată cu `s_reg.cholesky()`, având fallback automat pe `s_reg.svd(true, true).solve(..., 1e-9)`. Covarianța $\mathbf{P}_{22\times 22}$ este actualizată prin **Forma Joseph** și simetrizată explicit la fiecare cadru.
5. **Conflație Zero-Lag prin `tokio::sync::watch` pe WebSocket-ul `axum`**:
   - Telemetria $60\text{ Hz}$ nu folosește cozi `mpsc` care pot acumula întârziere pe Wi-Fi, ci `tokio::sync::watch::channel`, livrând mereu browserului **exclusiv cel mai recent cadru calculat**.
6. **Determinism Reproductibil cu `ChaCha8Rng` și Gardă Globală `.is_finite()`**:
   - Cele 3 butoane *One-Click Director Mode* folosesc seed-ul criptografic fix `0x4C52_4356_2026` (`"LRCV 2026"`), garantând traiectorii identice la fiecare prezentare pe scenă, iar toate intrările și ieșirile trec prin `sanitize_f64(val, min, max, fallback)`.

---

## 5. Navigație Rapidă către Specificațiile Detaliate ale Crate-urilor

1. **[`01_aero_math_6dof.md`](./01_aero_math_6dof.md)** — *Phantom-Typed Frames*, Cuaternioni Unitari, Pre-integrare IMU, Inversa Adâncimii $\rho_{ground}$, Cuplajul Aerodinamic la Rafale (*Optical Gust Lookahead* + *Dutch Roll* + *Crab Angle*), ES-EKF 22-Stări și Arena celor 4 Estimatoare.
2. **[`02_aero_ai_cuda.md`](./02_aero_ai_cuda.md)** — Arhitectura `FogOpticalNavNetV2` în `candle-core`/`candle-nn`, `FrozenFogNavModel` vs. `ShadowTrainer`, funcția de cost Heteroscedastic Gaussian NLL + Inverse-Depth PINN și încărcarea `.safetensors`.
3. **[`03_aero_vision_fog.md`](./03_aero_vision_fog.md)** — Pipeline-ul vizual 100% Rust pur (`rayon` + SIMD): fuziunea Vizibil + SWIR/LWIR, CLAHE adaptiv, Tensorul de Structură pe 3 Cadre $(\lambda_1, \lambda_2)$, Covarianța Anizotropă Fisher (*Aperture Problem*) și Suprimarea Haloului ALS RWY 09.
4. **[`04_aero_meteo_cyber.md`](./04_aero_meteo_cyber.md)** — Ingestia asincronă `Open-Meteo` / `METAR LRCV` (`1.5s timeout`), partajarea `ArcSwap`, `Dual-Layer GNSS Watchdog` ($\chi^2$ + Vector CUSUM contra *Stealth Slow-Ramp Spoofing*) și `Optical-Aerodynamic Meteo Sanity Gate`.
5. **[`05_aero_cockpit_server.md`](./05_aero_cockpit_server.md)** — Bucla Real-Time $60\text{ Hz}$, protocolul WebSocket `axum` cu `tokio::sync::watch`, structura `TelemetryFrameV2`, controalele manuale 6-DoF și cele 3 Scenarii „One-Click Director Mode".

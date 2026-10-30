# Planificare Generală — AERO-SHIELD LRCV V2.0: FogVision 6-DoF (`Bulletproof Edition`)
**Eveniment**: Craiova Hackathon V2.0 (30 octombrie – 1 noiembrie 2026)  
**Locație**: Aeroportul Internațional Craiova (`ICAO: LRCV` / `IATA: CRA` — Pista RWY 09/27, `44.3181° N, 23.8886° E`)  
**Status**: **Faza de Concept, Audit Red-Team & Re-Validare V2.0 Finalizată (`100 / 100` — `APROBAT PENTRU CODARE`)**

---

## 1. Rezumatul Conceptului V2.0 (Elevator Pitch)
**AERO-SHIELD LRCV V2.0 (`FogVision 6-DoF — Bulletproof Edition`)** este un sistem **software-first** de navigație optică-inerțială 6-DoF pe cuaternioni și simulare tactică 3D/HUD în timp real, proiectat pentru a ghida în siguranță aeronavele și dronele spre pista Aeroportului Internațional Craiova (`RWY 09`) în condiții extreme:
1. **Bruiaj și Spoofing GPS Avansat (`Jump Spoofing` + `Stealth Slow-Ramp Spoofing`)**, detectate în $<100\text{ ms}$ / $<3.2\text{ s}$ printr-un watchdog dublu ($\chi^2$ Mahalanobis + Vector CUSUM pe filtre federate),
2. **Tăierea conexiunii la internet (`Comms Blackout`) & Otrăvirea prealabilă a datelor meteo (`Pre-Cut Meteo Cache Poisoning`)**, contracarate prin preluare live `Open-Meteo` / `METAR LRCV`, cache local și validare fizică încrucișată (*Optical-Aerodynamic Meteo Sanity Gate*),
3. **Ceață densă și plafon de nori joși (`CAT IIIc Zero-Visibility`)**, în care sistemul folosește **inversa adâncimii ($\rho_{ground}, \rho_{fog}$)**, **variația norilor și a ceții din jur**, **fuziunea Vizibil + SWIR/LWIR cu suprimarea haloului balizelor ALS** și **decuplarea completă a manevrelor 6-DoF ale avionului (dreapta, stânga, sus, jos, roll, accelerare, decelerare)** de modificările bruște ale ceții (**Optical Gust Lookahead** cu $0.6\text{–}1.1\text{ s}$ înainte de impact + **Aero-Gust Coupling**).

---

## 2. Încadrarea în Track-urile Oficiale Craiova Hackathon V2.0
* **Track Principal**: 🤖 **`AUTOPILOT`** (*AI, Automation & Intelligent Systems*)
* **Track Secundar**: 🛡️ **`SECURE THE GATE`** (*Cybersecurity & Defence* — executat 100% în mediu simulat/sandbox conform Art. 21 din Regulament)
* **Provocare Oficială**: 🗼 **`CONTROL TOWER`** (*Craiova Hackathon Official Challenge* — vedere duală Cockpit Pilot HUD + Turn de Control Digital Twin `LRCV` cu 4 estimatoare paralele în timp real)

---

## 3. Structura Documentației Actualizate V2.0 din Folderul `planificare/`

| Fișier | Conținut și Rol (Actualizat după Bucla Red-Team V2.0) |
| :--- | :--- |
| [`planificare.md`](./planificare.md) | Documentul central de planificare V2.0, obiective, resurse Google Cloud GPU și stadiul proiectului. |
| [`critica_si_imbunatatiri.md`](./critica_si_imbunatatiri.md) | **Auditul Red-Team Nemilos (`idea-validator-v2` $\rightleftarrows$ `idea-generator-v2`)**: Cele **12 vulnerabilități critice** identificate și soluțiile matematice & inginerești integrate în V2.0. |
| [`model_matematic_6dof_ceata.md`](./model_matematic_6dof_ceata.md) | Modelul matematic V2.0 complet: cinematică 6-DoF pe cuaternioni unitari $\mathbf{q}_b^n$, pre-integrare IMU $\Delta t_{sync}$, flux optic pe **inversa adâncimii $\rho_{ground}$** (fără `NaN` la orizont), dublă ancorare a scării $\hat{\rho}_{fog}$ și **cuplajul aerodinamic la rafale bruște (`Optical Gust Lookahead` + `Crab Angle`)**. |
| [`arhitectura_ai_gpu_si_date_meteo.md`](./arhitectura_ai_gpu_si_date_meteo.md) | Arhitectura mini-modelului `FogOpticalNavNet V2` (PyTorch CUDA cu **Heteroscedastic Uncertainty Loss**, pondere fotonică $\lambda_1, \lambda_2$, canal SWIR + suprimare halou ALS), fuziunea `Open-Meteo` / `METAR LRCV`, protecția anti-poisoning și **Pâlnia de Apropiere în 3 Faze**. |
| [`validare_si_scenariu_demo_juriu.md`](./validare_si_scenariu_demo_juriu.md) | Fișa de re-validare oficială V2.0 (`100/100`), benchmark-ul vizual cu **4 estimatoare simultane**, strategia pentru *Pre-Pitch (Mentori Top 10)* și scenariul cu **3 Butoane „One-Click Director Mode"** pentru *Final Pitch (Juriu)*. |

---

## 4. Resurse Cloud Alocate & Strategie VM cu GPU pentru Compilare și Training (Google Cloud Platform)
* **Cont & Proiect Activ**: `ai-lo-511715` (*AI-LO*), regiunea `europe-west1`
* **Buget Disponibil**: **\$159.92 USD** (*Google Developer Program Premium Credits*)
* **Cote Hardware Deblocate**:
  * **1 × GPU Dedicat (`GPUS_ALL_REGIONS = 1`)**: NVIDIA L4 (24 GB VRAM) / NVIDIA T4 (16 GB VRAM).
  * **32 × vCPU (`CPUS_ALL_REGIONS = 32`)**: Compute Engine.
* **Regulă Operațională Obligatorie — Utilizare VM cu GPU pentru Compilare (Rust) și Training (AI)**:
  1. **Zero Încărcare pe Laptopul Local (`i5-8250U` / `8 GB RAM`)**: Laptopul local este utilizat exclusiv ca interfață ușoară (editor și browser pentru afișarea HUD-ului / Command Center-ului la $60\text{ Hz}$). Nu se rulează compilări grele sau antrenări pe mașina locală.
  2. **Compilare 100% pe VM-ul cu GPU din Google Cloud**: Întregul Cargo Workspace Rust ([`arhitectura/arhitectura.md`](../arhitectura/arhitectura.md) — `candle-core` cu `features = ["cuda"]`, `nalgebra`, `rayon`, `tokio`, `axum`) se compilează direct pe mașina virtuală din Compute Engine (`cargo build --release --features cuda`), beneficiind de CPU multi-core rapid, RAM dedicat și toolkit-ul NVIDIA CUDA preinstalat.
  3. **Training & Fine-Tuning Live pe GPU-ul din VM**: Generarea datelor sintetice 6-DoF cu ceață și antrenarea mini-modelului `FogOpticalNavNet V2` (precum și butonul `LIVE CUDA FINE-TUNING` pentru sesiunea cu mentorii de la *Pre-Pitch*) rulează integral în VRAM-ul plăcii NVIDIA de pe VM, salvând greutățile în format `safetensors`.

---

## 5. Echipa de Subagenți & Orchestrare
1. **`idea-generator-v2`**: Arhitect de concepte, modele matematice 6-DoF și rețele neurale fizic-informate.
2. **`idea-validator-v2`**: Auditor Red-Team nemilos și evaluator oficial pe grila Craiova Hackathon V2.0 (Art. 18 & Art. 21).
3. **`code-generator-v2`**: Implementator software-first în **Rust + CUDA** (activat doar la comanda explicită a utilizatorului).
4. **`code-validator-v2`**: Validator de cod și *Demo Readiness* pe VM-ul cu GPU (activat în faza de construcție).
5. **`hackathon-orchestrator-v2`**: Coordonator central al buclelor iterative.

---

## 6. Planul de Execuție (Etape)
- [x] **Etapa 1 — Definire Subagenți & Calibrare Regulament Craiova Hackathon V2.0**
- [x] **Etapa 2 — Configurare Google Cloud CLI (`gcloud`), Credite (\$159.92) și Cotă GPU (`Value: 1`)**
- [x] **Etapa 3 — Definire Concept Inițial V1 (`AERO-SHIELD LRCV: FogVision 6-DoF`)**
- [x] **Etapa 4 — Buclă Severă de Audit Red-Team (`idea-validator-v2` $\rightleftarrows$ `idea-generator-v2`) & Upgrade la V2.0 (`Bulletproof Edition` — 12 Vulnerabilități Rezolvate & Documentate în `planificare/`)**
- [x] **Etapa 4.5 — Proiectare & Validare Arhitectură 100% Rust + CUDA în [`arhitectura/`](../arhitectura/arhitectura.md)**
- [ ] **Etapa 5 — Pornire VM cu GPU în Google Cloud (pentru Compilare Rust + Training Model Vizual) & Implementare Software V2.0 (`code-generator-v2`)** *(În așteptarea confirmării utilizatorului)*
- [ ] **Etapa 6 — Validare Cod pe VM (`code-validator-v2`) & Pregătire Demo Live pentru Pre-Pitch și Final Pitch**

# Modelul Matematic 6-DoF V2.0 (Pe Cuaternioni & Inversa Adâncimii) & Dinamica Ceții
**Sub-modul**: Cinematica Aeronavei Fără Singularități Cuplată cu Variația Optică și Aerodinamică a Ceții  
**Proiect**: **AERO-SHIELD LRCV V2.0 (`FogVision 6-DoF — Bulletproof Edition`)**

---

## 1. Obiectivul Fizic și Matematic (Versiunea V2.0)
Sistemul rezolvă simultan și fără nicio singularitate numerică următoarele probleme fundamentale în timpul zborului fără GPS și fără internet spre pista **RWY 09 a Aeroportului Internațional Craiova (`LRCV` — `44.3181° N, 23.8886° E`, altitudine `191.0 m`, cap compas `087°`)**:
1. **Detectarea anticipativă (`Optical Gust Lookahead`, cu $0.6\text{–}1.1\text{ s}$ înainte de impact) și la impact a modificării bruște a ceții ($\Delta\mathbf{w}_{fog}$)** — atât când avionul zboară rectiliniu, cât și când rafala îi induce un șoc aerodinamic (*Dutch Roll* / derapaj $\beta_{sideslip}$).
2. **Actualizarea completă și instantanee a tuturor calculelor atunci când avionul execută manevre 6-DoF arbitrare** — **ia la dreapta, ia la stânga (yaw $\psi$), urcă sus, coboară jos (pitch $\theta$), se rotește în jurul axei longitudinale (roll $\phi$), accelerează sau decelerează ($a_x = \dot{u}$)** — cuplând exact mișcarea avionului cu variația direcției și vitezei ceții.

---

## 2. Cinematica 6-DoF pe Cuaternioni Unitari & Sincronizarea IMU–Cameră

### 2.1. Starea Aeronavei în Reperul Pistei `LRCV` ($\mathcal{F}_n$) și Reperul Corpului ($\mathcal{F}_b$)
* **Poziția 3D în reperul NED centrat pe pragul RWY 09 `LRCV`**: $\mathbf{p}(t) = [x(t),\, y(t),\, z(t)]^T$, cu altitudinea față de sol $h_{AGL}(t) = -z(t) > 0$.
* **Viteza liniară în reperul corpului aeronavei ($\mathcal{F}_b$)**: $\mathbf{v}_b(t) = [u(t),\, v(t),\, w(t)]^T$
  * $u(t)$: viteza longitudinală înainte/înapoi (controlată prin **accelerare / decelerare** $a_x$),
  * $v(t)$: viteza laterală (**glisadă / derapaj stânga–dreapta**),
  * $w(t)$: viteza normală verticală (**urcare sus / coborâre jos**).
* **Atitudinea Fără Singularități (Cuaternion Unitar $\mathbf{q}_b^n \in \mathbb{H}_1$)**:
  Pentru a preveni blocajul matematic (*Gimbal Lock* la $\theta = \pm 90^\circ$), orientarea internă este reprezentată prin $\mathbf{q}_b^n = [q_w, q_x, q_y, q_z]^T$ ($\|\mathbf{q}_b^n\| = 1$), convertită în unghiuri Euler $(\phi\text{ [Roll]},\, \theta\text{ [Pitch]},\, \psi\text{ [Yaw]})$ pentru afișarea în Cockpit HUD:
  $$\dot{\mathbf{q}}_b^n = \frac{1}{2} \mathbf{q}_b^n \otimes \begin{bmatrix} 0 \\ \boldsymbol{\omega}_{IMU}(t) - \hat{\mathbf{b}}_g(t) \end{bmatrix}$$
* **Matricea Exactă de Rotație 3D ($\mathbf{R}_b^n(\mathbf{q}) \in SO(3)$)**:
  $$\mathbf{R}_b^n(\mathbf{q}) = \begin{bmatrix} 1 - 2(q_y^2 + q_z^2) & 2(q_x q_y - q_w q_z) & 2(q_x q_z + q_w q_y) \\ 2(q_x q_y + q_w q_z) & 1 - 2(q_x^2 + q_z^2) & 2(q_y q_z - q_w q_x) \\ 2(q_x q_z - q_w q_y) & 2(q_y q_z + q_w q_x) & 1 - 2(q_x^2 + q_y^2) \end{bmatrix}$$

### 2.2. Pre-Integrarea IMU pe Durata Expunerii Cadrului ($\Delta t_{sync}$ Compensation)
Pentru a elimina reziduurile de rotație la viraje și rotiri (roll) rapide cauzate de decalajul temporal $\Delta t_{sync}$ dintre IMU ($400\text{ Hz}$) și shutter-ul camerei ($60\text{ Hz}$), rata unghiulară efectivă între cadrele $t_{k-1}$ și $t_k$ este pre-integrată exact:
$$\bar{\boldsymbol{\omega}}_k = \frac{1}{\Delta t_k} \int_{t_{k-1}+\hat{\Delta t}_{sync}}^{t_k+\hat{\Delta t}_{sync}} \left(\boldsymbol{\omega}_{IMU}(\tau) - \hat{\mathbf{b}}_g\right) d\tau + \dot{\boldsymbol{\omega}}_k \hat{\Delta t}_{sync}$$

---

## 3. Ecuația Fluxului Optic 3D în Inversa Adâncimii (Zero `NaN`/`Inf` la Orizont)

### 3.1. Parametrizarea Inverse-Depth a Solului ($\rho_{ground}$)
În loc să împărțim la adâncimea $Z_c$ (care tinde la $\infty$ sau devine negativă când avionul ridică botul peste orizont), definim **inversa adâncimii planului solului** $\rho_{ground}(x_c, y_c) \equiv \frac{1}{Z_{ground}} \ge 0$:

$$\rho_{ground}(x_c, y_c, \phi, \theta, h_{AGL}) = \frac{\max\left(0,\; \cos\phi\cos\theta - x_c\sin\theta + y_c\sin\phi\cos\theta\right)}{\max(h_{AGL},\; 0.5\text{ m})}$$

* **Garanție Numerică Absolută**: Pentru toți pixelii situați deasupra liniei orizontului (care privesc spre cer/nori în timpul unui cabraj $\theta > 0$ sau ruliu $\phi$ extrem), $\rho_{ground}(x_c, y_c) = 0$ exact. Nu apare niciodată împărțire la zero (`NaN` / `Inf`).

### 3.2. Ecuația Completă a Fluxului Optic Cuplat (Avion 6-DoF + Masă de Ceață Dinamică)
Viteza optică totală observată la pixelul $(x_c, y_c)$ este:

$$\dot{\mathbf{u}}(x_c, y_c) = \underbrace{(1 - \alpha_{fog})\,\rho_{ground}(x_c, y_c)\,\mathbf{A}_{tr}(x_c, y_c)\mathbf{v}_b(t)}_{\text{Fluxul Translațional al Solului/Pistei}} + \underbrace{\alpha_{fog}\,\hat{\rho}_{fog}(t)\,\mathbf{A}_{tr}(x_c, y_c)\left[\mathbf{v}_b(t) - (\mathbf{R}_b^n(\mathbf{q}))^T \mathbf{w}_{fog}(z, t)\right]}_{\text{Fluxul Translațional Relativ al Ceții/Norilor}} + \underbrace{\mathbf{B}_{rot}(x_c, y_c) \bar{\boldsymbol{\omega}}_k}_{\text{Rotația 3D (Roll, Pitch, Yaw)}}$$

unde matricile cinematice de proiecție perspectivă sunt:
$$\mathbf{A}_{tr}(x_c, y_c) = \begin{bmatrix} -1 & 0 & x_c \\ 0 & -1 & y_c \end{bmatrix}, \qquad \mathbf{B}_{rot}(x_c, y_c) = \begin{bmatrix} x_c y_c & -(1 + x_c^2) & y_c \\ 1 + y_c^2 & -x_c y_c & -x_c \end{bmatrix}$$

### 3.3. Dubla Ancorare Fizică a Scării Adâncimii Ceții ($\hat{\rho}_{fog} = 1 / \hat{Z}_{fog}$)
Pentru a separa matematic scara adâncimii ceții $\hat{\rho}_{fog}$ de viteza ceții $\mathbf{w}_{fog}(z,t)$, sistemul fuzionează bayesian două ancore fizice independente:
1. **Ancora Optică Koschmieder**: Din vizibilitatea meteorologică $V_{MOR}$ și panta de atenuare a contrastului în benzile Vizibil + SWIR, coeficientul de extincție este $\hat{\beta}_{ext} \approx \frac{3.0}{V_{MOR}}$, iar inversa adâncimii efective a stratului de ceață este $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext}$.
2. **Ancora Inerțială Activă (*Active Inertial Scale Probing*)**: La orice variație cunoscută a vitezei aeronavei $\Delta\mathbf{v}_b$ (măsurată din integrarea pe $200\text{ ms}$ a accelerației IMU $a_x$ sau din rata de coborâre barometrică $\dot{h}_{baro}$), scara metrică absolută este:
   $$\hat{\rho}_{fog, inert} = \frac{\|\Delta\dot{\mathbf{u}}_{fog, trans}\|}{\|\mathbf{A}_{tr}\Delta\mathbf{v}_b\|}, \qquad \hat{\rho}_{fog} = \frac{\sigma_{inert}^2 \hat{\rho}_{fog, opt} + \sigma_{opt}^2 \hat{\rho}_{fog, inert}}{\sigma_{inert}^2 + \sigma_{opt}^2}$$

---

## 4. Cuplajul Aerodinamic 6-DoF la Modificarea Bruscă a Ceții (`Aero-Gust Coupling`)

O schimbare bruscă a vitezei ceții $\Delta\mathbf{w}_{fog}(t)$ este o rafală de vânt (*wind shear*) purtată de masa de aer. Sistemul tratează ambele faze fizice ale fenomenului:

1. **Faza 1 — `Optical Gust Lookahead` ($0.6\text{–}1.1\text{ s}$ Înainte ca Rafala să Lovească Avionul)**:
   Camera detectează modificarea bruscă a câmpului de viteze din ceață $\Delta\dot{\mathbf{u}}_{fog}$ la distanța $L_{lookahead} \in [35\text{ m}, 80\text{ m}]$ în fața avionului, în timp ce pe aripile avionului încă avem $\Delta\mathbf{a}_b = \mathbf{0}$ și $\Delta\boldsymbol{\omega} = \mathbf{0}$. Sistemul izolează instantaneu vectorul rafalei viitoare:
   $$\widehat{\Delta\mathbf{w}}_{fog}(t) = -\frac{1}{\hat{\rho}_{fog}} \mathbf{R}_b^n(\mathbf{q}) \mathbf{A}_{tr}^\dagger \Delta\dot{\mathbf{u}}_{fog, trans}$$
   și pre-comandă preventiv suprafețele de control (*Feed-Forward Gust Alleviation*).
2. **Faza 2 — `Coupled Aero-Inertial Impact & Crab Compensation` (Când Rafala Lovește Avionul)**:
   La impactul cu rafala, viteza relativă față de aer devine $\mathbf{v}_{air,b} = \mathbf{v}_b - (\mathbf{R}_b^n(\mathbf{q}))^T\mathbf{w}_{fog}(z,t)$, modificând unghiul de atac $\alpha_{AOA} = \text{atan2}(w_{air}, u_{air})$ și unghiul de derapaj lateral $\beta_{sideslip} = \arcsin(v_{air}/\|\mathbf{v}_{air,b}\|)$, ceea ce induce momente aerodinamice de ruliu și girație (*Dutch Roll* $C_{l\beta}, C_{n\beta}$).
   * Sistemul scade rotația indusă $\mathbf{B}_{rot}\bar{\boldsymbol{\omega}}_k$, actualizează $\mathbf{R}_b^n(\mathbf{q})$ și calculează **unghiul de corecție în vânt (Crab Angle $\psi_{crab}$)**:
     $$\psi_{crab}(t) = \psi_{LRCV}(t) - \arcsin\left(\frac{\hat{w}_{fog, crosswind}(t)}{\|\hat{\mathbf{v}}_{air}\|}\right)$$

---

## 5. Tabel Sintetic: Actualizarea Calculelor la Fiecare Manevră 6-DoF și Variație a Ceții

| Manevra Avionului / Fenomenul | Parametrii Modificați | Cum se actualizează calculele în timp real ($60\text{ Hz}$) |
| :--- | :--- | :--- |
| **Viraj Dreapta / Stânga (Yaw)** | $\psi \pm \Delta\psi$, $r \neq 0$, $v \neq 0$, $\mathbf{q}_b^n$ | Se elimină analitic fluxul de rotație laterală $\mathbf{B}_{rot}[0,0,\bar{r}]^T$, iar matricea $\mathbf{R}_b^n(\mathbf{q})$ rotește vectorul ceții $\mathbf{w}_{fog}$ și viteza avionului în noul unghi față de pista `LRCV`. |
| **Urcare Sus / Coborâre Jos (Pitch)** | $\theta \pm \Delta\theta$, $q \neq 0$, $w \neq 0$, $h_{AGL} \pm \Delta h$ | Se recalculează inversa adâncimii $\rho_{ground}(x_c, y_c, \phi, \theta, h_{AGL})$ fără risc de împărțire la zero la orizont și se interpolează profilul vertical de vânt/ceață (`10m / 80m / 120m / 180m`). |
| **Rotire (Roll / Înclinare Aripi)** | $\phi \pm \Delta\phi$, $p \neq 0$, $\mathbf{q}_b^n$ | Câmpul vectorial din cameră este derotat pe cuaternioni cu $\mathbf{B}_{rot}[\bar{p},0,0]^T$ și reproiectat prin $\mathbf{R}_b^n(\mathbf{q})$, prevenind confundarea înclinării aripilor cu un vârtej de ceață. |
| **Accelerare / Decelerare (Throttle)** | $a_x = \dot{u} \neq 0$, $u(t) = u_0 + \int a_x dt$ | Crește/scade divergența radială a fluxului optic; fixează scara $\hat{\rho}_{fog, inert}$ prin *Active Inertial Scale Probing* și actualizează viteza la sol $\hat{\mathbf{v}}_{ground}$ și $ETA$ până la pragul RWY 09. |
| **Modificare Bruscă a Ceții (în zbor drept sau în timpul manevrelor)** | $\Delta\mathbf{w}_{fog}(t) \neq 0$, $\beta_{sideslip}$, $\alpha_{AOA}$ | După eliminarea exactă a rotației ($\bar{\boldsymbol{\omega}}_k$) și translației avionului ($\mathbf{v}_b$), variația reziduală din stratul de ceață este izolată ca $\widehat{\Delta\mathbf{w}}_{fog}(t)$, declanșând *Optical Gust Lookahead* și corecția $\psi_{crab}$. |

---

## 6. Calculul Final al Distanței, Azimutului și Pantei ILS către Pista Craiova (`LRCV`)
Din viteza estimată față de sol $\hat{\mathbf{v}}_{ground}(t) = \mathbf{R}_b^n(\mathbf{q})\hat{\mathbf{v}}_b(t)$, poziția 3D se integrează în Error-State EKF:
$$\hat{\mathbf{p}}(t + \Delta t) = \hat{\mathbf{p}}(t) + \hat{\mathbf{v}}_{ground}(t)\Delta t + \frac{1}{2}\mathbf{R}_b^n(\mathbf{q})\mathbf{a}_b(t)\Delta t^2$$
iar parametrii de ghidaj către pragul pistei RWY 09 Craiova (`44.3181° N, 23.8886° E`) se actualizează la fiecare cadru:
$$d_{LRCV}(t) = \|\mathbf{p}_{LRCV} - \hat{\mathbf{p}}(t)\|_2, \qquad \psi_{LRCV}(t) = \text{atan2}\big(y_{LRCV} - \hat{y}(t),\; x_{LRCV} - \hat{x}(t)\big)$$
$$\Delta\psi_{\text{heading}}(t) = \text{wrap}_{[-\pi, \pi]}\big(\psi_{crab}(t) - \psi(t)\big), \qquad \Delta\gamma_{\text{glide}}(t) = \text{atan2}\left(\hat{h}_{AGL}(t),\; d_{\text{horiz}}(t)\right) - 3.0^\circ$$

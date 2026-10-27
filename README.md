# KnifePlanner — Griff-Planer für Messerdesigns

Natives Desktop-Programm (Rust, egui/eframe) für Windows 11, macOS und Linux.

Du fotografierst den **Klingenrohling** und das **Griffmaterial**, das Programm
schätzt die Maße, berechnet daraus die Griffkontur, die Bohrlöcher und die
Materialtauglichkeit — und liefert einen baubaren Plan:

| Datei | Inhalt |
|---|---|
| `*.dxf` | Griffschalen als Zuschnittkontur (1:1 in mm), Layer `SCHALE`, `ANGEL`, `AUSSPA`, `BOHRUNGEN` |
| `*.svg` | Bohrplan als Vektorgrafik |
| `*.stl` | Griffbaugruppe als Volumen: beide Schalen, Angel, Bastion, Aussparung |
| `*.obj` + `*.mtl` | dieselbe Baugruppe mit Materialfarben (Holz, Stahl, Messing) |
| `griff3d_*.png` | fotorealistischer Render des fertigen Griffes |
| `bohrplan_*.pdf` | Bohrplan: Zeichnung, Tabelle mit allen Bohrungen, Legende |
| `schablone_1zu1_*.pdf` | Druckschablone im Maßstab 1:1 zum Auflegen auf den Rohling |
| `bauplan_*.pdf` | Bauplan: Maße, Materialbewertung, Arbeitsreihenfolge |
| `bauplan_*.txt` | derselbe Bauplan als Text |
| `vorschau_*.png` | 2D-Ansicht des Griffplans |

---

## 1. Bauen unter Windows 11

> **Download:** Die fertigen Programme liegen als Release hier:
> <https://github.com/inf1nit3/knifeplanner/releases> — dort gibt es das ZIP
> mit allen Beispielen und die EXE einzeln (die braucht der eingebaute
> Updater).

### Variante A — fertiges EXE (empfohlen)

Ein Windows-Build läuft als GitHub-Action (siehe `.github/workflows/windows.yml`).
Im Reiter **Actions** den Workflow **Build Windows** starten; das Artefakt
`knifeplanner-windows` herunterladen und entpacken. Es enthält
`knifeplanner.exe` — keine Installation, keine Python-/Rust-Laufzeit nötig.

### Variante B — selbst bauen (ohne GitHub)

Rust installieren: <https://rustup.rs> (Windows: `rustup-init.exe`, Standard
„Install“ reicht). Danach im Projektordner:

```bat
build_windows.bat
```

Das Skript baut das Programm und legt `dist\knifeplanner.exe` ab.

Manuell entspricht das:

```bat
cargo build --release
copy target\release\knifeplanner.exe dist\
```

### Variante C — aus dem Quellcode starten

```bat
cargo run --release
```

---

## 2. API-Schlüssel für die Bildanalyse

Die Maße und das Material werden per Bildanalyse geschätzt. Dafür wird ein
MiniMax-API-Schlüssel gebraucht.

**Im Programm:** links unten *Einstellungen / API* öffnen, Schlüssel
eintragen, *Schlüssel speichern*. Gespeichert wird in
`%APPDATA%\knifeplanner\config.json` (Linux/macOS: `~/.knifeplanner/config.json`).

**Zur Sicherheit:** Der Schlüssel steht dort unverschlüsselt — die Datei ist
für jeden lesbar, der sie lesen kann. Auf einem fremden Rechner lieber die
Umgebungsvariable nutzen, dann wird gar nichts gespeichert. Das Programm
zeigt den Ablageort auch direkt unter dem Eingabefeld an.

**Oder per Umgebungsvariable:**

```bat
set MINIMAX_API_KEY=sk-...
set MINIMAX_MODEL=MiniMax-M3.1-Flash
knifeplanner.exe
```

| Variable | Bedeutung | Standard |
|---|---|---|
| `MINIMAX_API_KEY` | API-Schlüssel | – |
| `MINIMAX_BASE_URL` | API-Endpunkt | `https://agent.minimax.io/mavis/api/v1/llm/v1` |
| `MINIMAX_MODEL` | Modell mit Bild-Eingabe | `MiniMax-M3` |

**Welches Modell?**

| Modell | Bild-Eingabe | Verhalten |
|---|---|---|
| `MiniMax-M3` | ja |antwortet direkt, schnell, gute Genauigkeit |
| `MiniMax-M3.1-Flash` | ja | kostenlos, denkt aber vor der Antwort sehr lange (rund 13.000 Token Denken, 15–40 Sekunden pro Foto) |

Für `MiniMax-M3.1-Flash` setzt das Programm das Token-Limit automatisch auf
20.000 — mit kleinerem Limit kommt von dem Modell nur der Denkblock zurück und
keine Antwort. Den Modellnamen kannst du in den Einstellungen direkt auswählen.

Die Bildanalyse ist optional: alle Maße lassen sich auch von Hand eintragen,
das Programm rechnet ohne API-Schlüssel vollständig weiter.

---

## 3. Bedienung

### Fotos wählen und analysieren lassen

1. **Fotos wählen** — *Klingenrohling wählen* und *Griffmaterial wählen*.
   Ein Foto je Rohling und Material reicht; mehrere Materialien nacheinander
   sind möglich.
2. **Analysieren** — *Rohling analysieren* bzw. *Material analysieren*.
   Das Programm schlägt Maße vor (Angel-Länge/-Breite/-Dicke, Art der Angel,
   usw.) und ordnet das Material einem Eintrag der Datenbank zu.

Die Bildanalyse liefert **Verhältnisse** (Angelbreite zu Klingenhöhe,
Angelmitte zwischen Rücken und Schneide, usw.), weil sich absolute Maße aus
einem Foto ohne Maßstab nicht bestimmen lassen. Fehlen absolute Werte, werden
sie aus den Verhältnissen und den von dir eingetragenen Maßen abgeleitet.
Die KI weist selbst darauf hin, wenn ein Foto ohne Referenz nicht belastbar
ist — bitte die Vorschläge in den Schiebereglern prüfen.

### Selbst messen (Reiter *Messen*)

Wenn du die Maße genau wissen willst, misst du direkt am Foto:

1. Fotografiere den Rohling **von oben** mit einem bekannten Referenzmaß
   daneben — Kreditkarte (85,60 mm), 10-Euro-Schein (147 mm) oder ein Lineal.
2. Reiter **Messen** öffnen, Modus *Referenz kalibrieren*, Linie über das
   Referenzobjekt ziehen, die echte Länge eintragen.
3. Modus *Messen*: Linie über das Messobjekt ziehen. Die Länge in Millimetern
   erscheint sofort.
4. In der Tabelle darunter der Messung den passenden Parameter zuordnen und
   **übernehmen** klicken — der Wert wandert direkt in den Rohling.

### Speichern und Laden

Unter *5 – Export* lassen sich die Pläne als JSON sichern und wieder laden:
**Plan speichern** und **Plan laden**. Gespeichert werden Rohling, alle
Gestaltungsparameter, das Material, der Projektname und die Kameraeinstellung
der 3D-Ansicht. Beim Start kann ein Plan direkt geöffnet werden:

```bat
knifeplanner.exe --plan mein_griff.json
```

### Ergonomie und Gewicht

Das Programm rechnet den **Umfang** des geschlossenen Griffquerschnitts aus
(Rechteck mit gerundeten Ecken) und vergleicht ihn mit dem Zielwert der
gewählten **Handgröße**. Zu dünne oder zu dicke Griffe und ein zu kurzer
Griffriff werden im Bauplan angemerkt — mit der empfohlenen Griffhöhe.

Außerdem schätzt es das **Griffgewicht** aus Volumen und Materialdichte
(Holz, Stahl, Metall getrennt) und bewertet die Waage zwischen Griff und
Klinge.

### Weitere Bedienung

3. **Prüfen und korrigieren** — die Vorschläge stehen danach in den Schiebereglern
   unter *2 – Rohling*. Der Plan rechnet bei jeder Änderung sofort neu.
4. **Gestalten** — unter *3 – Griff*: Bauart, Gesamtdicke, Mindestwand,
   Wölbungen, Endform, Bastions- bzw. Zwingenbreite, Lanyardloch.
5. **Material wählen** — unter *4 – Material* mit Suchfeld; die Eignung wird
   sofort bewertet (Pro/Contra/Risiken, Klebstoff, Befestigung, Finish).
6. **Exportieren** — *Alle Dateien exportieren* schreibt alle Dateien
   in den angegebenen Ordner. Im selben Tab liegt auch *Mit FreeCAD neu
   bauen*; das STEP-Modell landet im Unterordner `freecad` des Zielordners,
   den *Ordner öffnen* bringt ihn direkt ins Fenster.

### Der Zuschnitt: so groß muss das Holz sein

Das Programm rechnet aus, wie groß die Holzleiste sein muss, aus der
Griffkontur plus Schleif- und Hobelzugabe. Die Angabe steht im Bauplan,
im Plantext, im DXF-Beschriftungstext, in der Druckschablone und im
Export-Tab.

Bei zwei Griffschalen ist die Angel aus **Stahl** — das Holz sind nur die
beiden Schalen, die Dicke entspricht also zwei Schalendicken, nicht der
Gesamtdicke des Griffes.

Das Programm prüft dabei auch die **Angel gegen die Klinge**: Ist die Angel
am Heft deutlich niedriger als die Klinge, ragt der Stahl über die Angel
hinaus und die Klinge hängt nur an einem schmalen Streifen. Der Plan schreibt
dann einen Hinweis in die App und in alle Dokumente (Bohrplan, Bauplan,
Textplan) — mit den genauen Maßen und der Empfehlung, die Angel-Breite auf
die Höhe der Klinge zu setzen.

### Druckschablone 1:1

Der Export schreibt zusätzlich eine **Druckschablone im Maßstab 1:1**
(`schablone_1zu1_*.pdf`). Sie ist millimetergenau und wird auf den Rohling
gelegt:

- beide Griffschalen spiegelbildlich, mit 12 mm Abstand zum Beschnitt
- die Angel und ihre Fräsbahn grün gestrichelt
- alle Bohrungen als Kreis mit Fadenkreuz und Nummer
- die Vorderkante des Rohlings rot markiert — dort wird die Schablone
  ausgerichtet
- eine 100-mm-Kontrolllinie: ist sie nach dem Ausdruck genau 100 mm lang,
  stimmt der Maßstab

### Updates aus der App

Unter *Einstellungen / API* → *Update* → *Nach Updates suchen* prüft das
Programm auf GitHub, ob eine neuere Version vorliegt. Wenn ja, lädt es die
neue Programmdatei herunter und setzt sie an die Stelle der alten — unter
Windows geht das, weil die laufende Datei kurz umbenannt wird. Danach
startet man das Programm einmal neu.

Der Updater braucht nur eine Internetverbindung zu
`api.github.com`; das Programm selbst läuft ohne Netz völlig weiter.

Neue Versionen entstehen, wenn ein Tag mit `v` beginnt (z. B. `v0.2.0`) auf
`main` liegt: der Build läuft dann automatisch und legt ein Release an.

### Datenblätter anhängen

Im Export-Tab lassen sich **Datenblätter** an den Plan hängen — typisch
das Lieferdatenblatt des Klingenstahls mit Zusammensetzung, Härte und
Nachbehandlung, aber auch Zeichnungen, Prüfprotokolle oder Fotos.

* *Datenblatt hinzufügen* nimmt eine Datei (PDF, Bild oder Text) auf, der
  Dateiname wird dabei als Bezeichnung vorgeschlagen.
* Die Bezeichnung ist frei änderbar, *Öffnen* zeigt die Datei im
  Systemprogramm, *Entfernen* nimmt sie wieder vom Plan.
* Beim Export wandern die Dateien in den Unterordner `datenblaetter` des
  Zielordners — die Werkstatt hat damit alle Unterlagen zusammen.
* Der Bauplan (PDF und Text) nennt die Datenblätter mit Bezeichnung und
  Dateinamen, damit nichts verloren geht.
* Gespeichert wird alles in der Plan-Datei (JSON). Alte Plan-Dateien ohne
  Datenblätter lassen sich weiterhin öffnen.

Tabs rechts: **3D** (drehbares Modell), **Vorschau** (2D-Zeichnung),
**Messen**, **Bohrplan** (Tabelle), **Material** (Bewertung),
**Bauplan** (Text), **Log** (Rohantworten der KI).

### Grifflaenge und Querschnitt

Die **Grifflaenge** laesst sich unabhaengig von der Angel vorgeben. Reicht die
Angel nicht bis zum Griffende, entsteht die Bauart eines **Partial Tangs** —
das Programm weist darauf hin, weil dann nur noch der Kleber die Schalen
zusammenhaelt.

Der Reiter **Schnitt** zeigt den Querschnitt an jeder Stelle des Griffs:
beide Schalen, die Angel in der Mitte und die Wandstaerken am Ruecken und an
der Bauchseite. Ueber den Regler *Schnittposition* laesst sich die Angel
durch den ganzen Griff verfolgen — so sieht man sofort, ob die Wand zu dünn
wird oder die Angel zu tief sitzt. Derselbe Querschnitt steht als Zeichnung
im **Bohrplan-PDF** — links die Bohrbank, rechts der Schnitt.

Mit *Griffstärke nach Handgröße übernehmen* setzt das Programm die Griffhöhe
automatisch auf den Wert, der zum Umfangs-Ziel der gewählten Handgröße passt.

### Querschnitt

Der Griff ist kein Quader, sondern ein **Loft mit echtem Querschnitt**:

- **Querschnittsform** – *Flach* (zwei Griffschalen mit planen Klebeflächen),
  *Oval* (weich in der Hand) oder *Oval mit Längsgrat*
- **Rückenrundung** und **Bauchrundung** – die Komfortrundung in mm
- **Längsgrat** – die Tiefe des Grats an der Bauchseite (Ein-Stück- und Zwingengriff)
- **Dickeverjüngung** – wie stark der Griff zum Ende hin dünner wird

Beim Ein-Stück-Griff bleibt die Nut für die Angel exakt maßhaltig – sie wird
nicht mit gestaucht.

### FreeCAD als Rechenkern

Optional, aber empfehlenswert: Ist **FreeCAD** installiert, rechnet das Programm
das 3D-Modell im Hintergrund mit FreeCAD statt mit dem eigenen Netzgenerator.
Das bringt echte Vorteile:

* **Ein echtes Volumen** — kein aus Dreiecken gerastertes Netz, sondern ein
  geschlossener B-Rep-Solid, den CAD-Programme ohne Nacharbeit lesen
* **Gleiche Form wie die 3D-Ansicht** — der Griff entsteht als Loft durch
  dieselben Ringe, aus denen auch die Ansicht in der App aufgebaut wird.
  Komfortrundung, Längsgrat, Verjüngung und Kantenfase stecken dadurch von
  Anfang an im Volumen, statt nachträglich weggefranst zu werden
* **Exakte Bohrungen** als echte Zylinder im Volumen
* **Klinge und Angel sind ein Stück** — sie werden zu einem Solid
  verschmolzen, so wie sie am Messer wirklich aus einem Stück Stahl sind
* **Zwinge und Bastion** sind Mantel um das Holz bzw. mit gebrochenen Kanten,
  keine aufgesetzten Klötze
* **Geprüfte Übereinstimmung** — der Selbsttest vergleicht das Volumen des
  eingebauten Netzes mit dem von FreeCAD: beide Modelle zeigen dasselbe
* **STEP-Ausgabe** (`.step`) — die native Form, die FreeCAD, Fusion, SolidWorks
  und Inventor einwandfrei lesen

**Einrichtung:** FreeCAD installieren (freecad.org). Das Programm findet
`FreeCADCmd.exe` in den üblichen Pfaden automatisch — beim Start wird
danach gesucht. **Ist FreeCAD installiert, rechnet das Programm das
3D-Modell automatisch damit**; etwas einzustellen ist nicht nötig.
Falls nicht gefunden: *Einstellungen / API* → *FreeCAD* → Pfad eintragen
(z. B. `C:\Program Files\FreeCAD 1.0\bin`) → *Suchen* → *Speichern*.

Wer lieber das eigene Modell der App sieht, schaltet das im selben Fenster
ab — die Entscheidung wird gemerkt.

**Bedienung:** Reiter *3D* → *Mit FreeCAD neu bauen*. Die App startet
`FreeCADCmd` ohne Fenster, lässt das Volumen rechnen und zeigt das Ergebnis
anschließend in der 3D-Ansicht. Im Tab *Log* steht, wie lange es gedauert hat
und wie groß das gerechnete Volumen ist. Rechnen dauert zwei bis fünf
Sekunden, beim Ein-Stück-Griff mit Angel-Nut etwas länger. Die Oberfläche
bleibt dabei bedienbar — FreeCAD rechnet im Hintergrund.

Auf der Kommandozeile:

```bat
knifeplanner.exe --freecad                      :: FreeCAD suchen und rechnen
knifeplanner.exe --freecad "C:\PF\FreeCAD 1.0\bin" ausgabeordner
```

Ohne FreeCAD bleibt alles funktionsfähig — das eigene Modell ist der
Rückfall. Der Selbsttest sagt dir, welche Variante aktiv ist.

### Das 3D-Modell

Der Reiter **3D** zeigt den geplanten Griff als echtes Volumen — keine
Skizze, sondern die Konstruktion:

- **Die Klinge** mit ihrem Rücken- und Schneidenverlauf. Sie setzt die
  Kontur des Rohlings am Heft fort und läuft nach vorn in die Spitze aus;
  die Dicke verteilt sich vom dicken Rücken zur dünnen Schneide.
- **Beide Griffschalen** mit sauber gerundeten Kanten (Radius = Kantenabrundung,
  für die Darstellung großzügiger als der Fertigungswert).
- **Die Angel** sitzt bündig zwischen den Schalen, der Spalt ist sichtbar.
- **Bastion oder Zwinge** als eigener Metallkörper.
- **Die Aussparung** für die Angel liegt als Fräsbahn bei.
- **Beim Ein-Stück-Griff** ist die Nut für die Angel in den Boden gefräst und
  die Angel liegt sichtbar darin.
- **Die Bohrungen** erscheinen als rote Zylinder – man sieht direkt, wie die
  Bohrer durch die Schale gehen. Im STL sind sie bewusst nicht enthalten
  (dort gehören nur die echten Bauteile hinein).

Mit der Maus **ziehen** dreht das Modell in alle Richtungen (360° um die
Längs- und die Querachse), das **Mausrad** zoomt, ein **Doppelklick** setzt
die Ansicht zurück. *PNG speichern (hochaufgelöst)* legt einen Render mit
1800 × 1100 Pixel und Kantenglättung ab.

Der Renderer läuft auf der CPU und braucht daher keine GPU — auf einem
9800X3D ist die Darstellung flüssig, das Nachrechnen dauert wenige
Millisekunden.

### Kommandozeile

```bat
knifeplanner.exe --foto pfad\zu\rohling.jpg   :: Foto direkt im Messwerkzeug oeffnen
knifeplanner.exe --analyse pfad.jpg          :: Bildanalyse ohne Fenster (JSON)
knifeplanner.exe --analyse pfad.jpg --material
knifeplanner.exe --render3d ausgabe.png 55 -25 bastion :: 3D-Render, Stil frei waehlbar
knifeplanner.exe --demo                      :: Beispielplan ohne Fenster erzeugen
```

Stile für `--render3d`: `full` (zwei Griffschalen), `bastion`, `einstueck`, `zwinge`.
Danach optional das Material, z. B. `--render3d aus.png 45 -20 bastion palisander`.

---

## 4. Was die Geometrie-Engine rechnet

**Griffkontur** aus Rücken- und Bauchkurve des Heftprofils, mit garantierter
Mindestwand zwischen Angel und Außenkontur. Varianten: rund, flach, ausgeweitet.
Vorderkante optional mit Fase.

**Zwei Profilarten:**

- *Rohling-Profil folgen* — die Griffkontur übernimmt das Heftprofil des
  Rohlings. Passt, wenn die Klinge und der Griff eine Linie bilden sollen.
- *Ergonomisches Griffprofil* (Voreinstellung) — die Kontur ist vom Rohling
  entkoppelt: aus dem Heft wächst der Griff in den ersten 22 % auf die
  eingestellte **Griffhöhe**, der Rücken fällt zum Ende sanft ab, die Bauchlinie
  bildet eine **Ballenauflage** und eine flache **Fingerkehle** zwischen den
  Fingern. Die Angel muss weiterhin mit der Mindestwand hineinpassen — das
  prüft die Engine und reicht die Wand bei Bedarf auf.

**Schalendicke** aus der gewünschten Gesamtdicke:
`Schalendicke = (Gesamtdicke − Angeldicke) / 2`, begrenzt auf 2,5–10 mm.

**Bohrplan** nach den üblichen Regeln:

- *Vorsteckbolzen* – vorderer Bolzen im geraden Teil der Angel, hinterer so
  weit hinten, wie die Angel noch mindestens breit genug ist (Bolzen-Ø plus
  Rand). Abstand zwischen zwei Bolzen höchstens 75 mm, sonst kommt ein
  mittlerer dazu. Bolzen-Ø ≈ 0,5 × Angeldicke, begrenzt auf 2–5 mm.
- *Reibniet* – Ø 2,4 mm, halbtief, im geraden Teil der Angel, damit die Schale
  sich im vorderen Griffdrittel nicht verdreht. Die Nut wird in die Angel
  gefräst.
- *Lanyardloch* – Ø 5 mm, quer durch die Dicke hinter der Angel. Das Programm
  legt es automatisch hinter das Angelende, damit die Angel weder
  entlastet noch durchbohrt werden muss. Reicht der Griff dafür nicht
  (Angel reicht bis zum Ende), wird das Loch durch die Griffhöhe gebohrt
  und endet unter der Angel – dann braucht es einen Entlastungsslot in der
  Angel.
- *Querbolzen* statt Vorsteckbolzen, wenn die Angel dünner als 2,6 mm ist.

**Materialbewertung** aus 23 Materialien (Holz, Horn, Knochen, Geweih,
Messing, Kupfer, Edelstahl, Titan, Aluminium, Composites) mit Dichte,
Janka-Härte, Ölaufnahme, Formstabilität, Rissrisiko und Bearbeitbarkeit.
Ergebnis: Eignungsscore, Vor-/Nachteile, Risiken, Klebstoff, Befestigung,
Finish und Konstruktionstipps.

**Griffbauarten:** Full Tang mit zwei Griffschalen, verdeckter Tang mit
Bastion, Ein-Stück-Griff, Holzgriff mit Messing-Zwinge. Die Engine schlägt
passend zum Rohling eine Bauart vor.

---

## 5. Hinweise für die Werkstatt

- Der Klebespalt für Horn, Knochen und Geweih muss 0,03–0,05 mm betragen, sonst
  bricht die Leimfuge. Holz verträgt einen Spalt von 0,00–0,03 mm.
- Ein Stift durch die Angel gehört in den geraden, vollen Teil — in der
  Verjüngung bricht die Angel aus.
- Bei einer Mindestwand von 2 mm und einer Angeldicke von 4,5 mm bleiben
  außerhalb der Angel gut 1 mm Holz. Das Programm warnt in diesem Fall
  ausdrücklich.
- Die Vorschau und der DXF sind Werkstattplanung, kein Konstruktionsersatz.
  Wer selbst fräst, prüft Passung und Spannungsrisiko am Rohling.

---

## 6. Projektstruktur

```
src/
  model.rs        Datenmodell (Rohling, Griffparameter, Bohrungen)
  geometry.rs     Geometrie-Engine: Kontur, Angelprofil, Bohrbild
  blade.rs        Die Klinge: Profil, Querschnitte, Volumen
  materials.rs    Materialdatenbank und Eignungsbewertung
  mesh.rs         Ear-Clipping, Volumen, gerundete Platten, STL-Export
  render3d.rs     Software-Renderer (Z-Buffer, Beleuchtung, Maserung)
  measure.rs      Messwerkzeug: Kalibrierung und Messungen
  plan.rs         Plan-Zusammenbau, Bauplan-Text, PDF-Dokumente
  vision.rs       Bildanalyse über die MiniMax-API (JSON)
  export/
    dxf.rs        DXF R12 (Zuschnitt der Griffschalen)
    svg.rs        SVG-Bohrplan
    stl.rs        STL- und OBJ-Export der Baugruppe
    pdf.rs        Minimaler PDF-Writer (ohne Fremdbibliothek)
    preview.rs    PNG-Vorschau (2D)
  ui.rs           egui-Oberfläche
```

Tests: `cargo test` (40 Tests für Geometrie, Klinge, Mesh, Messwerkzeug und JSON-Parsing).

Beispielplan ohne Fenster erzeugen:

```bash
cargo run --release -- --demo
```

Der erste Release-Build dauert je nach Rechner einige Minuten (Rust kompiliert
die Abhängigkeiten einmalig); danach dauert ein Rebuild nur Sekunden.

---

## 7. Lizenz und Haftung

Freie Nutzung. Der Plan ist eine Arbeitshilfe — jede Fertigung
verantwortet der Messerbauer selbst.

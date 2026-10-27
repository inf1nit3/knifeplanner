# Prüfanleitung für den Testlauf

Diese Anleitung führt durch alles, was KnifePlanner kann. Wenn du mir am Ende
das Ergebnis und die Logdatei schickst, kann ich gezielt nacharbeiten.

---

## 0. Vor dem Start (5 Minuten)

### a) Selbsttest laufen lassen

```
knifeplanner.exe --selbsttest pruefbericht
```

Der Test prüft ohne Fenster: die Geometrie über **864 Parameterkombinationen**,
Randfälle, alle vier Bauarten mit allen sieben Exporten, den Querschnitt, die
Materialdatenbank, die Bauarten-Empfehlung, das Speichern/Laden und — falls ein
API-Schlüssel hinterlegt ist — einen echten Bildanalyse-Aufruf.

Erwartet wird:

```
10 Pruefpunkte bestanden, 0 fehlgeschlagen
```

Der Bericht liegt danach in `pruefbericht\selbsttest.txt`, die erzeugten
Dateien in `pruefbericht\selbsttest_*\`.

### b) API-Schlüssel prüfen

*Einstellungen / API* → Schlüssel eintragen → *Schlüssel speichern* → *Material
analysieren* mit einem Foto. Kommt eine Antwort, passt der Schlüssel.

Zwei Modelle stehen zur Wahl:

| Modell | Verhalten |
|---|---|
| `MiniMax-M3` | antwortet direkt, ca. 4 Sekunden, genauer |
| `MiniMax-M3.1-Flash` | kostenlos, denkt erst sehr lange, ca. 40 Sekunden |

### c) FreeCAD (optional)

Ist FreeCAD installiert, rechnet es das 3D-Modell als echtes Volumen mit
Fasen und Bohrungen und liefert zusätzlich eine STEP-Datei.

*Einstellungen / API* → *FreeCAD* → *Suchen* → *Speichern*. Findet das
Programm FreeCAD nicht von selbst, den Pfad eintragen:
`C:\Program Files\FreeCAD 1.0\bin` (bzw. der Ordner, in dem
`FreeCADCmd.exe` liegt).

Der Selbsttest sagt dir, ob FreeCAD gefunden wurde und ob es rechnen kann —
er prüft alle vier Bauarten durch und meldet Zeit, Größe der STEP-Datei und
das gerechnete Volumen. Im Reiter *3D* findest du *Mit FreeCAD neu bauen* —
die App startet FreeCAD ohne Fenster, rechnet und zeigt das Ergebnis.

**Automatik:** Beim Start sucht das Programm nach FreeCAD. Ist es
installiert, rechnet es das Modell **von selbst damit** — du musst nichts
einstellen. Im Fenster *Einstellungen / API* steht, ob FreeCAD gefunden
wurde und welche Variante gerade aktiv ist. Wer das eigene Modell der App
sehen will, nimmt das Kästchen weg; die Entscheidung bleibt dann erhalten.
Die Oberfläche bleibt während des Rechnens bedienbar.

**Prüfen:** Öffne die erzeugte `.step`-Datei in FreeCAD. Sind die Fasen an
den Kanten sauber, sind die Bohrungen echte Zylinder (keine aufgesetzten
Punkte) und ist es ein geschlossenes Volumen? Genau das ist der Unterschied
zum eingebauten Modell. Die Datei liegt im Exportordner unter `freecad\`.

**Konsistenz:** Der Selbsttest vergleicht das Volumen des eingebauten Netzes
mit dem, was FreeCAD gerechnet hat — beide Modelle müssen dasselbe Ding
darstellen (Abweichung bis 3 % durch die Facettierung).

**Unter Windows:** FreeCAD sucht das Programm in den üblichen Pfaden
(`C:\Program Files\FreeCAD ...`). Findet er es nicht, den Pfad in den
Einstellungen eintragen. Auch Pfade mit Leerzeichen und mit Apostroph im
Benutzernamen laufen.

### d) Protokolldatei

Alles wird protokolliert. Die Datei liegt hier:

* Windows: `%APPDATA%\knifeplanner\knifeplanner.log`
* Im Tab *Log* steht der Pfad ebenfalls.

---

## 1. Fotos richtig machen

Für brauchbare Maße braucht die Analyse **ein bekanntes Referenzmaß im Bild**:

1. **Kreditkarte** (85,60 mm) neben den Rohling legen — die ist überall dabei.
2. **Von oben** fotografieren, Kamera parallel über dem Objekt, kein Perspektivfehler.
3. Gleichmäßiges Licht, keine harten Schatten, Hintergrund ruhig.
4. Rohling und Material getrennt fotografieren.
5. Beim Material: ein Stück vom **verwendeten Holz** fotografieren, nicht nur die
   Schachtel. Gegenlicht vermeiden.

Tipp: Nimm für den Rohling **zwei** Fotos — eines von der Seite (für die
Dicke) und eines von oben (für die Kontur). Das Programm erkennt die
Angelform besser, wenn sie vollständig im Bild ist.

---

## 2. Was du prüfen kannst

### 2.1 Bildanalyse (Reiter *Log* zeigt die Rohantwort)

| Prüfpunkt | Erwartung |
|---|---|
| Maße | plausibel für deinen Rohling, **keine 0-Werte** |
| Verhältnisse | `tang_width_ratio` zwischen 0,2 und 0,4 |
| `notes` | nennt die Unsicherheit, wenn kein Maßstab im Bild ist |
| Material | Kategorie und Art passen, Vorschlagsliste passt zum Material |

**Wichtig:** Die Analyse ist ein *Vorschlag*. Prüfe jede Zahl im Regler
*2 – Rohling* nach. Stimmt etwas nicht, korrigiere es — die Berechnung
übernimmt sofort.

### 2.2 Messwerkzeug (Reiter *Messen*)

1. Foto laden, *Referenz kalibrieren*.
2. Linie über die Kreditkarte ziehen, Länge 85,60 mm eintragen.
3. Modus *Messen*: Linie über den Rohling ziehen.
4. Prüfen: Entspricht die gemessene Länge dem erwarteten Maß?
5. Weitere Messungen anlegen und die Werte einem Parameter zuordnen.

### 2.3 Querschnitt (Reiter *Schnitt*)

1. *Schnittposition* über den ganzen Griff schieben.
2. Prüfen: Bleibt die Wand überall ≥ 1,5 mm?
3. Prüfen: Sitzt die Angel mittig im Material?
4. Bei einer dünnen Stelle: Der Schnitt warnt rot.

### 2.4 3D-Modell (Reiter *3D*)

1. Ziehen → drehen. Mausrad → zoomen. Doppelklick → Ansicht zurücksetzen.
2. Prüfen: Stimmen die Bohrungen mit dem Bohrplan überein (Nummern 1–4)?
3. Querschnittsform durchprobieren: *Flach*, *Oval*, *Oval mit Längsgrat*.
4. Rücken-/Bauchrundung und Verjüngung verstellen.
5. Prüfen: Steht die Klinge vor dem Griff? Ihre Kontur muss am Heft genau in
   die Griffkontur übergehen, ohne Absatz. Liegt das Lanyardloch hinter der
   Angel? Es darf nicht in der Angel liegen, sonst muss die Angel entlastet
   werden.

### 2.5 Zuschnitt (Export-Tab und Bauplan)

1. Prüfen: Steht die Angabe *Zuschnitt: Holzleiste … x … x … mm* im
   Export-Tab, im Bauplan und im DXF-Beschriftungstext?
2. Prüfen: Ist die Dicke bei zwei Schalen kleiner als die Gesamtdicke
   des Griffes? (Die Angel ist Stahl, das Holz sind nur die Schalen.)
3. Prüfen: Umfasst die empfohlene Leiste die Griffkontur mit etwas
   Zugabe zum Schleifen?

### 2.6 Druckschablone 1:1

1. *Alle Dateien exportieren* → `schablone_1zu1_*.pdf` öffnen.
2. Prüfen: Liegen beide Griffschalen spiegelbildlich auf dem Blatt, mit
   12 mm Abstand zum Beschnitt?
3. Prüfen: Sind alle Bohrungen als Kreis mit Fadenkreuz und Nummer
   eingetragen, passend zur Tabelle?
4. Ausdrucken und die **100-mm-Linie** messen: sie muss genau 100 mm
   lang sein, sonst stimmt der Maßstab nicht.
5. Prüfen: Liegt die rote Vorderkante an der Vorderkante des Rohlings,
   wenn die Schablone aufgelegt wird?

### 2.7 Datenblätter (Reiter *5 – Export*)

1. *Datenblatt hinzufügen* → eine PDF (z. B. das Datenblatt des
   Klingenstahls) auswählen.
2. Prüfen: Steht die Datei in der Liste, und lässt sie sich mit *Öffnen*
   anzeigen?
3. Prüfen: Nennt der Bauplan (PDF und Text) das Datenblatt mit Bezeichnung
   und Dateinamen?
4. *Alle Dateien exportieren* → Prüfen: Liegt die Datei im Unterordner
   `datenblaetter` des Zielordners?
5. Prüfen: *Plan speichern*, neu laden — das Datenblatt ist noch da.

### 2.8 Export (Reiter *5 – Export*)

*Alle Dateien exportieren* erzeugt:

| Datei | Prüfen |
|---|---|
| `*.dxf` | öffnen in Inkscape/LightBurn/Fusion — zwei Schalen, gespiegelt, Maß 1:1 |
| `*.stl` | in Meshmixer/Fusion öffnen — Volumen, nicht nur eine Hülle |
| `*.obj` + `*.mtl` | mit Materialfarben (Holz, Stahl, Messing) |
| `bohrplan_*.pdf` | Zeichnung, Tabelle, Querschnitt — passt alles auf eine Seite? |
| `bauplan_*.pdf` | Maße, Materialbewertung, Arbeitsreihenfolge |
| `datenblaetter/` | die angehängten Datenblätter (z. B. vom Klingenstahl) |
| `schablone_1zu1_*.pdf` | Druckschablone 1:1 — die 100-mm-Linie messen! |
| `bauplan_*.txt` | zum Nachlesen |
| `griff3d_*.png` | der Render, wenn er dir gefällt |

### 2.9 Bauarten durchprobieren

Wechsle *Griffbauart* oben in der Liste:

| Bauart | Prüfen |
|---|---|
| Full Tang, zwei Griffschalen | Schalen gespiegelt, Angel dazwischen |
| Verdeckter Tang mit Bastion | Bastion als Metallkörper, Holz ab der Bastion |
| Ein-Stück-Griff | Nut in der Bauchseite, Angel liegt darin |
| Holzgriff mit Messing-Zwinge | Zwinge als eigener Körper |

### 2.10 Material

*4 – Material* → verschiedene Materialien durchprobieren (Ebenholz, Horn,
Olivenholz, Messing). Beobachte Eignungsscore, Risiken und Gewicht.

### 2.11 Handgröße und Ergonomie

*Handgröße* wechseln → *Griffstärke nach Handgröße übernehmen* →
der Umfang im Tab *Material* sollte zum Zielwert passen.

---

## 3. Speichern und Laden

*Plan speichern* → JSON ablegen → *Plan laden* → alle Werte müssen wieder da
sein (inklusive Material, Querschnitt und Kameraeinstellung).

---

## 4. Was du mir schickst

1. **Die Protokolldatei** `%APPDATA%\knifeplanner\knifeplanner.log`
2. **Den Selbsttest-Bericht** `pruefbericht\selbsttest.txt`
3. **Ein Foto deines Rohlinges** und **ein Foto des Materials** — dann kann ich
   die Analyse mit deinen echten Bildern prüfen
4. **Eine Liste**, was nicht funktioniert hat oder unlogisch wirkt

---

## 5. Bekannte Grenzen

* Die Bildanalyse schätzt **Verhältnisse** zuverlässig, absolute Millimeter nur
  mit Referenzmaß im Bild. Ohne Maßstab bleiben die Absolutwerte leer.
* Die 3D-Ansicht ist ein **Weichbild** mit Z-Buffer — kein fotorealistisches
  Rendering, aber konstruktiv korrekt (alle Maße stimmen).
* Der STL enthält die **Baugruppe** (Schalen, Angel, Bastion, Aussparung als
  getrennte Körper). Für den 3D-Druck musst du die Volumen im CAD vereinigen
  oder einzeln drucken.
* Bei der **Bastion** (teildeckende Angel) liegt die Angel im massiven Holz —
  das Modell kann keinen geschlossenen Kanal für die Angel darstellen. Im
  FreeCAD-Modell überlappen sich Holz und Angel dort. Beim Ein-Stück- und
  beim Zwingengriff ist die Angel in einer echten Nut, dort stören sich die
  Volumen nicht.
* Die **Zwinge** ist ein Mantel um das vordere Griffende. Sie überbrückt die
  Nut der Angel nicht — dafür müsste ihr Loch die Form der Angel haben.
* Bohrungen erscheinen **nur in der 3D-Ansicht** als rote Zylinder, im STL
  gehören nur die echten Bauteile hinein.

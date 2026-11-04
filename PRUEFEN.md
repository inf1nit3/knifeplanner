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
15 Pruefpunkte bestanden, 0 fehlgeschlagen
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
| Verhältnisse | `tang_width_ratio` = Höhe des Stahls am Heft / Höhe der Klinge am Heft — bei einer durchgehenden Angel nahe 1,0, bei einem verdeckten Ansatz etwa 0,2 bis 0,4 |
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

## 3a. Der Updater

Das Programm sucht **zwei Sekunden nach dem Start** von selbst auf GitHub
nach einer neuen Version. Im Protokoll steht das so:

```
[1793743517] Suche nach einer neuen Version ...
[1793743518] Alles aktuell (Version 0.5.0)
```

Ist eine neuere Version da, steht der Hinweis oben im Fenster:

```
[1793743560] Neue Version gefunden: 0.5.0
```

Der Ablauf ist halbautomatisch: **Suchen** passiert von selbst, das
**Laden** ist ein Klick auf *Herunterladen und einsetzen*, und danach setzt
das Programm die neue Datei selbst ein und **startet neu**. Vorher wird der
Plan als `letzter_plan.json` neben dem Programm gesichert.

**Selbst prüfen** (ohne die Oberfläche zu brauchen):

```
cargo test --release -- --ignored --nocapture
```

Das testet gegen das echte Release auf GitHub: es muss eine neuere Version
finden und die Datei byteweise herunterladen. Erwartet wird:

```
gefunden: Version 0.5.0 (11.0 MB)
geladen: 11511808 Bytes (erwartet 11511808)
```

**Abschalten:** *Einstellungen / API* → *Update* → *Beim Start selbst
suchen*.

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
  oder einzeln drucken. Die Aussparung gibt es nur dort, wo das Holz wirklich
  für die Angel aufgeschnitten wird — bei zwei Griffschalen gibt es keine.
* Bei der **Bastion** (teildeckende Angel) liegt die Angel vorn im Metall
  und läuft weiter durch das Holz. Seit 0.7.0 sparsen das Holz sie dort
  aus — vorher steckten Holz und Stahl ineinander, der Griff war an dieser
  Stelle nicht baubar. Die Nut beginnt dort, wo das Holz beginnt (hinter
  der Bastion).
* Die **Zwinge** ist ein Mantel um das vordere Griffende. Sie überbrückt die
  Nut der Angel nicht — dafür müsste ihr Loch die Form der Angel haben.
* Bohrungen erscheinen **nur in der 3D-Ansicht** als rote Zylinder, im STL
  gehören nur die echten Bauteile hinein.
* Die **Tasche für die Angel** gibt es beim Ein-Stück-Griff, beim Zwingengriff
  und bei der Bastion: Dort wird das Holz für die Angel aufgeschnitten — die
  Tasche sitzt **so tief wie die Angel dick ist** (4,5 mm). Das Holz steht
  daneben fest, es bleibt also eine Klebefläche. Im Modell liegt die Tasche
  in der Mitte der Dicke (z = −2,25 … +2,25 mm), das Holz aussen und
  innen ist massiv.
  *Warum das nicht einfach war:* Der Griffkörper ist ein Loft über Ringe,
  und ein Ring hat an jeder Stelle gleich viele Punkte. Eine flache Nut
  braucht aber an der Vorderkante andere Punkte als der Rest. Deshalb
  bekommen auch die Ringe ohne Nut dieselbe Zahl Punkte: die Vorderkante
  wird feiner unterteilt. An den Grenzen der Nut stehen zwei Ringe
  nebeneinander — dazwischen liegt die Schulter, auf der die Angel
  aufliegt. Ohne das fehlte im Griff eine ganze Schicht Holz (siehe 5.3).
* Bei **zwei Griffschalen** gibt es **keine** Tasche: die Angel sitzt in der
  Fuge zwischen den Schalen, die Schalen liegen flach an ihr. Der Plan, der
  DXF und die Druckschablone weisen das auch so aus — im DXF ist der Layer
  *AUSSPA* bei dieser Bauart leer.

### 5.1 Angelsteg (ab 0.8.0)

Der Rohling kann einen **Angelsteg** haben (Handschutz). Bis 0.7.x gab es
dazu nur einen Schalter in der Oberfläche — im Modell war nichts zu sehen.
Seit 0.8.0 steht der Steg quer vor dem Griff:

- Dicke, Flügelüberhang und der Teil unter der Schneide sind einstellbar.
- Die Kontur liegt in der y-z-Ebene, weil der Steg quer zur Klinge steht —
  dafür gibt es im Netz und im Makro ein eigenes Prisma.
- Die Angel geht durch den Schlitz in der Mitte; der Steg wird dort
  festgeklemmt.

**Prüfen:** *2 – Rohling* → *Angelsteg* anhaken. In der 3D-Ansicht steht
ein Stahlsteg vor dem Griff, im Bauplan steht der Herstellungsschritt, und
der Selbsttest prüft, dass FreeCAD den Steg als eigenen Körper erzeugt.

### 5.2 Angel und Klinge: die durchgehende Angel (ab 0.3.0)

Bis 0.2.x war die Angel im Modell **6,5 mm hoch**, die Klinge am Heft aber
**26,0 mm**. Die Angel setzte die Klingenkontur also nicht fort: der Stahl
ragte je 9,8 mm über die Angel hinaus und die Klinge hing nur an einem
schmalen Streifen. Ein Griff mit dem Namen „Full Tang" sah damit nicht aus
wie ein Full Tang.

**Seit 0.3.0 ist das behoben.** Die Angel nimmt die Höhe der Klinge am Heft
an und verjüngt sich nach hinten auf eine Spitze. Nachgemessen im 3D-Modell
(Standardmaß, Bauart *Full Tang, zwei Griffschalen*):

| Teil | y (Höhe) | z (Dicke) |
|---|---|---|
| Klinge | −13,0 … +13,0 (26,0 mm) | −2,25 … +2,25 (4,5 mm) |
| Angel | −13,0 … +13,0 (26,0 mm) | −2,25 … +2,25 (4,5 mm) |
| Griffschalen | −16,0 … +15,0 | ±2,25 … ±11,0 |

Der Griff ist dadurch 2 mm höher (31 mm statt 29 mm): über und unter der
Angel bleiben 2,0 mm bzw. 3,0 mm Holz, damit der Stahl nirgends aus dem
Griff herausragt.

**Prüfen:**

1. *2 – Rohling* → *Angel-Breite* steht auf **0** = automatisch = Klingenhöhe
   am Heft.
2. Der Bohrplan zeigt die Angel (grün gestrichelt) über die volle Griffhöhe,
   am Heft bündig mit der Klingenkontur.
3. Der Querschnitt meldet *Wand 2,0 / 3,0 mm* — über und unter der Angel
   bleibt Holz.
4. *3 – Griff* → *Ein-Stück-Griff*: dort steht die **Angelhöhe am Heft** bei
   6,5 mm (der verdeckte Ansatz), der Regler *Verdeckter Ansatz* ist nur bei
   dieser Bauart sichtbar.

**Ein Rohling mit abgesetztem Heft** (Choil, Parierstich) bekommt über
*Angel-Breite* einen eigenen Wert; der Stahl sitzt dann mittig auf der
Mittellinie der Klinge, ragt aber über die Angel hinaus. Das Programm
schreibt dann die Warnung in den Plan:

> Die Angel ist nur 14,0 mm hoch, die Klinge am Heft 26,0 mm: der Stahl ragt
> je 6,0 mm über die Angel hinaus …

**Der Absatz der Klinge (seit 0.4.0):** Beim Ein-Stück-Griff steckt nur ein
kurzer verdeckter Ansatz in der Tasche im Holz. Die Klinge muss am Heft auf
diese Höhe geschliffen werden — sonst steht sie als 26 mm hoher Stahlkeil vor
dem Griff und passt nicht in die Tasche. Seit 0.4.0 rechnet das Programm
diese Schleifung mit: die Klinge ist am Heft so hoch wie der Ansatz (6,5 mm)
und wächst über *Länge des Absatzes* (12 mm) auf die volle Klingenhöhe.
Die Arbeitsreihenfolge nennt den Schleifschritt, das 3D-Modell zeigt die
geschliffene Form, und FreeCAD verschmilzt Klinge und Angel sauber.

*Prüfen:* Bauart *Ein-Stück-Griff* → der Absatz der Klinge ist in der
3D-Ansicht deutlich schmaler als die Klinge, und im Textplan steht
„Absatz der Klinge schleifen". Bei zwei Schalen bleibt die Klinge am Heft
voll hoch — dort ist die Angel durchgehend und es wird nichts geschliffen.

### 5.3 Das Holz neben der Angel (ab 0.9.0)

Beim **Ein-Stück-Griff**, beim **Zwingengriff** und bei der **Bastion** liegt
die Angel in einer Nut im Holz. Bis 0.8.x war diese Nut im Griff **eine
Lücke von 3,6 mm** auf beiden Seiten der Angel: Das Holz fehlte dort
vollständig, der Griff war in der Mitte der Dicke auseinander.

Nachgemessen am Standardmaß (Bastion, 22 mm dick, Angel 4,5 mm):

| Bauart | Holzvolumen bis 0.8.x | ab 0.9.0 |
|---|---|---|
| Ein-Stück-Griff | 34.456 mm³ | 53.755 mm³ |
| Bastion | 30.980 mm³ | 49.115 mm³ |
| Zwinge | 36.405 mm³ | 58.528 mm³ |

Das Holz ist jetzt **ein einziger Körper** statt drei, und es fehlt nichts
mehr. Der Selbsttest prüft das über das Volumen, der Modeltest prüft, dass
das Netz **geschlossen** ist (jede Kante gehört zu genau zwei Dreiecken).

**Der Zuschnitt war davon betroffen:** Bei der Bastion wurde das Holz wie
zwei Schalen gerechnet (2 × 8,8 mm statt 22 mm) — der Rohling wäre rund
6,5 mm zu dünn gewesen. Jetzt ist der Bastion-Block ein **Block in voller
Dicke**, und die Dokumente sagen das auch: Textplan, Bauplan, DXF und
Druckschablone nennen bei zwei Schalen „2 Stück", sonst einen Block.

*Prüfen:* Bauart *Bastion* → der Textplan sagt
„Zuschnitt: Holzblock 108 × 35 × 24 mm", und im DXF (Layer *SCHALE*) steht
**eine** Kontur, nicht zwei. Beim Ein-Stück- und beim Zwingengriff steht im
DXF zusätzlich eine *AUSSPA*-Bahn — die Frässbahn für die Angel.

### 5.4 Das Gewicht (ab 0.9.0)

Das Gewicht im Plan wird jetzt **aus dem Modell gemessen**, nicht aus der
Kontur gerechnet. Vorher war der Griff bei der Bastion rund **30 % zu
leicht** (toter Zweig in der Rechnung) und beim Zwingengriff zu schwer, weil
die Zwinge als voller Klotz statt als Mantel gerechnet wurde.

*Prüfen:* Der Textplan nennt „Holz … g". Das muss zur Größe des Rohlings
passen: ein Block aus 24 mm dickem Holz wiegt deutlich mehr als zwei
Schalen aus 11 mm.

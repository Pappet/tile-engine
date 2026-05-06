# Tile Engine – Umsetzungsplan

> Operativer Plan zur Implementierung gemäss ARCHITEKTUR.md. Gliedert die
> Arbeit in klar abgegrenzte Pakete mit Inputs, Outputs, Akzeptanzkriterien
> und Abhängigkeiten. Pakete sollen einzeln an Agenten delegierbar sein.

**Voraussetzung:** Implementierung folgt der Spezifikation in
`ARCHITEKTUR.md`. Der Plan referenziert dort Sektionen, statt Inhalte zu
duplizieren.

---

## Lese-Hinweis für Agenten

Jedes Arbeitspaket folgt diesem Schema:

- **Ziel** – ein Satz, was am Ende existieren soll.
- **Bibel-Referenz** – relevante ARCHITEKTUR.md-Sektionen.
- **Inputs** – welche Pakete müssen vorher fertig sein.
- **Liefergegenstände** – was im Repo neu/verändert sein soll.
- **Akzeptanzkriterien** – wann ist das Paket "fertig"?
- **Out-of-Scope** – was bewusst nicht Teil des Pakets ist.

Pakete dürfen nicht über ihren Scope hinausgehen. Wenn beim Bauen klar wird,
dass etwas fehlt, wird es als Issue dokumentiert, nicht stillschweigend
hinzugefügt.

---

## Workspace-Struktur

Vor der ersten Arbeit liegt diese Crate-Struktur fest:

```
tile-engine/
├── Cargo.toml                  (workspace root)
├── ARCHITEKTUR.md
├── PLAN.md                     (dieses Dokument)
├── README.md
├── crates/
│   ├── core/                   (Datenmodell, Koordinaten, World-API)
│   ├── sim-fluid/              (Fluid-System)
│   ├── sim-reaction/           (Reaktions-System)
│   ├── sim-thermal/            (Wärmediffusion – später)
│   ├── worldgen-api/           (Stage-Traits, Stage-Resources)
│   ├── worldgen-earthlike/     (Standard-Welttyp)
│   ├── render-bevy/            (Renderer-Plugin)
│   ├── debug-ui/               (Debug-Overlay – oberste Schicht, sieht alles)
│   ├── persistence/            (Save/Load)
│   └── app/                    (Binary, fügt Plugins zusammen)
└── data/                       (RON/TOML-Definitionen)
    ├── materials.ron
    ├── liquids.ron
    └── reactions.ron
```

Crates können später hinzugefügt werden (`worldgen-avatar`, `sim-erosion`
etc.). Der Plan baut diese Struktur schrittweise auf.

---

## Phase 0 – Fundament

### P0.1 – Workspace-Setup

**Ziel:** Lauffähiges Cargo-Workspace mit leerem Binary.

**Bibel-Referenz:** Sektion 2 (Tech-Stack), Sektion 3 (Schichten).

**Inputs:** Keine.

**Liefergegenstände:**
- `Cargo.toml` als Workspace-Root mit `members`-Liste.
- Alle Crates aus der Workspace-Struktur als leere Library-Crates angelegt
  (mit `lib.rs` der mindestens den Crate-Namen kommentiert).
- `crates/app` als Binary-Crate, das ein leeres Bevy-`App` startet, ein
  Window öffnet, Update-Loop läuft.
- `.gitignore`, `rust-toolchain.toml` (stable), CI-Skelett (optional).
- `README.md` mit Build-Anweisungen.

**Akzeptanzkriterien:**
- `cargo build --workspace` ohne Warnings.
- `cargo run -p app` öffnet ein Window mit Bevy-Default.
- Bevy-Version ist explizit gepinnt (eine konkrete `0.x.y`).

**Out-of-Scope:** Inhalte der Crates, Renderer-Setup, Worldgen.

---

### P0.2 – Profiling-Integration

**Ziel:** Profiling-Hooks von Anfang an verfügbar.

**Bibel-Referenz:** Sektion 16 (Designprinzipien, "Profiling früh").

**Inputs:** P0.1.

**Liefergegenstände:**
- `puffin` oder `tracy` als optionales Feature im Workspace.
- Feature-Flag `profile` in app-Crate.
- Mindestens ein Profile-Frame-Marker im Update-Loop.

**Akzeptanzkriterien:**
- `cargo run -p app --features profile` startet mit aktivem Profiler.
- Eine Profiler-UI (Puffin Viewer / Tracy) zeigt mindestens den
  Update-Frame.

**Out-of-Scope:** Profiling spezifischer Sim-Systeme (kommt mit den Systemen
selbst).

---

## Phase 1 – Core-Datenmodell

### P1.1 – Koordinaten-Modul

**Ziel:** `WorldPos`, `ChunkCoord`, `LocalPos` mit Konvertierungs-API.

**Bibel-Referenz:** Sektion 5.

**Inputs:** P0.1.

**Liefergegenstände:**
- In `crates/core`: Modul `coords` mit den drei Typen.
- Konstanten `CHUNK_SIZE`, `CHUNK_AREA`, `CHUNK_SHIFT`, `CHUNK_MASK`.
- Konvertierungs-Methoden (`split`, `index`, Reverse-Konstruktion).
- `serde::Serialize`/`Deserialize` für alle drei Typen.
- `proptest`-basierte Roundtrip-Tests gemäss Bibel-Sektion 5.

**Akzeptanzkriterien:**
- Roundtrip-Tests bestehen für positive und negative Koordinaten,
  inklusive Grenzfälle (`-1`, `0`, `CHUNK_SIZE-1`, `CHUNK_SIZE`,
  `i32::MIN+1`, `i32::MAX`).
- `cargo test -p core` grün.
- Modul ist in der Crate-API exportiert.

**Out-of-Scope:** ChunkData, World-Resource.

---

### P1.2 – Material-Registry

**Ziel:** `MaterialId`, `Material`, `MaterialRegistry` als Bevy-Resource.

**Bibel-Referenz:** Sektion 4.5.

**Inputs:** P1.1.

**Liefergegenstände:**
- `MaterialId(u16)` und `MaterialFlags` (bitflags).
- `Material`-Struct gemäss Bibel.
- `MaterialRegistry` als Bevy-`Resource` mit Add/Get-API.
- Reservierte Konstanten: `MAT_AIR = MaterialId(0)`.
- `ElementId`-Typ als Stub (Enum oder u8) – konkrete Element-Liste erstmal
  Platzhalter (H, C, O, Fe, Si, S genügen).
- Testmaterialien als statische Definitionen: Granit, Sandstein, Sand, Air,
  Eis, Basalt. (Genug für spätere Reaktions-Tests.)
- Unit-Tests: Add → Get-Roundtrip, AIR-Konstante korrekt.

**Akzeptanzkriterien:**
- Registry kann mit den sechs Testmaterialien gefüllt werden.
- `Material`/`MaterialFlags`/`MaterialId` sind serde-fähig.
- `cargo test -p core` grün.

**Out-of-Scope:** Materialien aus RON-Files laden (kommt in P11).
Composition-basierte Queries (kommen mit Reaktions-System).

---

### P1.3 – ChunkData-Komponente (minimal)

**Ziel:** Erste Version von `ChunkData` als Bevy-`Component` – nur Terrain.

**Bibel-Referenz:** Sektion 4.3, 4.5, 9.9 (Felder werden später ergänzt).

**Inputs:** P1.1, P1.2.

**Liefergegenstände:**
- `ChunkData`-Struct mit `coord` und `terrain: Box<[MaterialId; CHUNK_AREA]>`.
- `dirty: bool` Feld (für späteres Save/Load).
- Konstruktor `ChunkData::new_filled(coord, MaterialId)`.
- `serde::Serialize`/`Deserialize` mit `#[serde(skip)]` für künftige
  Derived-Felder.
- Box-Allokation testen (kein Stack-Overflow bei Konstruktion).

**Akzeptanzkriterien:**
- `cargo test -p core` grün, inkl. Test, der eine ChunkData mit Granit füllt
  und Stichproben prüft.
- Speicherbedarf eines `ChunkData` lässt sich in einem Test loggen
  (Sanity-Check: ~2 KB für Terrain-only).

**Out-of-Scope:** Liquid-Felder, Stains, Activity – kommt schrittweise.

---

### P1.4 – World-Resource und WorldAccess

**Ziel:** Welt-Container und ergonomische Zugriffs-API als SystemParams.

**Bibel-Referenz:** Sektion 6.

**Inputs:** P1.3.

**Liefergegenstände:**
- `World`-Resource gemäss Bibel-Sektion 6.1.
- `WorldAccess` und `WorldAccessMut` als `SystemParam`.
- Methoden: `get_tile`, `set_tile`, `get_chunk`, `ensure_chunk`.
- `Tile`-Aggregations-Typ für API-Bequemlichkeit.
- Caveat-Verhalten (commands.spawn deferred) im Doctest dokumentieren.
- Integrationstest im app-Crate: System ruft `set_tile` und `get_tile`
  über mehrere Chunks (auch negative Koordinaten).

**Akzeptanzkriterien:**
- Test schreibt 100 Tiles über mehrere Chunks, liest sie zurück, alle
  korrekt.
- `set_tile` setzt `chunk.dirty = true`.
- `ensure_chunk` legt neue Chunks korrekt an.

**Out-of-Scope:** Activity-System (P1.6), Bulk-API (das ist Sim-spezifisch).

---

### P1.5 – Tile-Aggregations-API mit deferred-Caveat handhaben

**Ziel:** Saubere Lösung für das deferred-Spawn-Problem in WorldAccessMut.

**Bibel-Referenz:** Sektion 6.2 (Caveat).

**Inputs:** P1.4.

**Liefergegenstände:**
- Entweder ein Pending-Buffer in WorldAccessMut, oder explizite Doku, dass
  ein Worldgen-System dem Sim-System vorausgeht.
- Test, der das deferred-Verhalten zeigt (set_tile in nicht-existentem
  Chunk → Lesen im selben System → Verhalten dokumentiert).

**Akzeptanzkriterien:**
- Verhalten ist deterministisch und dokumentiert.
- Mindestens ein Pattern (Pending-Buffer oder Pre-Pass) ist umgesetzt.

**Out-of-Scope:** Performance-Optimierung der Pending-Buffer.

---

### P1.6 – Activity-System

**Ziel:** SystemMask, Activity-Tracking, Decay-System.

**Bibel-Referenz:** Sektion 7.3, 8.4.

**Inputs:** P1.4.

**Liefergegenstände:**
- `SystemMask` als bitflags-Typ, alle Bits (FLUID, TEMPERATURE, EROSION,
  REACTION, GAS) reserviert.
- ChunkData um `active: SystemMask` und `last_active_tick: u64` ergänzt
  (mit `#[serde(skip)]`).
- `WakeRequests`-Resource (Stub, leer für Phase 1).
- Activity-Decay-System: nach N Ticks Inaktivität wird `active` geleert.
- Tick-Counter-System.
- Bevy-Plugin `CorePlugin`, das diese Systeme registriert.

**Akzeptanzkriterien:**
- Tests: aktiver Chunk bleibt aktiv solange `last_active_tick` aktualisiert
  wird, fällt nach N Ticks ohne Update raus.
- Plugin lässt sich in app-Crate einbinden, läuft.

**Out-of-Scope:** Wake-Logik mit echten Inhalten – nur Stub.

---

## Phase 2 – Renderer-Skeleton

### P2.1 – Statischer Tile-Renderer

**Ziel:** Eine Z-Ebene wird sichtbar dargestellt.

**Bibel-Referenz:** Sektion 3 (Schichten), Designprinzipien.

**Inputs:** P1.6.

**Liefergegenstände:**
- In `crates/render-bevy`: Plugin, das ChunkData-Entities in Sprite-/Tile-
  Darstellung rendert.
- Mapping `MaterialId → Farbe oder ASCII-Zeichen` (einfaches Lookup,
  Material-Registry erweitern um `display_color: [u8; 4]`).
- Aktuelle Z-Ebene als Resource (`ActiveZLayer(i32)`).
- Renderer iteriert nur Chunks der aktiven Z-Ebene.
- Demo in app-Crate: zwei Chunks mit Test-Pattern, sichtbar.

**Akzeptanzkriterien:**
- Beim Start ist ein Test-Pattern aus zwei Chunks sichtbar.
- Z-Ebene per Tastendruck wechselbar (auch wenn andere Ebenen leer sind).

**Out-of-Scope:** Liquid-Rendering, Lichteffekte, Camera-Steuerung jenseits
des Minimums.

---

### P2.2 – Camera & Z-Layer-Wechsel

**Ziel:** Spieler kann sich über die Welt bewegen und Z-Layer wechseln.

**Bibel-Referenz:** keine spezifische, ergibt sich aus Renderer-Architektur.

**Inputs:** P2.1.

**Liefergegenstände:**
- 2D-Kamera mit WASD/Pfeiltasten-Pan.
- Plus/Minus für Z-Layer.
- Optional: Maus-Zoom.

**Akzeptanzkriterien:**
- Kamera-Bewegung flüssig, keine Renderer-Glitches an Chunk-Grenzen.
- Z-Wechsel ändert sichtbares Pattern (testbar mit Setup, das pro Z-Ebene
  ein anderes Material legt).

**Out-of-Scope:** UI, Minimap, sonstiges Frontend.

---

## Phase 3 – Mini-Worldgen

### P3.1 – Worldgen-API-Crate

**Ziel:** Stage-Set, Stage-Resource-Konventionen, gemeinsame Worldgen-Types.

**Bibel-Referenz:** Sektion 12.4, 12.7, 12.10.

**Inputs:** P1.4.

**Liefergegenstände:**
- `WorldgenSet`-Enum (Maps/Structures/Chunks).
- Stub-Resources für die Standard-Maps (`Heightmap`, `ClimateMap`,
  `RiverGraph`, `StratigraphyMap`) – ohne Inhalt, nur Typen.
- `subseed`-Hash-Funktion gemäss Bibel.
- `WorldgenConfig`-Resource (Master-Seed, Welt-Grösse).
- Stub-Trait `SurfaceQuery` als Service-API.

**Akzeptanzkriterien:**
- Crate baut, exportiert genannte Typen.
- Hash-Funktion hat Determinismus-Test (gleicher Input → gleicher Output).

**Out-of-Scope:** Stages selbst implementieren – nur API.

---

### P3.2 – Earthlike Worldgen-Subset (Heightmap + Stratigraphie)

**Ziel:** Einfachste lauffähige Worldgen-Pipeline, die ChunkData füllt.

**Bibel-Referenz:** Sektion 12.3 (Stages 2 und 9), 13.1.

**Inputs:** P3.1.

**Liefergegenstände:**
- In `crates/worldgen-earthlike`: Plugin `EarthlikeWorldgenPlugin`.
- Stage-System `gen_heightmap`: füllt Heightmap-Resource mit Noise (Crate-
  Wahl: `noise` oder `fastnoise-lite`).
- Stage-System `gen_stratigraphy`: leitet einfache Säulen-Definition pro
  (cx, cy) ab.
- Stage-System `gen_chunks`: iteriert Welt-Chunks, erzeugt ChunkData
  basierend auf Heightmap + Stratigraphie.
- `SurfaceQuery`-Implementierung aus Heightmap.
- Demo in app-Crate: Plugin geladen, Renderer zeigt generierte Welt.

**Akzeptanzkriterien:**
- Welt mit z.B. 32×32 Chunks (1024×1024 Tiles) in <5 Sekunden generiert.
- Renderer zeigt erkennbare Heightmap (Höhenlinien sichtbar in Z-Ebenen).
- Determinismus-Test: gleicher Seed → bitidentische ChunkData.

**Out-of-Scope:** Klima, Hydrologie, Höhlen, Erze (alles spätere Pakete).

---

### P3.3 – WorldStructures-Resource (leer)

**Ziel:** Datenmodell für deklarative Welt-Strukturen vorbereiten.

**Bibel-Referenz:** Sektion 12.5.

**Inputs:** P3.1.

**Liefergegenstände:**
- `WorldStructures`-Resource.
- Typen: `CaveStructure`, `OreVein` (Felder gemäss Bibel: deklarative
  Daten, keine Closures).
- `SpatialIndex`-Stub (R-Tree-Crate-Wahl: `rstar` oder eigener Bucket-Index).
- Marker-Test, dass Strukturen serde-fähig sind.

**Akzeptanzkriterien:**
- Resource lässt sich anlegen, ist leer.
- Stub-Funktion `structures_intersecting_chunk` existiert (kann erstmal
  leere Liste zurückgeben).

**Out-of-Scope:** Höhlen-Generierung (kommt in P9).

---

## Phase 4 – Fluid-System

### P4.1 – LiquidRegistry

**Ziel:** Liquid-Definitionen als Daten-Tabelle.

**Bibel-Referenz:** Sektion 9.2.

**Inputs:** P1.2.

**Liefergegenstände:**
- In `crates/sim-fluid`: `LiquidId`, `LiquidProperties`, `LiquidFlags`,
  `LiquidRegistry`.
- Reservierte Konstanten: `LIQ_NONE`.
- Test-Liquids als statische Definitionen: Water, Magma, Blood, Oil, Acid
  (gemäss Bibel-Tabelle).
- `GasId`-Stub (nur Typ, keine Logik).

**Akzeptanzkriterien:**
- Registry kann mit fünf Test-Liquids gefüllt werden.
- Serde-fähig, `cargo test` grün.

**Out-of-Scope:** Verhalten implementieren.

---

### P4.2 – ChunkData-Erweiterung um Liquid-Felder

**Ziel:** Liquid-SoA-Felder in ChunkData ergänzen.

**Bibel-Referenz:** Sektion 9.9.

**Inputs:** P1.3, P4.1.

**Liefergegenstände:**
- ChunkData um `liquid_kind`, `liquid_amount_read/write`,
  `liquid_temp_read/write`, `pressure_read/write` erweitern.
- `#[serde(skip)]` an Write-Buffern.
- `swap_buffers`-Methode.
- Speicher-Sanity-Test (ChunkData ≤ 20 KB).

**Akzeptanzkriterien:**
- Bestehende Tests grün.
- Save/Load-Roundtrip-Test (manuelles bincode encode/decode) erhält
  read-Buffer, ignoriert write-Buffer.

**Out-of-Scope:** Sim-Logik, Stains.

---

### P4.3 – Single-Chunk-Fluid-CA

**Ziel:** Wasser fliesst innerhalb eines Chunks. Noch kein Cross-Chunk.

**Bibel-Referenz:** Sektion 9.6, 9.12 (vereinfacht).

**Inputs:** P4.2.

**Liefergegenstände:**
- `FluidPlugin` mit System `fluid_step_local`.
- Horizontaler Fluss zwischen direkten Tile-Nachbarn (Bilanzgleichung).
- Vertikaler Fall innerhalb des Chunks (eine Z-Ebene).
- Buffer-Swap im PostTick.
- Demo: Block aus Wasser oben in einem Chunk, fliesst nach unten und seitwärts.

**Akzeptanzkriterien:**
- Sichtbares Fluid-Verhalten im Renderer.
- Massenerhaltung in Tests: Summe aller `liquid_amount` in einem
  geschlossenen Chunk ändert sich nicht (Toleranz: 0).
- Determinismus-Test: gleicher Anfangszustand → gleicher Verlauf.

**Out-of-Scope:** Cross-Chunk, Druck, Viskosität-Variation, Magma/Blut.

---

### P4.4 – Snapshot-Resource

**Ziel:** Cross-Chunk-Reads über Snapshot-Pattern ermöglichen.

**Bibel-Referenz:** Sektion 8.2.

**Inputs:** P4.3.

**Liefergegenstände:**
- `LiquidSnapshot`-Resource.
- System `snapshot_liquid` im PreTick: kopiert read-Buffer aller aktiven
  Chunks.
- Cross-Chunk-Read-Helper (gegeben WorldPos → liest aus Snapshot).
- Wake-Buffer für Aktivierung von Nachbar-Chunks.

**Akzeptanzkriterien:**
- Test: zwei aneinander grenzende Chunks, Wasser läuft sichtbar von einem
  zum anderen.
- Massenerhaltung über Chunks hinweg.

**Out-of-Scope:** Snapshot-Optimierung (nur Border-Tiles).

---

### P4.5 – Vertikale Schichtung

**Ziel:** Z-Fall mit Dichte-Vergleich.

**Bibel-Referenz:** Sektion 9.5.

**Inputs:** P4.4.

**Liefergegenstände:**
- System `liquid_vertical_flow` mit Dichte-Vergleich.
- Korrekte Behandlung: Liquid fällt in Air-Tile durch (kein Schichtung-
  Vergleich nötig); zwischen zwei verschiedenen Liquids erfolgt
  Dichte-Tausch.

**Akzeptanzkriterien:**
- Test: Öl auf Wasser bildet sichtbare Schichtung über mehrere Z-Ebenen.
- Test: Wasser fällt durch Luft korrekt (kein Hänger an Z-Grenzen).

**Out-of-Scope:** Magma+Wasser-Reaktion (kommt mit Reaktions-System).

---

### P4.6 – Druckmodell (Option 2)

**Ziel:** U-Rohre funktionieren.

**Bibel-Referenz:** Sektion 9.7.

**Inputs:** P4.5.

**Liefergegenstände:**
- Druckpropagation im PreTick (Druck-CA gemäss Bibel).
- `flow_with_pressure` ersetzt `flow_between` in der Sim.
- Test-Setup: U-Rohr aus zwei Becken + verbindendem unteren Tile.

**Akzeptanzkriterien:**
- U-Rohr-Test: nach N Ticks haben beide Seiten ähnliche Höhe (innerhalb
  einer Toleranz von 1).
- Wasserstand in geschlossenem Becken steigt auf Quell-Niveau.

**Out-of-Scope:** Option 3 (BFS), Performance-Optimierung der
Druckpropagation.

---

### P4.7 – LiquidSource und LiquidDrain

**Ziel:** Quellen und Senken als Entities.

**Bibel-Referenz:** Sektion 9.8.

**Inputs:** P4.6.

**Liefergegenstände:**
- `LiquidSource`- und `LiquidDrain`-Components.
- Systeme, die diese Components in PreTick auswerten und Liquid in/aus
  Tiles bringen.
- `LiquidFilter`-Typ für Drain (alle / nach Liquid-Flags filtern).
- Demo: Bach-Quelle füllt langsam ein Becken, Drain leert es.

**Akzeptanzkriterien:**
- Source mit `rate=2` produziert über 100 Ticks 200 Einheiten (modulo
  max_pressure).
- Drain mit `rate=1` reduziert sichtbar.
- Source und Drain serde-fähig.

**Out-of-Scope:** Worldgen-Hydrologie-Anbindung.

---

### P4.8 – Multiple Liquid-Typen

**Ziel:** Magma und Öl funktionieren in der Sim.

**Bibel-Referenz:** Sektion 9.6 (Viskosität), 9.5 (Dichte).

**Inputs:** P4.7.

**Liefergegenstände:**
- Test-Welt mit Magma-Quelle und Öl-Quelle.
- Verifizierung, dass Viskosität korrekt skaliert (Magma kriecht, Öl
  fliesst mittel).
- Verifizierung, dass Magma sichtbar leuchtet (Renderer-Erweiterung um
  `emits_light`).

**Akzeptanzkriterien:**
- Akzeptanz-Test 3 (Bibel 9.13): Öl auf Wasser bildet stabile Schicht.
- Visueller Vergleich: Magma fliesst sichtbar langsamer als Wasser.

**Out-of-Scope:** Magma+Wasser-Kollision (braucht Reaktions-System).

---

## Phase 5 – Save/Load (Minimal)

### P5.1 – Save-Header und Verzeichnis-Layout

**Ziel:** Grundgerüst für Save-Files.

**Bibel-Referenz:** Sektion 11.6, 11.7.

**Inputs:** P4.8.

**Liefergegenstände:**
- In `crates/persistence`: `SaveHeader`-Typ, Magic-Bytes, Versions-Konstante.
- Funktionen: `write_header`, `read_header`, `validate_header`.
- Verzeichnis-Manager: legt `chunks/`, `worldgen/` etc. an.

**Akzeptanzkriterien:**
- Header-Roundtrip-Test.
- Korrupte Magic-Bytes werden mit klarem Fehler abgelehnt.

**Out-of-Scope:** Eigentliche Daten speichern.

---

### P5.2 – Chunks und World speichern/laden

**Ziel:** Welt-Zustand persistieren und wiederherstellen.

**Bibel-Referenz:** Sektion 11.1, 11.3, 11.7.

**Inputs:** P5.1.

**Liefergegenstände:**
- Funktion `save_world(slot)` schreibt:
  - Header
  - World-Resource (chunk-mapping, current_tick)
  - Alle dirty Chunks als einzelne Files
  - Worldgen-Maps
  - WorldStructures
- Funktion `load_world(slot)` rekonstruiert:
  - Worldgen-Maps zurückladen
  - Worldgen-Stage `gen_chunks` für nicht-dirty Chunks aufrufen
  - Dirty Chunks aus Files laden
  - World-Resource zurücksetzen
- Save/Load via Bevy-Events (`SaveRequest`/`LoadRequest`).

**Akzeptanzkriterien:**
- Akzeptanz-Test 1 (Bibel 11.15): Save → Load → Save erzeugt
  bitidentische Files.
- Akzeptanz-Test 2: Determinismus nach Load – N Ticks weiterlaufen
  ergibt identisches Resultat zur ungespeicherten Variante.
- Atomare Schreibvorgänge: bei Crash mid-save bleibt alter Stand intakt.

**Out-of-Scope:** Entity-Save (P5.3), Versions-Migration (später),
Auto-Save (später), Async (später).

---

### P5.3 – Entity-Persistenz

**Ziel:** LiquidSource/LiquidDrain überleben Save/Load.

**Bibel-Referenz:** Sektion 11.8.

**Inputs:** P5.2.

**Liefergegenstände:**
- `Persistent`-Trait gemäss Bibel.
- Implementierungen für `LiquidSource`, `LiquidDrain`.
- Entity-Serializer/-Deserializer.
- Erweiterung von `save_world`/`load_world` um Entities.

**Akzeptanzkriterien:**
- Save mit aktiven Sources/Drains laden → Sources/Drains funktionieren
  weiter.
- Render-Components werden nicht gespeichert (Test: Save enthält keine
  Sprite-Daten).

**Out-of-Scope:** Reaktions-Cooldowns (kommt nach Reaktions-System).

---

## Phase 6 – Reaktions-System

### P6.1 – ReactionRegistry-Skelett

**Ziel:** Datenmodell und Indexing.

**Bibel-Referenz:** Sektion 10.2, 10.3.

**Inputs:** P4.1.

**Liefergegenstände:**
- In `crates/sim-reaction`: `ReactionId`, `Reaction`, `ReactionRegistry`.
- Trigger/Condition/Effect-Enums (komplett gemäss Bibel 10.4-10.6).
- Index-Aufbau: `by_trigger`, `by_material`, `by_liquid`.
- Test: Registry mit 3 Test-Reaktionen, Indexe korrekt.

**Akzeptanzkriterien:**
- Registry serde-fähig.
- Index-Lookup-Test: gegeben MaterialId, korrekte Kandidatenliste.

**Out-of-Scope:** Resolver-Logik.

---

### P6.2 – Periodic-Trigger-Resolver

**Ziel:** Erste Reaktionen laufen (Wasser→Eis, Eisenerz-Schmelze).

**Bibel-Referenz:** Sektion 10.8, 10.9, 10.10.

**Inputs:** P6.1.

**Liefergegenstände:**
- System `periodic_reaction_system`.
- Condition-Evaluator (alle atomaren Bedingungen + Boolean-Operatoren).
- Effect-Applier (alle SetTile-/SetLiquid-/AddTemperature-Effects).
- `PendingEffects`-Buffer für deferred Apply.
- Determinismus-Hash gemäss Bibel.
- Test-Reaktionen: Wasser→Eis, Eisenerz→Magma.

**Akzeptanzkriterien:**
- Akzeptanz-Test 1, 2 (Bibel 10.13): Eisenerz-Schmelze und Wasser-Eis-
  Zyklus laufen ohne Code-Änderung, nur durch Registry-Einträge.
- Determinismus-Test: gleicher Seed/Tick → gleiches Resultat.

**Out-of-Scope:** Andere Trigger, komplexe Effects (Propagate, Area).

---

### P6.3 – LiquidCollision-Trigger

**Ziel:** Magma+Wasser=Verdampfung funktioniert.

**Bibel-Referenz:** Sektion 9.4, 10.4.

**Inputs:** P6.2.

**Liefergegenstände:**
- Trigger-Hook im Fluid-System: bei zwei verschiedenen Liquids → Reaktions-
  Resolver aufrufen.
- Test-Reaktion Magma+Wasser gemäss Bibel-Beispiel.
- `IncomingLiquidIs`-Condition korrekt ausgewertet.

**Akzeptanzkriterien:**
- Akzeptanz-Test (Bibel 9.13 Test 1): Magma trifft Wasser → Wasser weg,
  Magma erstarrt zu Basalt nach Abkühlung.

**Out-of-Scope:** Gas-Erzeugung (Effect existiert, aber kein Gas-System
nimmt Output entgegen – wird im Test ignoriert).

---

### P6.4 – Komplexere Effects

**Ziel:** PropagateToNeighbor und AreaEffect funktionieren.

**Bibel-Referenz:** Sektion 10.6.

**Inputs:** P6.3.

**Liefergegenstände:**
- Effect-Applier-Erweiterung um Propagate und AreaEffect.
- Test-Reaktion: Säure-Korrosion (gemäss Bibel-Beispiel).

**Akzeptanzkriterien:**
- Säure auf Marmor-Block: Marmor erodiert sichtbar über Zeit.

**Out-of-Scope:** SpawnEntity-Logik (braucht Entity-Type-System – kommt
mit Items/Kreaturen).

---

### P6.5 – Reaktions-Cooldowns persistieren

**Ziel:** Save/Load erweitern um Cooldown-State.

**Bibel-Referenz:** Sektion 11.9.

**Inputs:** P5.3, P6.4.

**Liefergegenstände:**
- `ReactionCooldowns`-Resource.
- Resolver respektiert Cooldowns.
- Save/Load erweitert um Cooldowns.

**Akzeptanzkriterien:**
- Reaktion mit Cooldown auslöst, Save mit aktivem Cooldown, Load → Reaktion
  triggert nicht sofort, sondern erst nach verbleibender Cooldown-Zeit.

**Out-of-Scope:** External-Trigger-Reaktionen.

---

## Phase 7 – Stains und Wake-Propagation

### P7.1 – Stain-System

**Ziel:** STAINS-Liquids hinterlassen Spuren.

**Bibel-Referenz:** Sektion 9.11.

**Inputs:** P6.4.

**Liefergegenstände:**
- ChunkData um `stain_kind`, `stain_amount` ergänzt.
- System `stain_apply`: bei Liquid-Abfluss aus Tile mit STAINS-Flag,
  Stain hinterlassen.
- System `stain_decay` (selten, alle 100 Ticks).
- Renderer-Erweiterung: Stains farbig darstellen.

**Akzeptanzkriterien:**
- Blut fliesst durch Tunnel, hinterlässt sichtbare Spur.
- Spur klingt über Zeit ab.

**Out-of-Scope:** SACRED-Stain-Reaktionen (kommt als Test in P7.3).

---

### P7.2 – Wake-Propagation real

**Ziel:** Inaktive Chunks werden korrekt durch Cross-Chunk-Effekte aktiviert.

**Bibel-Referenz:** Sektion 8.4.

**Inputs:** P4.4.

**Liefergegenstände:**
- WakeRequests-Resource korrekt befüllen aus Fluid-System (bei Cross-Chunk-
  Flow oder Stain-Setzen).
- `process_wakes`-System im PostTick.

**Akzeptanzkriterien:**
- Test: zwei Chunks, Chunk A aktiv mit Wasser, Chunk B inaktiv. Wasser
  läuft rüber, Chunk B wird aktiv (active-Bit, last_active_tick).

**Out-of-Scope:** Wake-Optimierung.

---

### P7.3 – Khorne-Altar-Akzeptanztest

**Ziel:** Vollständiger Lackmustest des Sim-Stacks.

**Bibel-Referenz:** Sektion 9.13 (Test 2).

**Inputs:** P7.1, P7.2, P6.4.

**Liefergegenstände:**
- Test-Setup: Altar-Entity (LiquidSource für SACRED-Blut), Tunnel-Tiles,
  Sammelraum.
- SACRED-Stain-Reaktion (Beispiel aus Bibel: Dämon-Spawn ist optional, da
  Entity-Spawn-Effect noch nicht voll umgesetzt – Reduktion auf
  `EmitEvent`).
- Verifizierungs-Test als integrierter Run.

**Akzeptanzkriterien:**
- Blut fliesst zäh durch Kanäle, Stains entlang des Wegs.
- Sammelraum-Pegel steigt auf Quell-Niveau (Druck-Test).
- Visueller Eindruck stimmt mit Erwartung überein.

**Out-of-Scope:** Tatsächliches Dämon-Spawnen.

---

## Phase 8 – Worldgen-Tiefe

### P8.1 – Hydrologie

**Ziel:** Drainage-Graph, Flüsse, automatische LiquidSources.

**Bibel-Referenz:** Sektion 12.3 (Stage 7), Sektion 9.8.

**Inputs:** P3.2, P4.7.

**Liefergegenstände:**
- Stage `gen_hydrology`: Drainage-Akkumulation auf Heightmap.
- `RiverGraph`-Resource gefüllt.
- Stage `gen_river_sources`: erzeugt LiquidSource-Entities an Quellen.

**Akzeptanzkriterien:**
- Generierte Welt zeigt sichtbare Flüsse, die korrekt nach unten fliessen.
- Determinismus-Test mit Hydrologie.

**Out-of-Scope:** Erosions-Rückkopplung (P9.3).

---

### P8.2 – Höhlen-Worldgen

**Ziel:** Höhlen über Region und Z-Ebenen hinweg.

**Bibel-Referenz:** Sektion 12.5.

**Inputs:** P3.3.

**Liefergegenstände:**
- Stage `gen_caves`: füllt `WorldStructures.caves` mit CA-basierten Cluster
  und Worm-Gängen.
- Spatial-Index aufgebaut.
- `gen_chunks` erweitert: berücksichtigt Höhlen aus WorldStructures.

**Akzeptanzkriterien:**
- Generierte Welt hat sichtbare Höhlen über mehrere Chunks und Z-Ebenen
  (renderer kann durch Z-Ebenen scrollen, Höhlen sind erkennbar).
- Keine Nähte an Chunk-Grenzen.

**Out-of-Scope:** Erzadern (P8.3), Strukturen wie Ruinen.

---

### P8.3 – Erzvorkommen

**Ziel:** Erze gemäss geologischer Logik platziert.

**Bibel-Referenz:** Sektion 12.3 (Stage 12).

**Inputs:** P8.2.

**Liefergegenstände:**
- Material-Registry um Erz-Materialien erweitert (Hämatit, Magnetit,
  Pyrit, Gold, etc.) – mindestens 5 Erze.
- Stage `gen_ore_veins`: platziert Veins basierend auf Stratigraphie und
  (falls vorhanden) Tektonik-Daten – für MVP einfache Heuristik.
- `gen_chunks` materialisiert Veins in ChunkData.

**Akzeptanzkriterien:**
- Eisen-Erze in höheren Schichten, "magmatische" Erze in der Nähe von
  Granit (auch wenn Granit in MVP-Stratigraphie nur grob platziert).
- Determinismus-Test.

**Out-of-Scope:** Tektonik-Stage (sehr aufwendig, später).

---

## Phase 9 – Erosion und geologische Zeit

### P9.1 – Sediment-Tracking

**Ziel:** Lose Auflage als zusätzliches ChunkData-Feld.

**Bibel-Referenz:** Sektion 1 (Vision), nicht detailliert in Bibel
ausspezifiziert – Pakete erzeugt das Detail.

**Inputs:** P4.8.

**Liefergegenstände:**
- ChunkData um `sediment: Box<[u8; CHUNK_AREA]>` erweitert.
- Material-Registry um Sediment-Eigenschaften erweitert (oder eigenes
  Sediment-Modell – Architektur-Entscheidung im Paket).
- Update der ARCHITEKTUR.md Sektion 9.9.

**Akzeptanzkriterien:**
- Sediment-Feld serde-fähig.
- Visualisierung im Renderer (z.B. Höhe der Sediment-Schicht erkennbar).

**Out-of-Scope:** Erosionsmechanik selbst.

---

### P9.2 – Erosions-System

**Ziel:** Strömung erodiert weiches Material, lagert Sediment ab.

**Bibel-Referenz:** Sektion 7.2 (Multi-Rate, Erosion alle 100 Ticks).

**Inputs:** P9.1.

**Liefergegenstände:**
- System `erosion_system`, läuft im 100-Tick-Takt.
- Erosion-Formel: pro Tile mit Liquid-Geschwindigkeit über Schwelle und
  weichem Untergrund (`erodibility` aus Material-Registry).
- Sediment-Aufnahme/-Ablagerung gemäss Strömung-Kapazität.

**Akzeptanzkriterien:**
- Bach in Sand-Tile gräbt sich über Spielzeit messbar ein.
- Massenerhaltung: erodiertes Material wird nicht "vergessen", sondern als
  Sediment transportiert.

**Out-of-Scope:** Lithifizierung (P9.3).

---

### P9.3 – Lithifizierung als Reaktion

**Ziel:** Sediment wird unter Druck zu Stein.

**Bibel-Referenz:** Sektion 10.7 (Beispiel sediment_lithifies).

**Inputs:** P9.2, P6.4.

**Liefergegenstände:**
- Reaktions-Eintrag (gemäss Bibel-Beispiel).
- `PressureAtLeast`-Condition korrekt ausgewertet (vertikaler Stein-Druck
  als Tile-Pressure, nicht Liquid-Pressure – ggf. neue Resource oder
  Berechnung pro Chunk im Worldgen).

**Akzeptanzkriterien:**
- Test mit langer Spielzeit + Sediment-Akkumulation: Sediment wandelt sich
  zu Sandstein.

**Out-of-Scope:** Echte geologische Spielzeit (1 Tick = wie viele Sekunden
ist offene TODO-Frage).

---

## Phase 10 – Mechanik

### P10.1 – Pumpen-Entity

**Ziel:** Aktives Heben von Liquid um eine Z-Ebene.

**Bibel-Referenz:** Sektion 9.13 Test 5.

**Inputs:** P4.7.

**Liefergegenstände:**
- `Pump`-Component (Drain unten + Source oben + Energie-Bedarf-Stub).
- System `pump_system`: bei aktiver Pumpe Liquid von unten nach oben
  bewegen.
- Renderer zeigt Pumpen-Tile.

**Akzeptanzkriterien:**
- Akzeptanz-Test 5 (Bibel 9.13): Pumpe hebt Wasser ein Niveau, Wasser
  fliesst weiter durch Druck.

**Out-of-Scope:** Energie-Verteilung (eigenes Subsystem später).

---

### P10.2 – Wasserrad / Mechanik-Antrieb

**Ziel:** Strömung erzeugt mechanische Energie.

**Bibel-Referenz:** Vision (mechanische Komponente DF-Stil).

**Inputs:** P10.1.

**Liefergegenstände:**
- `Waterwheel`-Component und System.
- Mechanische-Energie-Resource oder -Component-Graph (Architektur-
  Entscheidung im Paket; wenn eigene Sektion in der Bibel erforderlich,
  zuerst dort spezifizieren).

**Akzeptanzkriterien:**
- Wasserrad in Bach erzeugt Energie-Output.
- Output an Pumpe gekoppelt → Pumpe arbeitet.

**Out-of-Scope:** Komplexe Mechanik-Netze, Zahnräder, Förderbänder.

---

## Phase 11 – Daten-Loading und Modding-Vorbereitung

### P11.1 – RON/TOML-Loader für Materialien und Liquids

**Ziel:** Registries aus Daten-Files füllen.

**Bibel-Referenz:** Sektion 10.12, Tech-Stack (RON/TOML).

**Inputs:** P1.2, P4.1.

**Liefergegenstände:**
- `data/materials.ron` mit allen bisherigen Test-Materialien.
- `data/liquids.ron` mit allen bisherigen Test-Liquids.
- Loader-System im Startup, das die Files parst und Registry füllt.
- Statische Definitionen aus den Crates entfernt.

**Akzeptanzkriterien:**
- App startet, Registries enthalten dieselben Inhalte wie vorher.
- Hinzufügen eines neuen Materials in der RON-Datei wird ohne Code-
  Änderung wirksam.

**Out-of-Scope:** Hot-Reload, schema-Validierung.

---

### P11.2 – RON-Loader für Reaktionen

**Ziel:** Reaktions-Registry aus Files.

**Bibel-Referenz:** Sektion 10.12.

**Inputs:** P11.1, P6.4.

**Liefergegenstände:**
- `data/reactions.ron` mit allen bisherigen Test-Reaktionen.
- Loader analog P11.1.
- Validation: Reaktionen referenzieren existierende MaterialIds/LiquidIds.

**Akzeptanzkriterien:**
- Alle bestehenden Reaktions-Tests grün ohne Code-Änderung.
- Neue Reaktion in RON-Datei wirkt sofort.

**Out-of-Scope:** Mod-Ordner-Scanning.

---

## Phase 13 – Debug-UI-Crate

Eigenständige Crate `crates/debug-ui/`, die oberhalb aller Sim-Schichten
sitzt und Zugriff auf alle Interna hat. Behebt die aktuelle Schichtverletzung
(`render-bevy` importiert `sim_reaction`).

**Schichtprinzip:** `debug-ui` darf von `tile_core`, `sim_fluid`,
`sim_reaction`, `sim_thermal`, `render_bevy` und `worldgen_api` abhängen –
aber keine dieser Crates darf von `debug-ui` abhängen.

---

### P13.1 – debug-ui Crate + Migration

**Ziel:** Bestehende Debug-UI aus `render-bevy` herauslösen, Schichtverletzung beseitigen.

**Bibel-Referenz:** Sektion 3 (Schichten).

**Inputs:** P6.3 (Reaktions-System vorhanden), P2.1 (Renderer vorhanden).

**Liefergegenstände:**
- Neue Crate `crates/debug-ui/` mit `DebugUiPlugin`.
- `render-bevy/src/debug.rs` Inhalt nach `debug-ui/src/lib.rs` migriert.
- `render-bevy` verliert Abhängigkeit auf `sim_reaction` und `tile_core::activity::WakeRequests` (nur noch Renderer-interne Typen wie `ActiveZLayer`).
- `ActiveZLayer` aus `render-bevy` re-exportiert oder in `tile_core` verschoben, damit `debug-ui` darauf zugreifen kann ohne zirkuläre Abhängigkeit.
- `app/Cargo.toml` ersetzt `render-bevy`-Debug-Feature durch `debug-ui`.
- Bestehende Debug-Fenster-Funktionalität bleibt erhalten (Performance, World, Sim Internals).

**Akzeptanzkriterien:**
- `cargo build --workspace` grün.
- `render-bevy/Cargo.toml` hat keine `sim_reaction`-Abhängigkeit mehr.
- Debug-Fenster zeigt dieselben Informationen wie vorher.
- `cargo test --workspace` grün.

**Out-of-Scope:** Neue Debug-Panels, Inspector, Overlays.

---

### P13.2 – Erweiterte Sim-Stats-Panels

**Ziel:** Detaillierte Laufzeit-Metriken aller Sim-Subsysteme einsehbar.

**Bibel-Referenz:** Sektion 7 (Sim-Set-Ordering), Sektion 9 (Fluid), Sektion 10 (Reaktionen).

**Inputs:** P13.1.

**Liefergegenstände:**
- Panel **Fluid**: Gesamtflüssigkeit nach Liquid-Typ (Summe `liquid_amount_read` pro `LiquidId`), aktive Chunks (mindestens 1 non-zero Tile), WakeRequests-Tiefe.
- Panel **Reactions**: `PendingEffects`-Tiefe, Reaktionen/Tick (gleitender Durchschnitt, z.B. über 60 Ticks).
- Panel **World**: bestehende Chunks/Entities/Tick-Anzeige, ergänzt um Dirty-Chunk-Zähler.
- Alle Panels als `ui.collapsing()`-Sektionen (standardmässig zugeklappt).
- Stats werden nur berechnet wenn Panel aufgeklappt (lazy evaluation via `CollapsingHeader::show_unindented` mit Guard).

**Akzeptanzkriterien:**
- Fluid-Summe bleibt konstant bei geschlossenem Becken (Massenerhaltungs-Sichtbarkeit).
- Dirty-Chunk-Zähler fällt auf 0 wenn Sim einfriert (keine Änderungen).
- Kein messbarer FPS-Einbruch bei zugeklappten Panels.

**Out-of-Scope:** Zeitreihengraphen (kommt in P13.4), Thermal-Stats (kein Thermal-System yet).

---

### P13.3 – Tile-Inspector

**Ziel:** Klick auf Tile zeigt alle Felder des Tiles im Debug-Panel.

**Bibel-Referenz:** Sektion 4.3 (ChunkData-Felder), Sektion 5 (Koordinaten).

**Inputs:** P13.1.

**Liefergegenstände:**
- Mausklick-System in `debug-ui`: Weltkoordinaten aus Cursor-Position + aktiver Z-Ebene berechnen (via `Camera`-`GlobalTransform`).
- `InspectedTile`-Resource (`Option<WorldPos>`) speichert zuletzt angeklickte Position.
- Inspector-Panel zeigt für angeklickte Tile:
  - `terrain: MaterialId` + Name aus `MaterialRegistry`
  - `liquid_kind: LiquidId` + Name + `liquid_amount_read`
  - `liquid_temp_read`, `pressure_read`
  - `temp`
  - `dirty`-Flag des Chunks
- Klick ausserhalb des Weltraums: Inspector leert sich.

**Akzeptanzkriterien:**
- Klick auf Magma-Tile zeigt `liquid_kind: LiquidId(2) "Magma"`, korrekte Amount.
- Klick auf Wand-Tile zeigt `terrain: MaterialId(1) "Granit"`, kein Liquid.
- Klick-System interferiert nicht mit der Sim (read-only).

**Out-of-Scope:** Multi-Tile-Selektion, Tile-Editierung via Inspector.

---

### P13.4 – Chunk- und Activity-Overlays

**Ziel:** Visuelle Overlays über den Tiles für Debugging von Sim-Verhalten.

**Bibel-Referenz:** Sektion 7.3 (Activity-System), Sektion 8.4 (Wake-Propagation).

**Inputs:** P13.1.

**Liefergegenstände:**
- Toggle-Resource `DebugOverlays { chunk_borders: bool, activity: bool, liquid_heatmap: bool }`.
- Tastenbelegung (z.B. F1/F2/F3) schaltet Overlays um.
- **Chunk-Borders-Overlay**: dünne farbige Linie entlang Chunk-Grenzen (egui-Painter oder eigene Bevy-Gizmo-Sprites).
- **Activity-Overlay**: aktive Chunks (per `SystemMask`) leicht eingefärbt (z.B. FLUID-aktiv = Blau-Tint).
- **Liquid-Heatmap**: `liquid_amount_read` als Farbintensität über dem normalen Tile-Render (additiv, semi-transparent).
- Overlays nur auf aktiver Z-Ebene gezeichnet.

**Akzeptanzkriterien:**
- Chunk-Border-Overlay zeigt korrekte 32×32-Gitter.
- Activity-Overlay zeigt Ausbreitung von Aktivität wenn Flüssigkeit fliesst (sichtbar durch Farbänderung).
- Alle drei Overlays kombinierbar aktiv ohne Abstürze.

**Out-of-Scope:** Performance-Profiling-Overlay (Puffin ist dafür), Zeitreihen-Graphen.

---



Diese Phase ist offen und wird nach Bedarf gefüllt. Mögliche Pakete:

- **Auto-Save mit Slot-Rotation** (Bibel 11.11).
- **Async-Save** (Bibel 11.12).
- **Save-Versions-Migration** (Bibel 11.6) – sobald erste Inkompatibilität
  in der Praxis auftritt.
- **Pathfinding** (Bibel TODO).
- **Wärmediffusion-System** (`crates/sim-thermal`).
- **Gas-System** (Bibel 9.10).
- **Worldgen-Erweiterungen** (Sonneneinstrahlung, Wind, Biome – Bibel 14).
- **Avatar-Floating-Islands-Worldgen** (Bibel 13.2) – als Erweiterungs-
  Lackmustest, dass die Engine wirklich welttyp-agnostisch ist.

---

## Abhängigkeitsgraph (Übersicht)

```
P0.1 ──┬─ P0.2
       ├─ P1.1 ─ P1.2 ─ P1.3 ─ P1.4 ─┬─ P1.5
       │                              ├─ P1.6 ─ P2.1 ─ P2.2
       │                              └─ P3.1 ─┬─ P3.2 ──────┐
       │                                       └─ P3.3 ──────┤
       │                                                     │
       └─ P4.1 ─ P4.2 ─ P4.3 ─ P4.4 ─ P4.5 ─ P4.6 ─ P4.7 ─ P4.8 ─┐
                                                                  │
                                                          P5.1 ─ P5.2 ─ P5.3
                                                                  │
                                                          P6.1 ─ P6.2 ─ P6.3 ─ P6.4 ─ P6.5
                                                                       │               │
                                                          P13.1 ───────┘    P7.1 ─ P7.2 ─ P7.3 ──┘
                                                          P13.2 ─ P13.3 ─ P13.4
                                                                  │
                                                          P8.1 ──┤
                                                          P8.2 ─ P8.3
                                                                  │
                                                          P9.1 ─ P9.2 ─ P9.3
                                                                  │
                                                          P10.1 ─ P10.2
                                                                  │
                                                          P11.1 ─ P11.2
```

(Vereinfacht – nicht alle indirekten Abhängigkeiten dargestellt.)

---

## Parallelisierungs-Hinweise

Pakete, die parallel an verschiedene Agenten gehen können:

**Gleichzeitig nach P1.4 möglich:**
- P1.5 (deferred-Caveat) und P1.6 (Activity-System) sind unabhängig.
- P3.1 (Worldgen-API) parallel zu Phase-2-Renderer.

**Gleichzeitig nach P4.8 möglich:**
- P5.1 (Save-Header) und P6.1 (Reaktions-Skelett) sind unabhängig.

**Gleichzeitig nach P6.3 möglich:**
- P13.1 (debug-ui Extraktion) ist unabhängig von P6.4+, kann jetzt gestartet werden.
- P13.2, P13.3, P13.4 sind nach P13.1 untereinander unabhängig (parallel durchführbar).

**Gleichzeitig nach P6.4 möglich:**
- P7.1 (Stains), P8.1 (Hydrologie), P9.1 (Sediment) sind unabhängig.

**Komplett unabhängig:**
- P11.1 und P11.2 können jederzeit nach Existenz der jeweiligen Registry
  laufen.

---

## Globale Disziplin-Regeln

Für jeden Agenten, jedes Paket gilt:

1. **Bibel ist Wahrheit.** Bei Widerspruch zwischen Plan und Bibel
   gewinnt die Bibel. Wenn Bibel unklar ist, Frage in Issue stellen
   statt selbst entscheiden.
2. **Tests gehören zum Paket.** Akzeptanzkriterien sind Pflicht, nicht
   Nice-to-have.
3. **Nicht über den Scope hinaus.** Lieber neues Paket vorschlagen als
   bestehendes aufblähen.
4. **Determinismus prüfen.** Jede Sim-relevante Funktion bekommt einen
   Determinismus-Test (gleicher Input → gleicher Output, auch parallel).
5. **Serde-Annotationen ab Tag 1.** Jedes neue Datum ist sofort persistent
   gemacht oder explizit als Derived markiert (`#[serde(skip)]`).
6. **Niemals `thread_rng()`** in Sim oder Worldgen.
7. **Keine Floats in Sim-State.** Bibel-Begründung in 11.5.
8. **Renderer-Code anfassen ist Renderer-Paket-Sache.** Sim-Pakete
   modifizieren nicht den Renderer und umgekehrt.

---

## Dokument-Pflege

Dieser Plan wird parallel zur Bibel weitergeführt. Wenn ein Paket abgeschlossen
ist, wird hier vermerkt (mit Datum, Commit-SHA, evtl. Abweichungen). Wenn
sich die Bibel ändert, werden betroffene Pakete aktualisiert.

**Status-Spalte hinzufügen sobald Implementierung beginnt** – zum Beispiel
als simple Tabelle:

| Paket | Status | Agent | PR |
|-------|--------|-------|-----|
| P0.1  | TODO   |       |     |
| P0.2  | TODO   |       |     |
| ...   |        |       |     |

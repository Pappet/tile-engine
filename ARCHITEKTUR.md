# Tile Engine – Architektur-Bibel

> Lebendiges Architektur-Dokument. Wird parallel zur Diskussion erweitert und
> dient als Referenz für die spätere Implementierung. Code-Skelette sind
> illustrativ, nicht copy-paste-fertig.

**Version:** v0.5 (Save/Load)
**Letzte Änderung:** Save/Load-Architektur als Sektion 11, Authoritative-vs-
Derived-Trennung, Verzeichnis-Layout, Versionierung, Determinismus nach Load

---

## 0. Oberster Leitsatz

> **Die API ist das Fundament. Jedes nachträglich gewünschte System muss sich
> einfügen lassen, ohne die Kern-API zu brechen. Erweiterungen kommen später,
> aber die API muss von Anfang an *erweiterbar* sein.**

Konkret heisst das:

- **Schicht-Trennung mit kleinen Verträgen.** Komponenten reden über minimale
  APIs. Was hinter dieser API passiert, ist austauschbar.
- **Plugin-fähig statt monolithisch.** Subsysteme werden als Bevy-Plugins
  gebaut, nicht als hardcoded Pipelines. Verschiedene Implementierungen
  desselben Konzepts sind separate Plugins.
- **Type-Map statt zentrale Mega-Structs.** Resources werden einzeln
  registriert (Bevy macht das geschenkt), nicht in einem grossen Struct
  zusammengefasst, das alle möglichen Felder anzeigen muss.
- **Sim weiss nichts über Worldgen, Worldgen nichts über Renderer.** Jede
  Schicht arbeitet auf der API der nächsten unten – nicht auf deren Internals.
- **Verhalten als Daten, nicht als Code.** Materialien, Liquids, Reaktionen
  sind Registry-Einträge. Neue Phänomene = neue Daten, keine Code-Änderung.

**Die Frage bei jeder API-Entscheidung lautet:** "Würde diese API es
verhindern, später X einzubauen?" Wenn ja, eine Schicht tiefer abstrahieren.

Diese Sektion steht bewusst an Position 0 – sie ist kein Detail, sondern der
Massstab, an dem alle anderen Entscheidungen gemessen werden.

---

## 1. Projektvision

Eine hochperformante 2.5D-Tile-Engine in Rust mit folgenden Säulen:

- **DF-Tiefe**: echte Voxel-Tiles in mehreren Z-Ebenen, Höhlen, mehrstöckige
  Strukturen, fliessende Materialschichten.
- **GregTech-Materiallogik**: Materialien sind komponierte Datensätze
  (Elemente, Eigenschaften), keine hardcoded Enums. Reaktionen, Schmelz-
  und Verarbeitungsprozesse fallen aus den Eigenschaften ab.
- **Mechanische Systeme**: Fluide, Druck, Temperatur, Pumpen, Räder.
- **Adaptive Geomorphologie**: Erosion, Sedimentation, Verwitterung – die Welt
  verändert sich messbar durch Strömung und Zeit.
- **Welt-Vielfalt**: das Engine-Datenmodell ist welt-typ-agnostisch. Standard-
  Erdwelt, schwebende Inseln (Avatar), Wüstenplaneten, Hohlwelten – alles
  durch austauschbare Worldgen-Plugins darstellbar.
- **Generische Fluide**: Wasser, Magma, Blut, Öl, Säure, beliebige zukünftige
  Liquids fallen aus einer Eigenschafts-Tabelle. Kein Spezialcode pro
  Flüssigkeitstyp.
- **Daten-getriebene Reaktionen**: Schmelzen, Gefrieren, Korrosion,
  Verbrennung, geologische Umwandlungen, magische Effekte – alle als
  Registry-Einträge. Eine einzige Resolver-Engine handhabt alles.

Bewusst **kein** Ziel: echte 3D-Voxel-Grafik. Wir bleiben bei DF-Stil 2.5D mit
Z-Ebenen, weil das Pathfinding, Rendering und Speicher entlastet, ohne den
mechanischen Tiefgang zu beschränken.

---

## 2. Tech-Stack-Entscheidung

| Komponente            | Wahl                          | Begründung |
|-----------------------|-------------------------------|------------|
| Sprache               | Rust                          | Performance, Determinismus, Speichersicherheit |
| ECS / Scheduler       | Bevy (`bevy_ecs` mind.)       | Paralleles System-Scheduling, Plugin-Architektur |
| Renderer              | Bevy bzw. wgpu/macroquad      | Entscheidung später, fürs Erste Bevy-Standard |
| Save-Format           | bincode + serde               | Kompakt, schnell, einfach. rkyv später falls nötig |
| Profiling             | puffin und/oder tracy         | Frühe Integration |
| Property-Tests        | proptest                      | v.a. für Koordinatenkonvertierung |
| Noise                 | `noise` oder `fastnoise-lite` | Worldgen, später entscheiden |
| Daten-Format          | RON oder TOML                 | Reaktions-/Material-Definitionen, Modding |

**Nicht** in Erwägung: `hecs`/`legion` standalone (Scheduling-Eigenbau),
ECS-Eigenbau (zu früh).

---

## 3. Schichten-Architektur

Die Engine besteht aus drei klar getrennten Schichten mit minimalen Verträgen:

```
┌──────────────────────────────────────────────────────┐
│  Renderer-Schicht                                     │
│  liest: ChunkData, MaterialRegistry, LiquidRegistry  │
│  → austauschbar (Bevy / wgpu / ASCII / ...)          │
├──────────────────────────────────────────────────────┤
│  Sim-Schicht                                          │
│  liest/schreibt: ChunkData                           │
│  Vertrag: Tiles haben Material, Liquid, Temperatur   │
│  → mehrere Sub-Plugins (Fluid, Reaktion, Erosion)    │
├──────────────────────────────────────────────────────┤
│  Worldgen-Schicht                                    │
│  schreibt: ChunkData beim Start (oder lazy)          │
│  → austauschbar als Plugin-Preset (Earthlike,        │
│     Avatar-Floating, Mars, Hollow-Earth, ...)        │
├──────────────────────────────────────────────────────┤
│  Core-Datenmodell                                    │
│  ChunkData, MaterialId, LiquidId, ReactionId,        │
│  Koordinaten – die EINZIGE Schicht, die alle         │
│  anderen kennen                                       │
└──────────────────────────────────────────────────────┘
```

**Sim sieht nicht, woher die Chunks kommen.** Renderer sieht nicht, was die
Sim als nächstes machen wird. Worldgen muss nichts über Sim wissen.

---

## 4. Kern-Datenmodell

### 4.1 Tiles, Entities und Chunks

**Drei klar getrennte Konzepte:**

1. **Tile** = Voxel an einer (x, y, z)-Position. Trägt Material, Liquid,
   Temperatur etc. **Kein** ECS-Entity. Lebt als Index in einem Chunk-Array.
2. **Chunk** = 32×32 Tiles einer einzelnen Z-Ebene. Genau **eine** ECS-Entity
   mit `ChunkData`-Komponente.
3. **Entity** = alles, was sich individuell verhält: Kreaturen, Items,
   Strukturen mit Eigenstate (Pumpen, Mechanismen), Projektile,
   Liquid-Quellen und -Senken.

**Begründung der Tile/Entity-Trennung:** Bei einer 256×256×100-Karte sind das
~6,5 Mio Tile-Positionen. Als ECS-Entities wäre der Archetype- und
Change-Detection-Overhead untragbar, und räumliche Lokalität ginge verloren.
Als dichte Arrays in Chunks bekommen wir Cache-Effizienz und
SIMD-Vektorisierung praktisch geschenkt.

### 4.2 Z-Ebenen-Strategie

**Chunks sind 2D pro Z-Ebene** (32×32×1), **nicht** 3D-Würfel à la Minecraft.

Begründung:

- 90 % aller Z-Ebenen einer typischen Karte sind massiver Stein – die wollen
  wir nicht allokieren müssen.
- Rendering einer Z-Ebene ist trivial.
- Vertikale Nachbarschaft wird wie horizontale behandelt: ein Cross-Chunk-Read
  über eine Z-Grenze ist mechanisch derselbe Vorgang wie über eine X-Grenze.
- Erlaubt später `Filler`-Optimierung (siehe 4.4).
- Welt-Typen wie schwebende Inseln, bei denen ganze Z-Ebenen praktisch leer
  sind (nur ein paar Insel-Cluster), profitieren massiv von dieser
  Granularität.

### 4.3 Storage-Layout (SoA)

Innerhalb eines Chunks: **Struct of Arrays** statt Array of Structs.

Vorteile:

- Cache-Lines bleiben mit relevanten Daten gefüllt – ein Fluid-Sweep liest
  nur `liquid_amount`, nicht Terrain und Temperatur mit.
- Compiler kann fixe Array-Grössen (1024) gut optimieren / vektorisieren.
- `Box<[T; N]>` statt `Vec<T>`: kein Kapazitäts-Overhead, keine
  Bounds-Check-Kosten bei kontrollierter Indexierung.

**Heterogenität innerhalb eines Chunks ist explizit gewünscht und einfach:**
jedes der 1024 Tiles hat ein unabhängiges `terrain[i]`. Granit, Sandstein,
Höhle (Air), Wasser-gefüllte Tile alles im selben Chunk – gar kein Problem.

Vollständige `ChunkData`-Struktur siehe Sektion 9.9.

### 4.4 Filler-Optimierung (Phase 2)

Z-Ebenen mit uniformem Stein müssen nicht alloziert werden:

```rust
enum ChunkSlot {
    Filler(MaterialId),    // ganzer Chunk uniform, keine Allokation
    Materialized(Entity),  // echte ChunkData als Component
}
```

Schreibzugriff auf einen Filler-Chunk **materialisiert** ihn. Lesezugriff
gibt das Filler-Material zurück, ohne zu allozieren.

Wichtig für alternative Welttypen: Avatar-Welt hat 99 % Filler(Air), nur
einzelne Insel-Cluster sind materialisiert. Hohlwelt hat zwei Filler-Schichten
(Stein aussen, Air innen) mit dünner Materialisierungs-Schicht dazwischen.

> Nicht für MVP. Aber das Datenmodell muss es zulassen – deshalb 2D-Chunks
> und nicht 3D-Würfel.

### 4.5 Materialien sind Daten, keine Enums

Zentrale Material-Registry, `MaterialId` ist nur ein Index:

```rust
struct Material {
    name: String,
    composition: SmallVec<[(ElementId, f32); 4]>, // z.B. Hämatit = Fe2O3
    density: f32,
    hardness: f32,
    melting_point: f32,
    erodibility: f32,
    flags: MaterialFlags,
    is_solid: bool,
    is_diggable: bool,
}
```

Reservierte IDs: `MAT_AIR = MaterialId(0)`.

Reaktionen werden als Recipes auf Komposition+Bedingungen formuliert (siehe
Sektion 10), nicht auf MaterialId-Tupel.

---

## 5. Koordinaten

```rust
pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;
const CHUNK_SHIFT: i32 = 5;
const CHUNK_MASK: i32 = 31;

pub struct WorldPos   { pub x: i32, pub y: i32, pub z: i32 }
pub struct ChunkCoord { pub cx: i32, pub cy: i32, pub cz: i32 }
pub struct LocalPos   { pub lx: u8, pub ly: u8 }
```

Konvertierung mit Bitshifts (negative Koordinaten!):

```rust
impl WorldPos {
    pub fn split(self) -> (ChunkCoord, LocalPos) {
        let cc = ChunkCoord {
            cx: self.x >> CHUNK_SHIFT,
            cy: self.y >> CHUNK_SHIFT,
            cz: self.z,
        };
        let lp = LocalPos {
            lx: (self.x & CHUNK_MASK) as u8,
            ly: (self.y & CHUNK_MASK) as u8,
        };
        (cc, lp)
    }
}
```

**Pflicht-Test (proptest):** Roundtrip über positive *und* negative Koordinaten.

**Konventionen:**

- Indexing: `ly * CHUNK_SIZE + lx` (row-major).
- `MaterialId`: `u16`. `LiquidId`: `u16`. `ReactionId`: `u32`.
- Liquid Amount: `u8` 0..7.

---

## 6. World-API

### 6.1 World als Resource

```rust
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct World {
    chunks: HashMap<ChunkCoord, Entity>,
    pub current_tick: u64,
}
```

### 6.2 SystemParam für ergonomischen Zugriff

```rust
#[derive(SystemParam)]
pub struct WorldAccess<'w, 's> {
    pub world: Res<'w, World>,
    pub chunks: Query<'w, 's, &'static ChunkData>,
}

#[derive(SystemParam)]
pub struct WorldAccessMut<'w, 's> {
    pub world: ResMut<'w, World>,
    pub chunks: Query<'w, 's, &'static mut ChunkData>,
    pub commands: Commands<'w, 's>,
}
```

Methoden: `get_tile`, `set_tile`, `get_chunk`, `ensure_chunk`.

### 6.3 Bulk-API für Hot Loops

Sim-Systeme greifen direkt auf die Komponente zu, nicht über `set_tile`.

### 6.4 Sim-zugewandte Service-APIs

```rust
pub trait SurfaceQuery: Resource {
    fn surface_z(&self, x: i32, y: i32) -> Option<i32>;
}
```

Earthlike: aus Heightmap. Avatar: aus Insel-Index. Sim ruft `query.surface_z`
ohne zu wissen, welches Plugin antwortet.

### 6.5 Was bewusst **nicht** in der Kern-API ist

`get_chunks_in_radius`, `flood_fill`, `raycast`, Streaming/Save-Logik. Kern-API
bleibt bei ~10 Methoden.

---

## 7. Simulations-Pipeline

### 7.1 System-Sets und Tick-Phasen

```rust
enum SimSet {
    PreTick,    // Snapshots, Buffer-Reset
    Simulate,   // Fluid, Temp, Reaktionen
    PostTick,   // Buffer-Swaps, Wake-Resolution, Activity-Decay
}
```

Sim läuft in `FixedUpdate`, **nicht** in `Update`.

### 7.2 Multi-Rate-Ticking

| System              | Frequenz          |
|---------------------|-------------------|
| Fluid               | jeden Tick        |
| Reaktionen          | individuell pro Reaktion |
| Wärmediffusion      | alle 4 Ticks      |
| Erosion / Sediment  | alle 100 Ticks    |
| Klima / Hydrologie  | spielminütlich    |

### 7.3 Activity-Tracking

```rust
bitflags! {
    pub struct SystemMask: u16 {
        const FLUID       = 1 << 0;
        const TEMPERATURE = 1 << 1;
        const EROSION     = 1 << 2;
        const REACTION    = 1 << 3;
        const GAS         = 1 << 4;
    }
}
```

---

## 8. Cross-Chunk-Problem

### 8.1 Double-Buffering

`liquid_amount_read` / `liquid_amount_write`. Pro Tick lesen aus A, schreiben
nach B, am Ende swap. Iterationsreihenfolge irrelevant → deterministisch.

### 8.2 Snapshot-Pattern

Vor dem Sim-Schritt einen Snapshot in eine Resource ziehen. Bevy lässt nicht
zu, dass ein System gleichzeitig mut iteriert und read-only auf dieselbe
Component-Art querit – der Snapshot löst das.

### 8.3 Symmetrische Bilanzgleichung

```rust
fn flow_between(here: u8, there: u8, viscosity: u8) -> i16 {
    if here <= there + 1 { return 0; }
    let raw = (here as i16 - there as i16) / 4;
    raw * (256 - viscosity as i16) / 256
}
```

Beide Seiten der Chunk-Grenze rufen sie mit denselben Argumenten auf,
modifizieren nur eigenen Write-Buffer. Massenerhaltung gilt automatisch.

### 8.4 Wake-up-Propagation

A sammelt Wake-Requests für inaktive Nachbarn. Im PostTick-Set verarbeitet.

### 8.5 Drei Prinzipien

1. Lesen und Schreiben physisch trennen.
2. CA-Regeln symmetrisch und lokal formulieren.
3. Cross-Chunk-Effekte als deferred messages.

---

## 9. Fluid-System

### 9.1 Designgrundsätze

1. **Liquids sind Daten** (LiquidRegistry).
2. **Ein Tile, ein Liquid** – Kollisionen via Reaktionen (10.4).
3. **Verhalten fällt aus Eigenschaften**.
4. **Quellen/Senken sind Entities**.
5. **Druckmodell Option 2**.

### 9.2 LiquidId und LiquidRegistry

```rust
pub struct LiquidId(pub u16);
pub const LIQ_NONE: LiquidId = LiquidId(0);

pub struct LiquidProperties {
    pub name: String,
    pub density: f32,
    pub viscosity: u8,
    pub freeze_point: i16,
    pub boil_point: i16,
    pub freezes_to: MaterialId,
    pub boils_to: GasId,           // Vorbehalt 9.10
    pub damages_living: u8,
    pub corrosion: u8,
    pub ignites_flammable: bool,
    pub color: [u8; 4],
    pub emits_light: u8,
    pub flags: LiquidFlags,
}

bitflags! {
    pub struct LiquidFlags: u32 {
        const POTABLE       = 1 << 0;
        const SACRED        = 1 << 1;
        const CONDUCTIVE    = 1 << 2;
        const MAGNETIC      = 1 << 3;
        const STAINS        = 1 << 4;
        const EVAPORATES    = 1 << 5;
    }
}
```

Beispiele: Water (1.0/10), Magma (2.5/200), Blood (1.06/80, SACRED+STAINS),
Oil (0.85/40, STAINS), Acid (1.20/15).

### 9.3 LiquidCell (konzeptioneller Tile-Eintrag)

```rust
pub struct LiquidCell {
    pub kind: LiquidId,
    pub amount: u8,
    pub temperature: i16,  // eigene Temp, getrennt vom Tile
}
```

In ChunkData als SoA aufgespalten (siehe 9.9).

### 9.4 Single-Liquid-Annahme: Kollisionen via Reaktions-System

Wenn neues Liquid in ein Tile fliesst, das bereits ein anderes enthält, ruft
das Fluid-System den Reaktions-Resolver mit Trigger `LiquidCollision` auf.
Das Fluid-System implementiert keine Spezial-Kollisions-Logik. Verhalten als
Daten, Mechanik zentralisiert.

### 9.5 Schichtung durch Dichte (vertikal)

Beim Z-Schritt wird Dichte verglichen. Schwerere Liquids sinken, leichtere
steigen. Symmetrische Bilanzgleichung mit Snapshot-Inputs.

### 9.6 Viskosität in flow_between

Skaliert die Flussrate, nicht die Mathematik. Magma kriecht (visc=200 →
22 % Rate), Wasser fliesst frei (visc=10 → 96 %).

### 9.7 Druckmodell (Option 2)

Pro Tile ein zusätzliches `pressure: u8`-Feld, double-buffered. Druck
propagiert pro Tick zwischen Nachbarn. Fluss berücksichtigt Höhen- *und*
Druckdifferenz. Ergebnis: U-Rohre, Pumpen, Wasserdruck funktionieren.

Begrenzung: Druck propagiert pro Tick einen Schritt – mehrere Ticks Latenz
durch lange Rohre. Akzeptabel.

Option 3 (BFS zur offenen Oberfläche, DF-Style) als spätere
Optimierungsschicht möglich, ohne Mechanik zu brechen.

### 9.8 Quellen und Senken als Entities

```rust
#[derive(Component, Serialize, Deserialize)]
pub struct LiquidSource {
    pub pos: WorldPos,
    pub kind: LiquidId,
    pub rate: u8,
    pub temperature: i16,
    pub max_pressure: u8,
}

#[derive(Component, Serialize, Deserialize)]
pub struct LiquidDrain {
    pub pos: WorldPos,
    pub rate: u8,
    pub accepts: LiquidFilter,
}
```

Vulkan, Bachquelle, Khorne-Altar, Brunnen, Abflussrost, Pflanzenwurzel –
alles dasselbe Pattern. Sim-Vertrag für Worldgen-Hydrologie: River-Graph
wird zu LiquidSource-Entities.

### 9.9 Erweiterte ChunkData (Vollständig)

```rust
#[derive(Component, Serialize, Deserialize)]
pub struct ChunkData {
    pub coord: ChunkCoord,
    
    // Tile-Material (authoritative)
    pub terrain: Box<[MaterialId; CHUNK_AREA]>,
    pub temp: Box<[i16; CHUNK_AREA]>,
    
    // Fluid-Felder (read = authoritative, write = derived – siehe Save 11.4)
    pub liquid_kind:           Box<[LiquidId; CHUNK_AREA]>,
    pub liquid_amount_read:    Box<[u8; CHUNK_AREA]>,
    #[serde(skip)]
    pub liquid_amount_write:   Box<[u8; CHUNK_AREA]>,
    pub liquid_temp_read:      Box<[i16; CHUNK_AREA]>,
    #[serde(skip)]
    pub liquid_temp_write:     Box<[i16; CHUNK_AREA]>,
    pub pressure_read:         Box<[u8; CHUNK_AREA]>,
    #[serde(skip)]
    pub pressure_write:        Box<[u8; CHUNK_AREA]>,
    
    // Stains (authoritative)
    pub stain_kind:   Box<[LiquidId; CHUNK_AREA]>,
    pub stain_amount: Box<[u8; CHUNK_AREA]>,
    
    // Reserviert für Gas (siehe 9.10)
    
    // Save-Tracking
    pub dirty: bool,  // wurde nach Worldgen modifiziert
    
    // Activity (derived – nach Load rekonstruiert)
    #[serde(skip)]
    pub active: SystemMask,
    #[serde(skip)]
    pub last_active_tick: u64,
}
```

Speicher pro Chunk: ~18 KB. Bei 10.000 aktiven Chunks ~180 MB. Gespeicherte
Felder pro Chunk (ohne Write-Buffer und Activity): ~12 KB.

### 9.10 Gas-Vorbehalt

Reserviert: `GasId`, `GasRegistry`, `GAS`-Bit, `EmitGas`-Effect,
`boils_to: GasId`. ChunkData hat keine Gas-Felder, werden bei Bedarf
nachgerüstet.

### 9.11 Stain-System

STAINS-Flag-Liquids hinterlassen beim Abfliessen Stains. Eigenes Sim-System,
selten (~100 Ticks). Stains können Reaktionen triggern (SACRED-Stain →
Dämonenspawn).

### 9.12 Fluid-Pipeline (Tick-Reihenfolge)

```
1. snapshot_liquid
2. liquid_source_system
3. liquid_collision           → Reaktions-System mit LiquidCollision-Trigger
4. liquid_horizontal_flow
5. liquid_vertical_flow
6. liquid_drain_system
7. liquid_evaporation
8. liquid_temp_diffusion
9. pressure_propagation
10. (PostTick) swap_buffers + process_wakes + stain_decay
```

### 9.13 Akzeptanz-Tests

1. Magma + Wasser → Verdampfung + Erstarrung.
2. Khorne-Altar → Blut fliesst zäh, Stains, U-Rohr-Verhalten.
3. Öl auf Wasser → stabile Schichtung.
4. U-Rohr → Pegelausgleich.
5. Pumpe → Wasser steigt ein Niveau, fliesst weiter.

---

## 10. Reaktions-System

### 10.1 Trennung: Tile-Reaktionen vs Verarbeitungs-Recipes

- **Tile-Reaktionen** (10): in-place auf Tiles, getrieben von Welt-Bedingungen.
- **Verarbeitungs-Recipes** (10.11): in Maschinen, vom Spieler ausgelöst.

### 10.2 ReactionId und Registry

```rust
pub struct ReactionId(pub u32);

#[derive(Resource)]
pub struct ReactionRegistry {
    reactions: Vec<Reaction>,
    by_trigger: HashMap<TriggerKind, Vec<ReactionId>>,
    by_material: HashMap<MaterialId, Vec<ReactionId>>,
    by_liquid: HashMap<LiquidId, Vec<ReactionId>>,
}
```

Indexe sind essentiell für Resolver-Performance.

### 10.3 Reaction-Struktur

```rust
pub struct Reaction {
    pub name: String,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,  // implizites UND
    pub effects: Vec<Effect>,
    
    pub primary_material: Option<MaterialId>,
    pub primary_liquid: Option<LiquidId>,
    pub min_temperature: Option<i16>,
    pub max_temperature: Option<i16>,
    
    pub probability: u16,
    pub cooldown_ticks: u16,
}
```

### 10.4 Trigger

```rust
pub enum Trigger {
    Periodic { every_ticks: u32 },
    LiquidCollision,
    LiquidTouchesMaterial,
    TemperatureCrossing { threshold: i16 },
    External(EventId),
}
```

### 10.5 Conditions

`conditions: Vec<Condition>` ist UND-verknüpft (Default). Boolean-Operatoren
für komplexere Logik:

```rust
pub enum Condition {
    // Atomar
    TileMaterialIs(MaterialId),
    TileLiquidIs(LiquidId),
    TileLiquidAmountAtLeast(u8),
    TempBetween(i16, i16),
    LiquidTempBetween(i16, i16),
    PressureAtLeast(u8),
    NeighborMaterialIs { dir: Direction, mat: MaterialId },
    NeighborLiquidIs { dir: Direction, liq: LiquidId },
    AnyNeighborIs(MaterialId),
    MaterialHasFlag(MaterialFlags),
    LiquidHasFlag(LiquidFlags),
    MaterialContainsElement { elem: ElementId, min_fraction: f32 },
    AgeAtLeast { ticks: u32 },
    IsTimeOfDay(TimeOfDay),
    DepthBelow(i32),
    HasTag(TagId),
    IncomingLiquidIs(LiquidId),  // nur bei LiquidCollision
    
    // Boolean
    Not(Box<Condition>),
    AnyOf(Vec<Condition>),
    AllOf(Vec<Condition>),
}
```

`MaterialContainsElement` ist der GregTech-Trick: eine Reaktion gilt
automatisch für alle Erze mit passender Komposition.

### 10.6 Effects

```rust
pub enum Effect {
    SetTileMaterial(MaterialId),
    SetLiquid { kind: LiquidId, amount: u8, temp: i16 },
    AddLiquidAmount(i16),
    AddTemperature(i16),
    SetTemperature(i16),
    SetStain { kind: LiquidId, amount: u8 },
    EmitGas { kind: GasId, amount: u8 },
    SpawnItem { item: ItemId, count: u8 },
    SpawnEntity { entity_type: EntityTypeId },
    EmitEvent(EventKind),
    PropagateToNeighbor { dir: Direction, effect: Box<Effect> },
    AreaEffect { radius: u8, effect: Box<Effect> },
}
```

### 10.7 Beispiel-Reaktionen

Iron-Schmelze (Composition-Filter), Wasser-Eis, Säure-Marmor (Propagation),
Khorne-Blut-Dämon (Stochastik), Sediment-Lithifizierung (geologische Zeit),
Magma-Wasser-Kollision. Siehe Vorgänger-Versionen für vollständige Beispiele.

### 10.8 Resolver mit deferred Effects

Phase 1 (parallel): Reaktionen finden, Effekte sammeln in `PendingEffects`.
Phase 2 (sequenziell): Effekte anwenden. Konflikt-Auflösung: deterministische
Reihenfolge nach ReactionId.

### 10.9 Determinismus

```rust
fn roll_probability(prob: u16, coord: ChunkCoord, idx: usize, tick: u64, rid: ReactionId) -> bool {
    let hash = mix_hash(coord, idx, tick, rid);
    (hash & 0xFFFF) < prob as u64
}
```

Reproduzierbar, parallel-sicher.

### 10.10 Performance-Strategien

Registry-Indexe, Min/Max-Temp-Filter, Periodic Phase-Offset, Activity-Bit,
Condition-Ordering (billig zuerst).

### 10.11 Verarbeitungs-Recipes (Vorbehalt)

Eigene Datenstruktur `ProcessingRecipe`, ähnliche Mechanik. Detaillierung
beim Maschinen-System.

### 10.12 Modding-Ausblick

Reaktionen, Materialien, Liquids in RON/TOML. Beim Start geladen, Registry
gefüllt. Modder kann Engine erweitern ohne Rust-Code.

### 10.13 Akzeptanz-Tests

1. Eisenerz schmelzen (auch automatisch für Magnetit, Pyrit).
2. Wasser-Eis-Zyklus.
3. Säure korrodiert spezifische Materialien.
4. Khorne-Altar spawnt Dämon.
5. Sediment wird über Zeit zu Sandstein.
6. Magma+Wasser=Dampf via LiquidCollision.
7. Komplexe Bedingung mit AnyOf/Not.

---

## 11. Save/Load-System

Save/Load betrifft alle bestehenden Sektionen. Wenn früh gut entworfen, bleibt
es ein lokales Subsystem; wenn falsch entworfen, verseucht es die ganze
Codebase.

### 11.1 Authoritative vs Derived – die zentrale Trennung

Vier Kategorien von Daten, die unterschiedlich behandelt werden:

| Kategorie | Beispiele | Save? |
|-----------|-----------|-------|
| **Authoritative State** | terrain, liquid_kind, liquid_amount_read, pressure_read, stains, current_tick, World.chunks, WorldStructures.caves, Entity-Components | **Ja** |
| **Derived State** | liquid_*_write, active-Bits, last_active_tick, LiquidSnapshot, PendingEffects | **Nein** (regeneriert) |
| **Static Data** | MaterialRegistry, LiquidRegistry, ReactionRegistry | **Nein** (aus Daten-Files) |
| **Runtime-Caches** | WorldStructures.spatial_index, Registry-Indexe | **Nein** (rebuild beim Load) |

Jedes neue Feld muss explizit einer Kategorie zugeordnet werden. Im Code via
`#[serde(skip)]` für Derived/Cache.

### 11.2 Worldgen-Maps: Strategie A (mitspeichern)

Die Worldgen-Maps (Heightmap, ClimateMap, RiverGraph, StratigraphyMap) werden
**mitgespeichert**. Begründung:

- Saves sind robust gegen Worldgen-Code-Änderungen.
- Load ist schnell und deterministisch unabhängig vom Worldgen-Plugin.
- Speicher-Mehrkosten überschaubar (ca. 30 MB unkomprimiert bei 1024×1024).

Verworfene Alternative B (nur Seed): bei jeder Worldgen-Bug-Fix wären alle
bestehenden Saves gebrochen – während der Entwicklung untragbar.

### 11.3 Tile-Modifikationen: Dirty-Chunks

Chunks haben ein `dirty: bool`-Flag. Gesetzt beim ersten `set_tile` nach
Worldgen. Save schreibt nur dirty Chunks vollständig; unmodifizierte werden
beim Load aus Worldgen regeneriert.

```rust
fn set_tile(&mut self, pos: WorldPos, tile: Tile) {
    let (cc, lp) = pos.split();
    let entity = self.ensure_chunk(cc);
    if let Ok(mut chunk) = self.chunks.get_mut(entity) {
        // ... mutation ...
        chunk.dirty = true;
    }
}
```

Konsequenz: Worldgen muss **deterministisch sein** für unmodifizierte Chunks
zu funktionieren. Das ist sowieso unsere Anforderung (Sektion 11.6).

Bei massiven Welten (Civilization-Phase) kann das viel sparen: 99 % der
Chunks sind unmodifiziert vom Worldgen.

### 11.4 Double-Buffer im Save

Nur **read-Buffer** wird gespeichert. Write-Buffer ist Derived, wird beim
nächsten Tick ohnehin neu geschrieben. Das halbiert den Save-Aufwand für
Fluid-Felder.

Folge: Save darf nur **zwischen** Sim-Ticks erfolgen, nicht in der Mitte.
Sonst sind read und write inkonsistent. Save-Trigger im PostTick oder vor
dem nächsten PreTick.

### 11.5 Format-Wahl: bincode + serde

bincode für MVP. Kompakt, schnell, einfach via serde-Annotationen. Wechsel
auf rkyv (zero-copy) möglich falls Lade-Performance zum Problem wird.

Floats sind okay in Worldgen-Maps (gespeichert, nicht regeneriert), aber
**nicht** in deterministischer Sim-State – dort `i32` / Fixed-Point.
Begründung: f32 ist plattformabhängig in den unteren Bits, bricht
Cross-Plattform-Saves.

### 11.6 Versionierung

Jedes Save hat einen Header:

```rust
const SAVE_MAGIC: [u8; 4] = *b"TILE";
const SAVE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct SaveHeader {
    magic: [u8; 4],
    version: u32,
    game_version: String,
    timestamp: u64,
    seed: u64,
    world_size: (i32, i32),
}
```

Beim Load: Magic prüfen, Version prüfen, ggf. Migration. Migrations als
Pipeline:

```rust
fn migrate_chunk_data(raw: &[u8], from: u32, to: u32) -> Result<ChunkData> {
    let mut current = decode_chunk_v(raw, from)?;
    for v in from..to {
        current = match v {
            1 => migrate_v1_to_v2(current),
            2 => migrate_v2_to_v3(current),
            _ => return Err(/* unsupported */),
        };
    }
    Ok(current)
}
```

Versionierung pro Datenstruktur, nicht nur global. Erlaubt feinkörnige
Migrations.

### 11.7 Save-Layout: Verzeichnis-Struktur

Ein Save ist ein **Verzeichnis** (optional als ZIP gepackt):

```
my_save/
├── header.bin          (Format-Version, Spielzeit, Seed, World-Size)
├── world.bin           (World-Resource: chunk-mapping, current_tick)
├── worldgen/
│   ├── heightmap.bin
│   ├── climate.bin
│   ├── rivers.bin
│   └── stratigraphy.bin
├── structures.bin      (caves, veins als deklarative Daten)
├── chunks/
│   ├── 0_0_0.bin       (jeder dirty Chunk)
│   ├── 0_0_1.bin
│   └── ...
├── entities.bin        (alle persistenten Bevy-Entities)
└── reactions/
    └── cooldowns.bin   (pro-Tile Reaktions-Cooldowns)
```

Vorteile:

- **Inkrementelles Speichern**: nur dirty Chunks neu schreiben.
- **Parallele Lade-Pfade**: Chunks parallel deserialisieren.
- **Debug-fähig**: einzelne Files inspizierbar.
- **Atomare Updates**: erst nach `tmp/`-Ordner schreiben, dann atomar
  umbenennen. Verhindert korrupte Saves bei Crash.

### 11.8 Entity-Serialisierung

Bevy-Entities sind Sammlungen von Components zur Laufzeit. serde weiss nicht,
welche Components ein Entity hat. Lösung: typsichere Component-Registry.

```rust
pub trait Persistent: Component + Serialize + DeserializeOwned {
    const TYPE_ID: u32;
}

impl Persistent for LiquidSource { const TYPE_ID: u32 = 1; }
impl Persistent for LiquidDrain  { const TYPE_ID: u32 = 2; }
impl Persistent for Creature     { const TYPE_ID: u32 = 3; }

#[derive(Serialize, Deserialize)]
struct SerializedEntity {
    components: Vec<(u32, Vec<u8>)>,  // (TYPE_ID, encoded bytes)
}
```

Beim Laden: iteriert Component-Liste, deserialisiert nach TYPE_ID.
Components ohne `Persistent`-Impl werden ignoriert (z.B. Renderer-Components
wie Sprite, Transform – werden nach dem Load wieder zugewiesen).

**Klare Trennung Sim-State / Render-State.** Renderer-Components sind nie
persistent.

### 11.9 Determinismus nach dem Laden

Nach Load muss die Sim *exakt* da weitermachen, wo sie aufgehört hat. Drei
oft übersehene Punkte:

**Wake-Requests**: Buffer muss am Save-Zeitpunkt leer sein. Lösung: Save nur
nach komplettem PostTick. Wake-Buffer wird dort verarbeitet, ist anschliessend
leer.

**Reaktions-Cooldowns**: separate Resource:

```rust
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct ReactionCooldowns {
    until_tick: HashMap<(WorldPos, ReactionId), u64>,
}
```

Pro Reaktion mit `cooldown_ticks > 0` wird hier eingetragen, wann der
Cooldown endet. Resolver prüft das vor jedem Trigger. Persistent.

**Stochastik**: Hash-basierte Würfel (10.9) brauchen kein State. Reproduzierbar
aus (Position, Tick, ReactionId). Geschenkt.

### 11.10 Save/Load als Bevy-Schedule

```rust
#[derive(Event)]
pub struct SaveRequest { pub slot: SaveSlot }

#[derive(Event)]
pub struct LoadRequest { pub slot: SaveSlot }

fn handle_save_requests(
    mut events: EventReader<SaveRequest>,
    world: &World,
) {
    for req in events.read() {
        do_save(world, req.slot);
    }
}
```

Save läuft **zwischen** Sim-Ticks (PostTick + nächster PreTick).
**Niemals** während `Simulate`-Phase – Buffer-Inkonsistenz garantiert.

### 11.11 Save-Slots und Auto-Save

Unverzichtbar:

- Mehrere Save-Slots (manuelle Slots 1, 2, 3 + Quicksave + Autosave).
- Auto-Save in Intervallen (z.B. alle 5 Spielminuten).
- Auto-Save mit **Rotation** (autosave-1, -2, -3 zyklisch). Verhindert, dass
  ein Bug-Crash kurz nach Auto-Save den letzten guten Stand überschreibt.
- Atomare Saves: erst `world.save.tmp/`, dann `mv` nach `world.save/`.

### 11.12 Async-Save (spätere Optimierung)

Für grosse Welten:

```rust
fn do_save_async(world: &World, task_pool: &AsyncComputeTaskPool) {
    let snapshot = SaveSnapshot::collect(world);  // synchron, kurz
    task_pool.spawn(async move {
        snapshot.write_to_disk("save_slot_1").await;
    });
}
```

Snapshot ist ein konsistenter Stand, der unabhängig von der laufenden Sim
geschrieben wird. Nicht für MVP, aber das Datenmodell muss es zulassen
(Snapshot-Struct kopierbar/sendbar).

### 11.13 Anti-Patterns

- **`Arc<T>`** in Save-Daten: doppelt serialisiert. Stattdessen IDs.
- **Closures**: nicht serde-fähig. Strukturen als deklarative Daten.
- **`Box<dyn Trait>`**: nicht serde-fähig out-of-the-box. Stattdessen Enum.
- **`f32` in Sim-State**: plattformabhängig. Stattdessen `i32`/Fixed-Point.

### 11.14 Save/Load in der Roadmap

Save/Load wird **nicht** in einem Schritt umgesetzt, sondern in zwei Phasen:

**Phase 1 – Vorbereitung** (parallel zu allen Roadmap-Schritten):
- Schritt 4 (ChunkData): `Serialize`/`Deserialize` ableiten,
  `#[serde(skip)]` für Derived-Felder.
- Schritt 5 (World): dito.
- Schritt 11 (LiquidRegistry): Versions-Konzept anlegen.
- Schritt 18 (ReactionRegistry): dito.

**Phase 2 – Implementierung**:
- Roadmap-Schritt 17.5: minimales Save/Load. Header + World + dirty Chunks +
  Entities. Synchron, ein Slot, kein Migration-Code.
- Nach Schritt 20 (Reaktionen): erweitert um ReactionCooldowns, Stains.
- Später: Auto-Save mit Rotation, Async, Migrations.

So vermeidet man, dass eine bestehende Sektion plötzlich umgebaut werden
muss, weil ein Datentyp nicht serialisierbar ist.

### 11.15 Akzeptanz-Tests

Save/Load ist "fertig", wenn:

1. **Roundtrip-Identität**: Save → Load → Save erzeugt **bitidentische**
   Files (nach Tick-Stand).
2. **Determinismus nach Load**: nach Load N Ticks weiterlaufen lassen,
   Resultat muss identisch sein wie ohne Save/Load.
3. **Cross-Version-Migration**: ein v1-Save lässt sich nach v2-Migration
   laden und weiterspielen.
4. **Atomarität**: Crash mitten im Save hinterlässt entweder kompletten
   alten oder kompletten neuen Save, nie korrupten Mischzustand.
5. **Inkrementelle Geschwindigkeit**: Save einer Welt mit nur 10 dirty
   Chunks dauert <100 ms (vs. Full-Save ~Sekunden).

---

## 12. Worldgen

### 12.1 Konzept: Pipeline aus Stages

Sequenzielle Pipeline. Jede Stage ist eine reine Funktion vom Seed plus
vorherigen Stages. Determinismus, Replayability.

**Worldgen ist ersetzbar.** Sim sieht nur fertige Chunks.

### 12.2 Skalen-Trennung

Welt-Skala (1 Wert pro Chunk-Region) / Region-Skala / Tile-Skala.

### 12.3 Standard-Earthlike-Pipeline

1. Tektonik · 2. Heightmap · 3. Hangneigung+Aspekt · 4. Klima-Basis ·
5. Sonneneinstrahlung · 6. Niederschlag · 7. Hydrologie · 8. Biome ·
9. Stratigraphie · 10. Bodentyp · 11. Höhlen · 12. Erzvorkommen ·
13. Vegetation · 14. Fauna · 15. Strukturen/Civilizations.

### 12.4 Stage-Resources

Jede Stage eigene Bevy-Resource. Stages, die nicht gebraucht werden, werden
nicht registriert. Bevy's Resource-System ist der Type-Map-Mechanismus.

### 12.5 Strukturen als globale Resource

Höhlen, Erzadern, Ruinen als deklarative Daten in Welt-Resource. Beim
Generieren: Spatial-Index sagt, welche Strukturen den Chunk schneiden, SDF
auswerten pro Tile. Keine Nähte über Chunk-Grenzen.

Strukturen sind serialisierbare Daten (Mittelpunkte, Radien, Pfadpunkte),
nicht Closures.

### 12.6 Determinismus

Master-Seed, pro Stage abgeleiteter Sub-Seed. Niemals `thread_rng()`.

### 12.7 ECS-Integration

```rust
enum WorldgenSet { Maps, Structures, Chunks }
```

Worldgen läuft in `Startup`, Sets sequentiell verkettet.

### 12.8 Streaming vs All-Upfront

MVP: endliche Welt, all-upfront. Später: lazy Chunk-Generation.

### 12.9 Naht-Lösungen

| Problem | Lösung |
|---------|--------|
| Höhle über mehrere Chunks | Strukturen als Welt-Resource |
| Heightmap-Übergänge | Domain-kontinuierliche Funktionen |
| Erzadern entlang Verwerfung | Verwerfung global, Adern als Resource |
| Stochastische Strukturen | Region-Pass mit Overlap-Rand |

### 12.10 Sim-Verträge an Worldgen

Worldgen muss liefern: gefüllte ChunkData, LiquidSource-Entities für
Flussquellen/Vulkane, optional SurfaceQuery-Implementierung, optional
Schwerkraft-Resource bei nicht-standardmässiger Welt.

---

## 13. Welttyp-Plugins

Verschiedene Welttypen sind separate Bevy-Plugins. Sim und Renderer laufen
unverändert darüber.

### 13.1 Earthlike (Standard)
### 13.2 Avatar-Floating-Islands
- Keine Heightmap, Inseln als 3D-Volumina.
- Wasser fällt von Inselrändern als Wasserfälle.
- Filler-Chunks essentiell.

### 13.3 Mars / Wüstenplanet
- Keine Hydrologie. Eisenoxid-Stratigraphie. Impact-Crater statt Höhlen.

### 13.4 Hollow-Earth / Dyson-Sphere
- Inverse Heightmap, zentrale Lichtquelle, radiale Schwerkraft.

### 13.5 Designvorgabe

Neuer Welttyp muss nur liefern: ChunkData, optional SurfaceQuery, optional
Schwerkraft-Resource. Sim läuft unverändert.

---

## 14. Worldgen-Erweiterungen (priorisiert)

| Stage | Konsumenten | Aufwand | Spielmechanik-Begründung |
|-------|-------------|---------|--------------------------|
| Hangneigung+Aspekt | Erosion, Vegetation, Pathfinding | sehr klein | Steile Hänge unbegehbar/erodieren stärker |
| Sonneneinstrahlung | Vegetation, Schnee | mittel | Bergpässe, Pflanzenwachstum |
| Bodentyp | Vegetation, Landwirtschaft | mittel | Erträge, realistische Pflanzen |
| Wind | Niederschlag, Brand | mittel | Regenschatten realistisch |
| Tektonik | Heightmap, Stratigraphie, Erze | hoch | Glaubwürdige Erzverteilung (GregTech!) |
| Jahreszeiten | Vegetation, Sim | klein | Frost, Trockenzeit |
| Magnetfeld | spezielle Mechanik | klein | Kompass-Anomalien, Spezial-Erze |
| Civilizations | Strukturen, Lore | sehr hoch | DF-Welt-History, optional |

Jede neue Stage braucht konkrete Spielmechanik-Begründung.

---

## 15. MVP-Roadmap

1. Crate-Setup, Bevy, Window.
2. Koordinaten-Module + proptest.
3. Material-Registry (Granit, Sandstein, Sand, Air).
4. ChunkData-Component mit Terrain. **serde-Ableitung von Anfang an.**
5. World-Resource + WorldAccess SystemParams.
6. Renderer für eine Z-Ebene, statisch.
7. Mehrere Chunks, lazy Spawn.
8. Activity-System mit Decay.
9. Z-Ebenen.
10. Mini-Worldgen-Plugin (Heightmap+Stratigraphie).
11. LiquidRegistry + Liquid-Felder, einfacher CA.
12. Snapshot-Resource + Cross-Chunk-Fluid.
13. Wake-Propagation.
14. Z-Fall + vertikale Schichtung.
15. Druckmodell (Option 2).
16. LiquidSource/LiquidDrain als Entities.
17. Mehrere Liquid-Typen.
**17.5. Save/Load (minimal)**: Header + World + dirty Chunks + Entities.
18. ReactionRegistry + Resolver-Skeleton.
19. Erste Reaktionen (Wasser-Eis, Eisenerz-Schmelze).
20. LiquidCollision-Trigger (Magma+Wasser). **Save erweitern um Cooldowns.**
21. Stains.
22. Worldgen-Hydrologie (River-Graph → LiquidSource).
23. Sediment-Tracking.
24. Erosion (100-Tick-System).
25. Höhlen-Worldgen (CA + Worm).
26. Komplexe Reaktionen (Säure, Lithifizierung, SACRED).
27. Mechanik (Pumpen, Räder).

Schritt 11: erstes Spielzeug. Schritt 17.5: Save funktioniert. Schritt 20:
Fluid+Reaktion-Kern reif. Schritt 26: GregTech-Tiefe.

---

## 16. Designprinzipien

- Erweiterbarkeit vor Bequemlichkeit (Sektion 0).
- Verhalten als Daten, nicht als Code.
- Determinismus von Anfang an. Niemals `thread_rng()`. Fixed-Point in
  Sim-State.
- Save/Load von Anfang an mitdenken: serde-Annotationen, klare
  Authoritative-vs-Derived-Trennung.
- Strukturen als Daten, keine Closures.
- Profiling früh (Schritt 5).
- Tiles modifizieren nur sich selbst.
- Kleine Kern-API, Algorithmen in eigenen Modulen.
- Sim/Worldgen/Renderer kommunizieren über minimale Verträge.
- Atomare Conditions/Effects, lieber kombinierbar als monolithisch.
- 2.5D, nicht 3D.
- Chunks bei 32×32.
- Ein Tile, ein Liquid.

---

## 17. Offene Fragen / TODOs

- [ ] DF-Style Druck (Option 3) als spätere Optimierungsschicht.
- [ ] Renderer-Wahl: Bevy-Standard für MVP, später ggf. eigener wgpu-Renderer.
- [ ] Pathfinding: A* mit Cross-Z, HPA* für grosse Karten.
- [ ] Wärmediffusion: voraussichtlich Bilanz-Trick reicht.
- [ ] Element-Tabelle: Granularität festlegen.
- [ ] Time scale: 1 Tick = ? Sekunden.
- [ ] Schwerkraft als Resource (Hollow-Earth/Avatar).
- [ ] Service-Traits: weitere Queries definieren wenn beim Bauen Bedarf
      entsteht.
- [ ] Worldgen-Caching: Resume bei langen Worldgen-Läufen.
- [ ] Gas-System: vollständige Architektur offen.
- [ ] Verarbeitungs-Recipes: Maschinen-System inkl. Energie-/Item-Logistik.
- [ ] Modding-API: RON/TOML-Loader für Registries.
- [ ] Save-Komprimierung: zstd-Layer falls 30 MB pro Save zum Problem werden.
- [ ] Cross-Plattform-Save-Tests: Save auf x86, Load auf ARM (verifiziert
      Fixed-Point-Determinismus).

---

## Changelog

- **v0.5** – Save/Load-Sektion (11): Authoritative-vs-Derived-Trennung,
  Strategie A für Worldgen-Maps, Dirty-Chunks für inkrementelles Speichern,
  bincode + serde, Versionierung mit Migration-Pipeline, Verzeichnis-Layout,
  typsicheres Persistent-Trait für Entities, ReactionCooldowns als
  persistente Resource, Bevy-Event-getriebenes Save/Load, Save-Slot-Rotation,
  Async-Save als spätere Optimierung, fünf Akzeptanz-Tests. ChunkData (9.9)
  um `dirty: bool` und `#[serde(skip)]`-Annotationen erweitert. Roadmap um
  Schritt 17.5 (minimal Save) und Save-Erweiterung in Schritt 20 ergänzt.
  Designprinzip Save/Load-Vorbereitung. TODOs um Save-Komprimierung und
  Cross-Plattform-Tests erweitert.
- **v0.4** – Reaktions-Sektion: ReactionRegistry, Trigger/Condition/Effect-
  Enums, Composition-Filter, Boolean-Operatoren, Resolver mit deferred
  Effects, Determinismus-Hash, Performance-Strategien.
- **v0.3** – Fluid-Sektion: LiquidRegistry, Single-Liquid-pro-Tile,
  Dichte-Schichtung, Viskosität, Druckmodell Option 2, Quellen/Senken als
  Entities, Stains, Gas-Vorbehalt.
- **v0.2** – Erweiterbarkeits-Leitsatz, Schichten-Architektur, Service-API,
  Worldgen-Pipeline, Welttyp-Plugins.
- **v0.1** – Erstfassung. Tech-Stack, Kern-Architektur, World-API,
  Cross-Chunk-Problem, MVP-Roadmap.

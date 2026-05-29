# Code-Audit: tile-engine

> Ganzheitliches Architektur- und Logik-Audit. Fokus auf Makroebene, strukturelle
> Integrität und modulübergreifende Logikrisse — nicht auf Linter-/Formatierungsfragen.
>
> **Stand:** Phase 14 (~6.800 Zeilen Rust). Hinweis: `CLAUDE.md` führt das Projekt
> fälschlich als „pre-implementation"; tatsächlich ist es weit fortgeschritten.

## Methodische Einordnung

Die einzelnen Module sind isoliert betrachtet sauber und gut getestet (Massenerhaltung,
Determinismus, U-Pipe etc. werden pro Crate verifiziert). Die Risse liegen **exakt an den
Nahtstellen zwischen den Modulen** — dort, wo kein einzelner Unit-Test hinreicht und wo
über die Iterationen hinweg der globale Kontext verloren ging.

Jeder Befund ist mit einem GitHub-Issue verknüpft.

---

## Kritische Bedrohungen

| ID | Titel | Issue |
|----|-------|-------|
| B1 | Sim-Orchestrierung fehlt: kein `SimSet`, kein `FixedUpdate` | [#58](https://github.com/Pappet/tile-engine/issues/58) |
| B2 | Inkompatible Buffering-Konventionen (Reaktion ↔ Fluid) ohne Ordering | [#59](https://github.com/Pappet/tile-engine/issues/59) |
| B3 | Massenerhaltung bricht bei Horizontal- + Vertikalfluss (Duplikation) | [#60](https://github.com/Pappet/tile-engine/issues/60) |
| B4 | Memory Leak: `WakeRequests.pending` wird nie geleert | [#61](https://github.com/Pappet/tile-engine/issues/61) |
| B5 | Save-Format nicht ladefest: Seed hartkodiert (0) und nie verifiziert | [#62](https://github.com/Pappet/tile-engine/issues/62) |

### B1 — Das architektonische Rückgrat fehlt

`ARCHITEKTUR.md`/`CLAUDE.md` schreiben zwingend vor: *„SimSet ordering: PreTick → Simulate
→ PostTick. Runs in Bevy `FixedUpdate`."* **Existiert nicht.** Kein `SimSet`, kein
`configure_sets`, kein `FixedUpdate`. Alle Sim-Systeme hängen in `Update` und ordnen sich
nur **innerhalb** ihres Plugins per `.chain()`. Zwischen den Plugins ist die Reihenfolge
undefiniert (Wurzel von B2/B3/S3/S8). Zudem ist die Simulationsgeschwindigkeit an die
Framerate gekoppelt → Replay-/Netzwerk-Determinismus prinzipiell unmöglich.

Belege: `sim-fluid/src/lib.rs:26`, `sim-reaction/src/resolver.rs:313`,
`core/src/activity.rs:51`, `app/src/main.rs:42-49`.

### B2 — Zwei inkompatible Buffering-Konventionen kollidieren

Fluid double-buffert (`_read`/`_write` + `swap_buffers`). Reaktionen schreiben **direkt in
die Read-Buffer** (`resolver.rs:116-119`). Beide teilen `Query<&mut ChunkData>`, Bevy
serialisiert sie — aber **ohne festgelegte Reihenfolge**. Je nach Topologie geht ein
`SetLiquid`/`AddLiquidAmount` beim Swap verloren oder nicht → dasselbe Programm liefert je
nach Build unterschiedliche Ergebnisse.

### B3 — Massenerhaltung bricht bei kombiniertem Fluss

`fluid_step_local` und `liquid_vertical_flow` lesen beide aus demselben Snapshot und addieren
ihre Deltas unabhängig auf `liquid_amount_write` (`fluid_ca.rs:314`, `vertical_flow.rs:117`).
Eine Quell-Kachel kann im selben Tick voll horizontal verteilen **und** voll nach unten
fallen; beide Senken werden gutgeschrieben, die Quelle nur per `clamp` auf 0 gedrückt →
**Masse wird erzeugt.** Kein Test kombiniert beide Flows.

### B4 — Unbegrenztes Wachstum von `WakeRequests.pending`

`wake.pending.push(...)` in `fluid_ca.rs:471` und `vertical_flow.rs:73,82`. **Kein System
leert den Vec** (nur `debug-ui` liest `.len()`). In einer laufenden Welt wächst er pro Tick
unbegrenzt.

### B5 — Save-Seed hartkodiert

`save.rs:147` schreibt `SaveHeader::new(0,(0,0))` (Kommentar: *„P5.3 will wire
WorldgenConfig"*). Der Seed wird beim Laden nie verifiziert. Da Clean-Chunks deterministisch
aus dem Seed regeneriert werden, korrumpiert ein Seed-Mismatch lautlos die Welt.

---

## Detaillierte Schwachstellenanalyse

| ID | Befund | Schwere | Issue |
|----|--------|---------|-------|
| S1 | `liquid_temp` ist totes Double-Buffer-Paar → Temperatur flackert | hoch | [#63](https://github.com/Pappet/tile-engine/issues/63) |
| S2 | Vertikalfluss: `=` statt `+=` clobbert Mehrschicht-Stapel | mittel | [#64](https://github.com/Pappet/tile-engine/issues/64) |
| S3 | `current_tick` ungeordnet inkrementiert → Tick-Inkonsistenz | hoch | [#65](https://github.com/Pappet/tile-engine/issues/65) |
| S4 | `LiquidFilter::ByFlags` ist ein No-Op | mittel | [#66](https://github.com/Pappet/tile-engine/issues/66) |
| S5 | Verwaistes `SystemMask`-Aktivitätssystem (toter Parallelpfad) | mittel | [#67](https://github.com/Pappet/tile-engine/issues/67) |
| S8 | Save läuft mid-Tick und ist nicht atomar (Invariante #6) | mittel | [#68](https://github.com/Pappet/tile-engine/issues/68) |
| S6/S7/S9/S10 | Aufräumen: verwaiste Crate, Namen, stille Fehler, toter Code | niedrig | [#69](https://github.com/Pappet/tile-engine/issues/69) |

### Logik & Datenfluss

- **S1 — `liquid_temp` totes Double-Buffer-Paar.** `chunk.rs:180` swappt `liquid_temp_read ↔
  liquid_temp_write` jeden Tick, aber `init_fluid_write_buffers` (`fluid_ca.rs:553`) kopiert
  `temp` nicht, und **niemand beschreibt `liquid_temp_write`**. Folge: `liquid_temp_read`
  oszilliert zwischen Anfangswert und 0. `Condition::LiquidTempBetween` (`resolver.rs:58`)
  sieht jeden zweiten Tick 0°C → Temperaturlogik der Flüssigkeiten faktisch funktionslos.
- **S2 — Vertikalfluss Clobber.** `vertical_flow.rs:80,104` nutzen `=` statt `+=`; eine
  Kachel durchläuft „unten"- und „oben"-Block, der zweite überschreibt das erste Delta. Bricht
  bei dichten 3-lagigen Stapeln.
- **S3 — Tick-Inkonsistenz.** `tick_counter_system` (`activity.rs:26`) ist in `Update`
  ungeordnet; tick-abhängige Systeme (`mix_hash`, `every_ticks`) sehen je nach Position N oder
  N+1.

### Architektur & Struktur

- **S5 — Toter `SystemMask`-Pfad.** Einziger Produktionsschreiber `activity_decay_system`
  (`activity.rs:38`) **leert** nur; nichts setzt je aktiv. Die echte Skip-Optimierung läuft
  über `snapshot.chunk_has_liquid()`. Zwei Aktivitätsmechanismen, einer tot, einer leckend
  (B4).
- **S6 — Verwaiste Crate.** `app/main.rs:40` nutzt `DemoWorldgenPlugin`; `worldgen-earthlike`
  ist als Dependency referenziert, dessen Plugin wird aber nie hinzugefügt.
- **S7 — Namens-Inkonsistenz.** `liquid_kind` trägt kein `_read`, anders als
  `liquid_amount_read`/`liquid_temp_read`/`pressure_read` (`chunk.rs`).
- **S10 — Toter Code.** `flow_between` (`fluid_ca.rs:14`) durch `flow_with_pressure` ersetzt.

### Fehlerbehandlung & Robustheit

- **S4 — `ByFlags`-Filter No-Op.** `source_drain.rs:93-96` gibt `=> true` zurück; ein
  flag-gefilterter Drain saugt jede Flüssigkeit ab.
- **S8 — Save mid-Tick & nicht atomar.** `save.rs:133` in `Update` ungeordnet (Verletzung
  Invariante #6); Entity-Save (`main.rs:120`) und World-Save sind getrennte, nicht atomare
  Pfade.
- **S9 — Stille Schreibfehler.** `world.rs:90` (`set_tile`) und `source_drain.rs:63` schlucken
  „Chunk nicht queryable" per `if let Ok(...)` ohne Log.

---

## Refactoring-Roadmap (priorisiert)

### Stufe 0 — Determinismus-Fundament (blockiert alles andere)
1. **`SimSet` einführen** (`PreTick → Simulate → PostTick`) in **`FixedUpdate`** (B1, #58).
   `tick_counter` zuerst in `PreTick` (S3, #65).
2. **Buffering-Konvention vereinheitlichen** (B2, #59): Reaktionen entweder auf
   Write-Buffer+Swap umstellen oder strikt in `PostTick` nach `swap_buffers` terminieren.

### Stufe 1 — Datenverlust & Lecks stopfen
3. **B3 (#60)** gemeinsames Outflow-Budget über Horizontal+Vertikal; kombinierter Test.
4. **B4 (#61)** `wake.pending` am Tick-Ende leeren oder Mechanik entfernen.
5. **S1 (#63)** `liquid_temp` korrekt mitführen oder Double-Buffer auflösen.

### Stufe 2 — Persistenz härten
6. **B5 (#62)** Seed in `SaveHeader` schreiben und beim Laden verifizieren.
7. **S8 (#68)** Save ans `PostTick`-Ende binden; Entity- + World-Save atomar zusammenführen.

### Stufe 3 — Aufräumen (Kontextverlust-Artefakte)
8. **S5 (#67)** toten `SystemMask`-Pfad reaktivieren oder löschen.
9. **S4 (#66)** `ByFlags`-Filter implementieren oder als unimplementiert kennzeichnen.
10. **S2 (#64)** Vertikal-Deltas konsistent akkumulieren.
11. **S6/S7/S9/S10 (#69)** verwaiste Dependency, Namensgebung, stille Fehler, toter Code.

---

## Gesamtbefund

Die größte Gefahr ist nicht ein einzelner Bug, sondern die **fehlende Sim-Orchestrierung
(B1)**, aus der B2/B3/S3/S8 als Folgeschäden erwachsen. Solange die Systeme in `Update` ohne
`SimSet` schweben, hält die Korrektheit der Einzelmodule nur, weil Bevy zufällig eine
günstige Reihenfolge wählt. Empfehlung: **Stufe 0 + 1 vor jeder weiteren Feature-Phase**
abarbeiten.

# Plano de Evolução — Ruana Studio

> Documento vivo. Cada parte entregue é marcada aqui **no mesmo commit** que
> implementa a mudança. Se o código mudar de rumo, este arquivo muda junto.

**Última atualização:** 2026-11-17

---

## Como este plano é executado

1. Uma **parte** por vez (item de checklist), nunca várias de uma vez.
2. Cada parte = **1 commit** com prefixo convencional + este `.md` atualizado
   (`[ ]` → `[x]`, e entrada em "Registro de progresso").
3. O app precisa **compilar e abrir** ao fim de cada parte
   (`cargo build -p ruana-studio` + execução de 15 s sem panic).
4. Se uma parte não couber em um commit coerente, ela é dividida aqui antes de
   começar.

Legenda: `[ ]` pendente · `[~]` em andamento · `[x]` feito · `[!]` bloqueado

---

## Diagnóstico de partida (2026-11-17)

| Área | Estado | Evidência |
|---|---|---|
| Timeline wgpu + waveform GPU | ✅ real | `timeline-render/`, `app/src/waveform_gpu/` |
| Engine de áudio (cpal, RT, stretch) | ✅ real | `crates/daw-audio/engine.rs` (94 KB) |
| Modelo de documento + undo/redo | ✅ real | `crates/daw-core/project.rs` (68 KB) |
| Beat sequencer / Sampler / Explorer | ✅ UI real | `ui/beat/`, `ui/sampler/`, `ui/explorer/` |
| Mixer | ⚠️ UI pronta, áudio não | modelo só tem `Volume/Pan/Muted/Soloed/Armed`; inserts não processam |
| Piano roll | ⚠️ WIP não compilado | `ui/piano_roll/*` + `app/src/piano_roll.rs` sem `mod piano_roll;` |
| Salvar/abrir projeto | ❌ ausente | `crates/daw-project/src/lib.rs` = 84 bytes |
| Plugins VST3/CLAP | ❌ ausente | `crates/daw-plugins/src/lib.rs` = 71 bytes |
| Export / bounce WAV | ❌ ausente | sem mixdown offline |
| Sends / buses / pan law | ❌ ausente | `daw-audio/src/mixer.rs` = soma estéreo flat + solo |

**Risco nº 1:** todo o trabalho do usuário se perde ao fechar o app.

---

## Fase 0 — Fundação (persistência)

- [x] **0.1** Formato de projeto JSON versionado em `crates/daw-project`
      (DTOs + `save`/`load` + testes de round-trip) — `FORMAT_VERSION = 1`,
      extensão `.ruana`, 5 testes verdes (`cargo test -p daw-project`)
- [x] **0.2a** `Salvar / Salvar como` (`Ctrl+S` / `Ctrl+Shift+S`) + `dirty flag`
      + título da janela com `*` — `app/src/project_io.rs`
- [x] **0.2b** `Abrir` (`Ctrl+O`) com resync do documento: transporte, modelos
      de tracks/cores/clips (mutados com `set_vec`, sem trocar os `Rc`),
      inserts, mixer e barras da timeline. A engine wgpu se re-espelha no
      tick seguinte via `clips_flag` — `app/src/project_open.rs`
- [ ] **0.2c** Confirmação ao fechar com alterações pendentes
- [ ] **0.2d** Resync pós-abrir do beat sequencer / sampler / explorer
      (hoje os padrões vêm no arquivo, mas a UI de beat mantém o estado anterior)
- [ ] **0.3** Autosave periódico + recuperação
- [ ] **0.4** Revisão do undo/redo (hoje grava `DocumentEvent` parcial)

**Aceite:** fechar e reabrir restaura tracks, clips, mixer, tempo map e playhead.

---

## Fase 1 — Mixer v2

### 1A · Layout e UX (`ui/mixer/*`)

- [x] **1A.1** Strip com seções colapsáveis — `ui/mixer/section_header.slint`
      (triângulo + rótulo) e toggles **globais** `show-inserts` / `show-pan`
      no `Mixer`, para abrir/fechar em todas as faixas de uma vez
- [x] **1A.2** Master fixo à direita: já estava fora do `Flickable`; agora com
      separador de 2px e recebendo `available-height` (antes herdava o default
      de 200px e desafinava o layout)
- [x] **1A.3** Scroll horizontal já vinha do `Flickable`; agora com **largura de
      canal ajustável** (72–148px, passo 14) pelos botões `−`/`+` do
      `MixerHeader`. O `content-width` acompanha e o scroll se recalcula
- [x] **1A.4** Reordenar inserts por drag — arrastar pelo número/nome do slot;
      o gesto vira `move_mixer_insert` no core (uma operação de undo) e a
      cadeia é re-projetada em todas as janelas do mixer
- [ ] **1A.5** Rename inline (double-click) em canal e inserts
- [ ] **1A.6** Cor do canal sincronizada com a track (`track-colors`)
- [ ] **1A.7** Meters: peak/RMS, escala dB, clip-hold, reset por clique
- [ ] **1A.8** Estado do mixer persistido (altura, seções, ordem)

**Aceite:** 24 canais usáveis em 1080p sem cortar conteúdo.

### 1B · Funcionalidade real (`daw-core/mixer.rs` + `daw-audio/mixer.rs`)

- [ ] **1B.1** `input_gain` + `output_bus` no `MixerChannel`
- [ ] **1B.2** `sends: Vec<Send{pre/post, level, target_bus}>` + `MixerParam::Send`
- [ ] **1B.3** Buses de grupo (`TrackKind::Bus`) somando no master, solo-safe
- [ ] **1B.4** Pan law −3 dB no centro
- [ ] **1B.5** Null/reset de canal + copy/paste de settings

**Aceite:** fader, pan e send mudam o som de fato.

---

## Fase 2 — DSP interno nos inserts

- [ ] **2.1** Trait `Processor { prepare, process, set_param }` em `daw-audio`
- [ ] **2.2** DSPs: Gain, EQ 3-band (biquad), Compressor, Limiter, Filter
- [ ] **2.3** Cadeia de inserts no caminho RT (ordem, bypass sem clique)
- [ ] **2.4** UI de parâmetros do insert (painel flutuante pequeno)
- [ ] **2.5** Metring de redução de ganho (GR)

**Aceite:** inserir EQ e ouvir a mudança; bypass A/B sem estalo.

---

## Fase 3 — Piano roll + MIDI

- [ ] **3.1** Ligar o WIP: `mod piano_roll;` + wiring
- [ ] **3.2** Decidir renderer (Slint vs wgpu) e remover a duplicação
- [ ] **3.3** Edição de notas: add/move/resize/velocity/quantize
- [ ] **3.4** Seleção múltipla, copy/paste, delete, legato
- [ ] **3.5** Scale assistant ligado ao snapping
- [ ] **3.6** Gravação MIDI (arm + input físico)

---

## Fase 4 — Produção

- [ ] **4.1** Export mixdown WAV offline
- [ ] **4.2** Export de stems por track/bus
- [ ] **4.3** Freeze/unfreeze de track
- [ ] **4.4** Loop region + punch in/out + count-in

---

## Fase 5 — Plugins (projeto separado)

- [ ] **5.1** Decisão de escopo (VST3 / CLAP)
- [ ] **5.2** Scanner + sandbox + cache de plugins
- [ ] **5.3** Ponte de áudio (block-based) reutilizando a trait `Processor`

---

## Sprint 1 (ordem de execução acordada)

1. **Fase 0.1 + 0.2** — formato de projeto + Salvar/Abrir
2. **1A.1 + 1A.2** — strip colapsável + master pinado
3. **1A.3** — scroll horizontal e largura de canal
4. **1A.4** — reordenar inserts
5. **1B.1 + 1B.4** — input gain, output bus, pan law
6. **1B.2** — sends
7. **3.1** — ligar o piano roll WIP

---

## Riscos permanentes

| Risco | Mitigação |
|---|---|
| Manutenção dual do renderer (`timeline-render` standalone × headless) | extrair módulo comum antes de mexer na timeline |
| `.slint` com chaves desbalanceadas reporta linha errada | compilar a UI após cada edição |
| Áudio granulado (buffer cpal) sem teste de escuta | validar antes de empilhar DSP |
| Piano roll WIP possivelmente incompatível com Slint 1.18 | ligar e compilar cedo |
| Auto-colapso de seção por altura em `.slint` gera binding loop | o `if` altera a altura que alimenta a condição; calcular no Rust e passar como flag |

---

## Registro de progresso

| Data | Commit | Entrega |
|---|---|---|
| 2026-11-17 | `b4df499` | fix(gpu): `COPY_DST` nos buffers da timeline (crash na abertura) |
| 2026-11-17 | _este commit_ | docs: criação deste plano |
| 2026-11-17 | _este commit_ | feat(project): formato `.ruana` versionado (Fase 0.1) |
| 2026-11-17 | _este commit_ | feat(app): `Ctrl+S` / `Ctrl+Shift+S` salva `.ruana` + dirty flag (Fase 0.2a) |
| 2026-11-17 | _este commit_ | feat(app): `Ctrl+O` abre `.ruana` com rebind do documento (Fase 0.2b) |
| 2026-11-17 | _este commit_ | feat(mixer): seções colapsáveis + master pinado com separador (Fase 1A.1/1A.2) |
| 2026-11-17 | _este commit_ | feat(mixer): largura de faixa ajustável no header (Fase 1A.3) |
| 2026-11-17 | _este commit_ | feat(mixer): reordenar inserts por drag (Fase 1A.4) |

# Plano: Meters não exibem sinal visual (verde) quando áudio toca

## Contexto

O pipeline dos meters é:

```
Engine (callback RT, engine.rs:1382)
  → MeterBank::publish() [valores atômicos por canal]
  → AudioBridge::meters() [retorna &MeterBank]
  → MeterPump::tick() [timer 33Hz, playback.rs:130-142]
      → bank.read(channel) → ChannelMeter::advance()
      → MeterModels::publish() [VecModel::set_row_data ×6 por canal]
  → Slint UI [channel-meters <=> root.channel-meters → meter-at(i,slot)
    → Meter → meter_reuse.slint desenha barras]
```

`set_channel_meters` é chamado **uma única vez** no startup (`main.rs:391`). Depois, `MeterPump::tick()` atualiza o modelo via `set_row_data` a cada ~33ms.

## Hipóteses de falha (ordenadas por probabilidade)

### H1: `VecModel::set_row_data` não notifica o Slint (mais provável)

`MeterModels::publish()` chama `self.values.set_row_data(base + FIELD_LEFT, ...)` seis vezes por canal, a cada tick. No Slint 1.18, `set_row_data` modifica os dados in-place **sem emitir `model_changed()`**. Como consequência, as bindings do Slint (`meter-at(i, slot)` que lê `root.meters[idx]`) nunca são reavaliadas e os metros ficam nos valores iniciais (zero).

Evidência contra: o comentário em `mixer.rs:88` diz que `set_row_data` propaga para todas as janelas. Isso pode ser verdade para modelos que usam `set_vec` para reset inicial + `set_row_data` para updates, mas só funciona se o Slint disparar notificação em `set_row_data`. Não foi verificado empiricamente.

### H2: `meter_timer` é descartado após `playback::install()` retornar

Em `playback.rs:110`, `meter_timer` é uma variável local (`Rc<RefCell<Timer>>`). O timer é iniciado na linha 130, mas o `Rc` não é armazenado em nenhum struct após o retorno de `install()`. Se o Slint não mantém o timer vivo independentemente do handle, o ticker para e `tick()` nunca é chamado depois do startup.

Evidência contra: `playhead_timer` (linha 111) segue o mesmo padrão e o playhead parece funcionar.

### H3: `AudioBridge.handle` é `None` (dispositivo não aberto)

Se o dispositivo de áudio não abriu, `AudioBridge::meters()` retorna um `MeterBank` estático silencioso (`SILENT` em `audio_bridge.rs:113-119`). `MeterPump::tick()` leria zeros e publicaria zeros. Os metros mostrariam silêncio, não sinal verde.

Evidência contra: o usuário diz que o áudio toca, o que implica dispositivo aberto.

## Plano de ação

### Passo 1: Confirmar se o timer está disparando e se há dados no MeterBank

Adicionar logs temporários em `MeterPump::tick()`:
- Log quando `tick()` é chamado (contador de ticks)
- Log dos valores lidos de `bank.read(0)` (peak_left, rms_left)
- Log de `bank.channel_count()` vs `channels`

Isso confirma se H2 ou H3 são reais.

### Passo 2: Corrigir a notificação de mudança do modelo (correção principal para H1)

Após publicar todos os canais em `MeterPump::tick()`, notificar o Slint que o modelo mudou. Opções:

**Opção A (recomendada):** Substituir `set_row_data` por `set_vec` no `publish()` — reconstrói o `Vec` inteiro a cada tick, o que dispara `model_changed`. Custo: 6 × channels escritas por tick (~30Hz), aceitável.

**Opção B:** Após o loop de canais, re-setar a propriedade no UI:
```rust
ui.set_channel_meters(self.meters.values.clone().into());
```
Isso força o Slint a re-avisar as bindings. Precisa de acesso ao `ui` dentro de `MeterPump`.

**Opção C:** Chamar `model_changed()` manualmente se a API do Slint 1.18 expor para `VecModel`.

Escolher a opção mais simples que compilar. Provável: Opção A.

### Passo 3: Garantir que o timer permaneça vivo (correção para H2)

Armazenar `meter_timer` em algum lugar com vida útil longa (ex: struct `App` ou `Bridge`), para garantir que não seja descartado após `install()` retornar.

### Passo 4: Validar

- Rodar `cargo check` — deve compilar sem erros
- Rodar o app, tocar áudio, observar se as barras verdes dos metros sobem
- Se não subir, verificar os logs do Passo 1 para descartar H2/H3

## Arquivos afetados

1. `app/src/meters.rs` — correção principal (`publish()` ou `tick()`)
2. `app/src/playback.rs` — armazenar `meter_timer` (se H2 confirmado)

## Critério de sucesso

Ao tocar áudio no DAW, as barras dos metros (mixer e timeline) exibem nível verde proporcional ao sinal, com peak-hold (linha branca) e decaimento quando o sinal para.

# Abyss

[![ci](https://github.com/cesarfilho/omarchy-abyss/actions/workflows/ci.yml/badge.svg)](https://github.com/cesarfilho/omarchy-abyss/actions/workflows/ci.yml) [![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Screensaver gamificado do fundo do mar para o [Omarchy](https://omarchy.org/). Um oceano em pixel-art roda no terminal enquanto você está longe do computador, evolui com o tempo ocioso e rende XP, troféus e pontos. Um plugin do shell dispara o screensaver na ociosidade e mostra suas estatísticas na barra.

*English: [README.md](README.md)*

<p align="center"><img src="docs/screenshots/swim.gif" alt="Cardumes nadando sobre um recife" width="720"></p>

## O que ele faz

- **Oceano vivo:** seis espécies (peixe-palhaço, cirurgião-azul, sardinha, peixe-lanterna, peixe-víbora, tamboril), águas-vivas, baleias, um **tubarão** de corpo articulado que faz os cardumes se abrirem à sua passagem, bolhas, neve marinha e marés bioluminescentes, tudo desenhado proceduralmente. Cada cardume ou peixe solitário escolhe o próprio rumo e passeia; os seguidores vão na trilha do líder e os peixes inclinam o focinho ao subir ou descer.
- **Um fundo que vale olhar:** cinco planos de paralaxe; floresta de algas, capim-marinho, rochas, leques-do-mar, anêmonas, estrelas, ouriços e caranguejos nas águas rasas, vermes-tubo e penas-do-mar brilhantes no fundo; areia ondulada com cáusticas e uma superfície que ondula; criaturas em profundidades diferentes (as distantes ficam menores e mais enevoadas).
- **O que repousa no fundo:** veleiro naufragado, submarino, avião, ruínas gregas, âncora, baú do tesouro, ânforas, esqueleto de baleia, fumarolas hidrotermais e, na zona abissal, um transatlântico partido ao meio. Cada um é uma descoberta.
- **Vários monitores, um só mar:** os monitores formam um único panorama; peixes e elementos do fundo atravessam a divisão entre telas. Cada execução visita uma das 12 regiões inspiradas em correntes oceânicas reais. A corrente regional desloca a câmera continuamente, enquanto a maré simulada oscila num ciclo de 12 h 25 min. Rochas, corais e estruturas mantêm suas posições no mundo.
- **Condições do mar no HUD:** a bússola de oito rumos mostra o fluxo combinado da corrente e da maré. O painel exibe região, velocidade, fase da maré e, alternadamente, vento/ondas/visibilidade e temperatura/salinidade/pressão. Vento e ondas afetam a superfície; corrente e maré movem o panorama e as partículas; ressurgência e tempestades ocasionais alteram cor, luz, cardumes e visibilidade. São valores ilustrativos da simulação, não medições ou previsões locais.
- **Progressão:** o tempo ativo rende XP fixo de 0,1 XP/s (6 XP/min), sem variar por zona, sequência ou evento. A curva quadrática atual chega à zona Hadal em cerca de 31,25 horas ativas. O recife perde um coral por dia após três dias de ausência; a sequência reinicia ao faltar um dia.
- **Bestiário e missões:** vinte criaturas e paisagens para descobrir (o primeiro avistamento rende aviso e pontos) e três metas diárias de tempo, incluindo 15 minutos ativos no dia. O progresso é salvo entre sessões e não depende de avistamentos aleatórios.
- **22 troféus, sequência e pontos:** ganhe um ponto por minuto ativo, além dos bônus por missões, troféus e descobertas. Troque pontos por três paletas cosméticas (300, 900 e 1.800 pontos), sem vantagem no jogo.
- **Eventos de temporada** mudam o visual e mantêm seus troféus: Noite das Águas-vivas (24/10 a 2/11), Luzes do Natal (20/12 a 2/1) e Semana do Oceano (5 a 12/6). Eles não alteram o XP.
- **Segue o tema do Omarchy:** água, brilho do plâncton e HUD puxam para as cores do tema ativo (`palette: "classic"` desliga).
- **Adapta-se à tela:** o desenho pausa com o monitor apagado e usa uma taxa de quadros menor na bateria; a resolução e as taxas podem ser ajustadas nas configurações.

## Galeria

| Um tubarão passa e o cardume se abre | O transatlântico, a 3.800 m |
|---|---|
| <img src="docs/screenshots/shark.gif" alt="Um tubarão cruzando um cardume" width="420"> | <img src="docs/screenshots/titanic.gif" alt="Um transatlântico partido na planície abissal" width="420"> |

| Um naufrágio | Luzes do Natal |
|---|---|
| <img src="docs/screenshots/wreck.png" alt="Naufrágio" width="420"> | <img src="docs/screenshots/events.png" alt="Plâncton de temporada" width="420"> |

### O plugin do shell

Um ícone na barra abre um painel com cinco abas para nível, profundidade, atividade, missões, bestiário, troféus, paletas cosméticas e ajustes. Tab troca de seção; as setas navegam pelos itens do bestiário e dos troféus; Enter abre a descrição e o progresso de descoberta ou desbloqueio. Os ajustes permitem escolher qual screensaver inicia na ociosidade, definir o tempo de inatividade, mudar a resolução, iniciar na hora e apagar o progresso após confirmação. A dica do ícone mostra o modo ativo e informa que o botão direito alterna entre os modos.

## Requisitos

- Omarchy (Hyprland + Quickshell) com foot, Alacritty, Ghostty ou Kitty como terminal padrão.
- `jq` e `socat` (vêm com o Omarchy); `ttfx` para o resumo ao sair (vem com o Omarchy, opcional).
- Rust (`cargo`) para compilar o programa `abyss`, ou o pacote Arch abaixo.

## Instalação

O Abyss tem duas partes: o programa `abyss` (desenha o mar e guarda seu progresso) e um plugin do shell (dispara o programa na ociosidade e mostra o painel). Instalar só o plugin não faz nada; o painel avisa enquanto o programa não estiver instalado.

### Pelo marketplace do Omarchy

```bash
omarchy plugin add https://github.com/cesarfilho/omarchy-abyss.git --enable
cd ~/.config/omarchy/plugins/io.github.cesarfilho.abyss
./install.sh --setup
```

O primeiro comando adiciona o plugin (o repositório inteiro é clonado na pasta do plugin). O `./install.sh` compila o programa e instala `abyss`, `abyss-launch`, `abyss-screensaver` e `abyss-setup` em `~/.local/bin`; o `--setup` também liga o Abyss ao Omarchy (abaixo). Depois de um `omarchy plugin update`, rode o `./install.sh` de novo para recompilar o programa.

`install.sh` e `uninstall.sh` respeitam `XDG_DATA_HOME`, `XDG_CONFIG_HOME` e `XDG_STATE_HOME`. Para os executáveis, aceitam `ABYSS_BIN_DIR`; sem essa variável, usam `~/.local/bin`. Se definir um diretório próprio, adicione-o ao `PATH`.

### A partir de um clone

```bash
git clone https://github.com/cesarfilho/omarchy-abyss.git
cd omarchy-abyss
./install.sh --setup
```

Sem `--setup` só instala; rode `abyss-setup enable` quando quiser plugar.

### Pacote Arch

`packaging/PKGBUILD` monta a versão publicada (tag no GitHub): `cd packaging && makepkg -si` (use `makepkg -p PKGBUILD.dev` para empacotar este diretório). Depois rode `abyss-setup enable` como seu usuário.

### O que o `abyss-setup enable` altera

Só roda quando você pede, é idempotente e faz backup de cada arquivo que edita (`*.bak.<timestamp>`):

1. habilita o plugin e põe o widget no lado direito da barra;
2. muda `idle.screensaver` para `0` em `~/.config/omarchy/shell.json`, para o temporizador embutido não disparar junto (o valor anterior fica em `~/.config/abyss/setup.json`);
3. aponta o item **Screensaver** do menu para o Abyss (uma entrada entre os marcadores `abyss-menu-start`/`abyss-menu-end` em `~/.config/omarchy/extensions/omarchy-menu.jsonc`);
4. reinicia o shell.

Depois confira tudo com `abyss doctor`.

## Remoção

```bash
abyss-setup disable                                   # devolve idle.screensaver e o menu, tira o widget da barra
omarchy plugin remove io.github.cesarfilho.abyss      # remove a pasta do plugin
./uninstall.sh                                        # num clone: apaga também o programa (--purge apaga seu progresso)
```

Seu progresso em `$ABYSS_STATE_DIR`, `$XDG_STATE_HOME/abyss` ou `~/.local/state/abyss`, e as configurações em `$ABYSS_CONFIG_DIR`, `$XDG_CONFIG_HOME/abyss` ou `~/.config/abyss`, são mantidos, a menos que use `--purge`.

## Uso

```bash
abyss run [--screensaver] [--no-intro] [--monitor NOME]   # roda a cena
abyss status [--json]                                     # nível, XP, série, bestiário, missões e troféus
abyss mode [abyss|default|toggle]                         # qual screensaver o plugin dispara
abyss quality [low|medium|high|ultra|max]                 # resolução (padrão: ultra)
abyss idle [SEGUNDOS]                                     # tempo de inatividade (padrão: 150 s)
abyss start                                               # inicia agora o screensaver configurado
abyss reset                                               # apaga estatísticas, XP e conquistas
abyss wake                                                # encerra mostrando o resumo
abyss stop                                                # encerra sem o resumo
abyss doctor [--fix]                                      # confere a instalação; --fix limpa restos de uma falha
abyss config                                              # mostra a configuração ativa
abyss snapshot [zona 0-5] [recife 0-30] [--ppm ARQ] [--t SEG] [--size LxA] [--season ID] [--landmark ID] [--region ID]
                                                          # desenha um quadro (ANSI, ou imagem PPM sem tocar a tela)
```

Qualquer tecla fecha o screensaver. Na barra, clique no ícone para ver as estatísticas; o botão direito alterna entre o Abyss e o screensaver padrão.

`--region` fixa a corrente em uma captura: `gulf-stream`, `labrador`, `brazil`, `benguela`, `kuroshio`, `oyashio`, `california`, `humboldt`, `agulhas`, `west-australian`, `east-australian` ou `antarctic`. Sem essa opção, a captura mantém o cenário anterior para comparação.

### Configuração

`$ABYSS_CONFIG_DIR/config.json` quando definido; caso contrário, `$XDG_CONFIG_HOME/abyss/config.json` ou `~/.config/abyss/config.json` (criado no primeiro uso; o painel e a CLI editam o mesmo arquivo):

| Chave | Padrão | Significado |
|---|---|---|
| `mode` | `"abyss"` | `"abyss"` ou `"default"` (o screensaver padrão) |
| `quality` | `"ultra"` | `low` 18 · `medium` 14 · `high` 12 · `ultra` 10,5 · `max` 9 (tamanho da fonte do terminal: menor = grade de pixels mais fina; o texto do HUD acompanha) |
| `fps` | `30` | quadros por segundo na tomada |
| `battery_fps` | `15` | quadros por segundo na bateria |
| `palette` | `"theme"` | `"theme"` segue o tema do Omarchy, `"classic"` mantém as cores originais |
| `seasons` | `true` | visuais e troféus sazonais |
| `idle_secs` | `150` | segundos de inatividade até o plugin iniciar o screensaver (10–7200) |

### Dados

O progresso fica em `$ABYSS_STATE_DIR` quando definido; caso contrário, em `$XDG_STATE_HOME/abyss/` ou `~/.local/state/abyss/` (`save.json`, `stats.json`, `last_session.json`, `crash.log`). O `save.json` tem versão de esquema e é migrado quando muda; um arquivo ilegível é separado como `save.json.corrupt-*`, nunca sobrescrito. Apague a pasta para recomeçar do nível 0.

Gerencie paletas cosméticas com `abyss cosmetic buy ID` e `abyss cosmetic select ID`. Variáveis para teste: `ABYSS_SEASON=halloween|xmas|oceans` (força um evento), `ABYSS_STATE_DIR`, `ABYSS_CONFIG_DIR`, `ABYSS_THEME_FILE`.

## Problemas

`abyss doctor` lista o que está errado e como resolver. Ele também avisa quando o `omarchy-launch-screensaver` do Omarchy mudou depois que o `abyss-launch` foi copiado dele: rode `scripts/diff-launch.sh` para ver a diferença, porte o que importa e depois `abyss-setup record-upstream`.

Nada acontece na ociosidade? O `stay-awake` suprime o disparo, e o `doctor` diz se ele está ligado.

## Desenvolvimento

```bash
cargo build --release
cargo test                          # testes: estado, mundo, renderizador, socket, config, tema, tubarão, rotas
cargo clippy --all-targets -- -D warnings
cargo test --release bench_frame -- --ignored --nocapture   # onde o tempo de um quadro vai
scripts/e2e-idle.sh                 # caminho real de ociosidade na sua sessão: plugin -> lançador -> janelas -> acordar
scripts/make-gifs.py                # renderiza de novo as animações do README
```

`scripts/e2e-idle.sh --real` espera o temporizador de ociosidade de verdade (não toque no teclado). A captura de diagnóstico é criada com permissões exclusivas do dono em um diretório temporário único e removida ao sair. Notas de arquitetura em [`CLAUDE.md`](CLAUDE.md).

Depois de um CI bem-sucedido na `main`, o GitHub Actions cria a tag da versão em `Cargo.toml`, atualiza o checksum do PKGBUILD de release e atualiza a submissão aberta ao marketplace Omarchy (ou abre um pedido de verificação se o plugin já estiver listado). Configure o segredo de repositório `OMARCHY_MARKETPLACE_TOKEN` com um token de acesso refinado, limitado à leitura e escrita de Issues em `omacom/omarchy-plugin-marketplace`. Se a primeira execução for ignorada por falta do segredo, adicione-o e execute uma vez o workflow `release-marketplace`.

## Licença

[MIT](LICENSE) © 2026 Cesar Roberto Pamplona Filho.

## Créditos

`bin/abyss-launch` é adaptado do `omarchy-launch-screensaver` do Omarchy (MIT, © Basecamp / David Heinemeier Hansson). As ideias de corrente marinha, cardume em trilha de líder e corpo de tubarão em cadeia foram inspiradas em [stackedlol/ascii-sharks](https://github.com/stackedlol/ascii-sharks); nenhum código foi aproveitado.

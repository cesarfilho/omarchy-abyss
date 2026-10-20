# Abyss

Screensaver gamificado do fundo do mar para o [Omarchy](https://omarchy.org/). Um oceano em pixel-art roda no terminal enquanto você está longe do computador, evolui com o tempo ocioso e rende XP, troféus e pontos. Um plugin para o shell do Omarchy dispara o screensaver e mostra suas estatísticas na barra.

## O que ele faz

- **Oceano vivo:** peixes (palhaço, cirurgião azul, prateado, tamboril), águas-vivas, baleias, bolhas, neve marinha, brilho bioluminescente e raios de luz. Os peixes são desenhados proceduralmente, com sombreado, nadadeiras, olhos e padrões.
- **2,5D:** o fundo tem cinco planos de paralaxe, os bichos nadam em profundidades diferentes (os distantes ficam menores e mais enevoados), há sombras no chão e rochas e algas no primeiro plano.
- **Vários monitores:** os monitores formam um único mar. Quando um peixe sai de uma tela, ele entra na outra.
- **Progressão:** cada minuto ocioso rende XP. Subir de nível aprofunda o mar por seis zonas (Superfície, Zona de Luz, Crepúsculo, Meia-noite, Abissal e Hadal), que liberam novas espécies. O recife cresce com o tempo e murcha se você ficar dias sem usar.
- **Conquistas:** 16 troféus, série de dias seguidos e pontuação. Cada conquista aparece como um aviso sobre o mar, em todos os monitores. Ao acordar o computador, um resumo mostra o que aconteceu enquanto você estava fora.
- **Plugin do shell:** ícone na barra com um painel de estatísticas (nível, XP, profundidade, atividade dos últimos 14 dias, troféus com progresso), seletor entre o Abyss e o screensaver padrão do Omarchy e seletor de resolução.

## Requisitos

- Omarchy (Hyprland e Quickshell) com um destes terminais como padrão: foot, Alacritty, Ghostty ou Kitty.
- Rust (cargo) para compilar.
- `ttfx` (já vem com o Omarchy), usado no resumo ao sair.

## Instalação

```bash
./install.sh
omarchy plugin enable io.github.cesarfilho.abyss
omarchy bar put io.github.cesarfilho.abyss --section right
```

Depois, desligue o screensaver embutido para ele não disparar junto, editando `~/.config/omarchy/shell.json` (faça um backup antes):

```json
"idle": { "lock": 1800, "screensaver": 0 }
```

O plugin passa a iniciar o screensaver após 150 s de ociosidade (`timeoutSeconds` em `plugin/Service.qml`). Se o plugin ou o QML mudar, rode `omarchy restart shell`.

Para o item **Screensaver** do menu do Omarchy abrir o Abyss, adicione ao `~/.config/omarchy/extensions/omarchy-menu.jsonc`:

```jsonc
"system.screensaver": {"icon":"󱄄","label":"Screensaver","action":"abyss-screensaver force"}
```

## Uso

```bash
abyss run [--screensaver] [--no-intro] [--monitor NOME]   # roda a cena
abyss status [--json]                                     # nível, XP, série e troféus
abyss mode [abyss|default|toggle]                         # qual screensaver o plugin dispara
abyss quality [low|medium|high|ultra|max]                 # resolução (padrão: ultra)
abyss wake                                                # encerra o screensaver mostrando o resumo
abyss snapshot [zona 0-5] [recife 0-30]                   # imprime um quadro ANSI e sai
```

Qualquer tecla fecha o screensaver. No painel da barra, clique no ícone para abrir as estatísticas; o clique com o botão direito alterna entre o Abyss e o screensaver padrão.

### Resolução

A resolução é o tamanho da fonte do terminal do screensaver, e o texto do HUD acompanha esse tamanho.

| Nível | Fonte |
|---|---|
| low | 18 |
| medium | 14 |
| high | 12 |
| ultra | 10,5 |
| max | 9 |

## Onde ficam os dados

Tudo em `~/.local/state/abyss/` (`save.json`, `stats.json`, `mode`, `quality`, `events.json`, `last_session.json`). Para recomeçar do nível 0, apague essa pasta.

## Desenvolvimento

```bash
cargo build --release
cargo test
```

Mais detalhes sobre a arquitetura e a integração com o Omarchy estão em [`CLAUDE.md`](CLAUDE.md).

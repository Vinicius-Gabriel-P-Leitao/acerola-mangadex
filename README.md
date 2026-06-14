# Acerola MangaDex

CLI em Rust para baixar capitulos do MangaDex como arquivos CBZ.

O app recebe um link de titulo do MangaDex ou um UUID de manga, busca os
capitulos no idioma configurado, baixa as paginas pelo AtHome e gera arquivos
CBZ dentro de uma pasta com o nome do manga.

## O que ele faz

- Baixa capitulos do MangaDex em CBZ.
- Filtra o feed por idioma antes de paginar a API.
- Hoje suporta `pt-br`.
- Permite baixar todos os capitulos, um indice, um intervalo de indices, um
  numero real de capitulo ou um intervalo por numero.
- Baixa a cover do manga quando disponivel.
- Pula arquivos CBZ que ja existem.
- Respeita timeouts, retries e intervalos entre chamadas para reduzir risco de
  limite de taxa.

## Requisitos

- Rust instalado.
- `cargo-nextest` instalado para rodar os testes.
- `cargo-make` instalado para rodar o fluxo padrao do projeto.

## Rodar em desenvolvimento

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk"
```

Se algum argumento nao for informado, a CLI pergunta no terminal.

## Gerar binario final

```bash
cargo build --release
```

No Windows, o executavel fica em:

```text
target\release\acerola-mangadex.exe
```

No Linux, o executavel fica em:

```text
target/release/acerola-mangadex
```

No Linux o binario normalmente nao tem extensao. Para deixar disponivel no
terminal, copie para uma pasta que esteja no `PATH`, como `/usr/local/bin`.

```bash
sudo cp target/release/acerola-mangadex /usr/local/bin/
sudo chmod +x /usr/local/bin/acerola-mangadex
```

O app nao precisa de administrador para executar. Ele so precisa de permissao
de escrita no diretorio de saida escolhido.

## Exemplos

Baixar todos os capitulos:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --selection all
```

Baixar por indice da lista:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --selection 401
```

Baixar intervalo por indice:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --selection 100-200
```

Baixar pelo numero real do capitulo:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --chapter 133
```

Baixar intervalo pelo numero real do capitulo:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --chapter 100-200
```

Escolher pasta de saida:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --output "C:\Users\vinicius\Downloads"
```

Escolher formato do nome do CBZ:

```bash
cargo run -- "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk" --chapter 163 --name-format chapter-title
```

Formatos aceitos:

- `chapter-title`: `Ch. 163 - A Sombra de uma Ideia (1).cbz`
- `number`: `163.cbz`

## Opcoes principais

- `-o`, `--output`: diretorio onde a pasta do manga sera criada.
- `-s`, `--selection`: selecao por indice. Aceita `all`, `401` ou `100-200`.
- `-c`, `--chapter`: selecao pelo numero real do capitulo. Aceita `133`,
  `0.01` ou `100-200`.
- `-l`, `--language`: idioma. Atualmente apenas `pt-br`.
- `--name-format`: formato do nome do CBZ. Aceita `chapter-title` ou `number`.

`--selection` e `--chapter` nao podem ser usados juntos, porque representam
formas diferentes de selecionar capitulos.

## Saida dos arquivos

A estrutura gerada e:

```text
Diretorio escolhido/
  Nome do manga/
    cover.jpg
    133.cbz
    134.cbz
```

Ou, usando `--name-format chapter-title`:

```text
Diretorio escolhido/
  Nome do manga/
    cover.jpg
    Ch. 133 - Titulo do capitulo.cbz
```

## Validacao do projeto

Rodar o fluxo padrao:

```bash
cargo make
```

Esse fluxo roda formatacao, clippy e testes com nextest.

Gerar documentacao local:

```bash
cargo doc --no-deps --document-private-items
```

A documentacao fica em:

```text
target/doc/acerola_mangadex/index.html
```

## Estrutura do codigo

- `cmd`: comandos da CLI, argumentos, prompts e relatorio final.
- `core`: regras de negocio, selecao de capitulos, nomes de arquivos e download.
- `data`: cliente MangaDex e modelos das respostas da API.
- `infra`: configuracao, erros e limitador de taxa.

# EnderCliTools Module System Spec (Draft v0.2)

Status: Draft  
Format language: this spec keeps key terms in English, examples in your style.

## 1) Ziele

- Erweiterbares `ect`-Modulsystem mit klaren, strikten Regeln.
- Zwangs-Dateinamen (inkl. ID + Mindest-ECT-Version + Modulversion).
- Manifest als Pflichtquelle für Metadaten + Abhängigkeiten.
- Sichere Installation per SHA-512 + HTTPS by default.
- Modul-Entwicklung per `ect module build`.

---

## 2) Namens- und Versionsmodell

## 2.1 Modul-ID

```text
{id} = {author}.{module_name}
```

Regeln:
- `author`: lowercase alnum + `-` (z. B. `endkind`)
- `module_name`: snake_case oder kebab-case, lowercase
- Komplettregex (praktisch):
  - `author`: `^[a-z0-9][a-z0-9-]*$`
  - `module_name`: `^[a-z0-9][a-z0-9_-]*$`
  - `id`: `{author}.{module_name}`

## 2.2 Versionsformate

- **ECT-Mindestversion im Dateinamen:** `breaking.feature.patch` (SemVer-ähnlich 3-teilig)
- **Modulversion:** `breaking.feature.patch`
- **Manifest `requirements.ect`:** Range (z. B. `>=1.2 <2`)

Hinweis: In deinem Beispiel kam teils `1.0` vor. Für technische Eindeutigkeit fixen wir auf 3 Teile (`1.0.0`).

---

## 3) Dateinamen (Zwang)

## 3.1 Paketdatei

```text
{id}.{min_ect_version}.{module_version}.ectm
```

Beispiel:

```text
endkind.docker.1.0.0.1.0.0.ectm
```

## 3.2 Checksum-Datei

```text
{id}.{min_ect_version}.{module_version}.sha512
```

Beispiel:

```text
endkind.docker.1.0.0.1.0.0.sha512
```

Wichtig:
- Dateiname ist **verpflichtend** (kein „nur Empfehlung“).
- `id` + `min_ect_version` + `module_version` im Dateinamen müssen mit Manifest konsistent sein.

---

## 4) Pakettyp und Struktur

- Archivtyp: **ZIP**
- Sichtbare Endung: `.ectm`

Pflichtstruktur im Archiv:

```text
manifest.toml
bin/<platform>/<binary_name>
```

Beispiel:

```text
bin/linux-x86_64/docker
bin/windows-x86_64/docker.exe
```

`platform` ist Liste im Manifest (Pflicht), und jede Plattform muss auf vorhandene Binaries mappen.

---

## 5) Manifest (`manifest.toml`) v1

## 5.1 Pflichtfelder

- `id` → `{author}.{module_name}`, muss mit Dateiname übereinstimmen
- `author`
- `name` → muss dem `module_name` entsprechen
- `display_name`
- `version` → `breaking.feature.patch`
- `[requirements]` mit mindestens `requirements.ect`
- `platform` → Liste unterstützter Plattformen

## 5.2 Optional

- `description`
- `[info]`
  - `homepage`
  - `source_code`
  - `license`
  - `license_file`
  - weitere URLs als key/value (z. B. `support = "..."`)
- `[aliases.*]`

## 5.3 GitHub-Konsistenz (wenn Quelle GitHub)

Wenn Installation per GitHub-Shortcut (`author/repo`) erfolgt:
- `author` soll dem GitHub Owner entsprechen.
- `name` soll dem GitHub Repo-Namen entsprechen (normalisiert, wenn nötig).

Bei harter Abweichung: Warnung oder Fehler (MVP-Einstellung noch offen, siehe offene Punkte).

---

## 6) Requirements-Modell

## 6.1 ECT (Pflicht)

```toml
[requirements]
ect = ">=1.2 <2"
```

Regel:
- Range parsing SemVer-kompatibel.
- Install blockieren, wenn aktuelles `ect` nicht matcht.
- Optionales Override-Flag möglich (siehe Security/Overrides).

## 6.2 Modul-Abhängigkeiten (optional)

```toml
[requirements.modules."endkind.docker"]
version = ">=1.0.0"

[requirements.modules."endkind.docker".source]
github = "endkind/ect_docker"

[requirements.modules."endkind.docker".source.url]
url = "https://example.com/endkind.docker.1.0.0.1.0.0.ectm"
sha512 = "https://example.com/endkind.docker.1.0.0.1.0.0.sha512"

[requirements.modules."endkind.docker".source.path.windows]
path = "\\\\192.168.1.200/ect/endkind.docker.1.0.0.1.0.0.ectm"
sha512 = "https://example.com/endkind.docker.1.0.0.1.0.0.sha512"

[requirements.modules."endkind.docker".source.path.linux]
path = "smb://192.168.1.200/ect/endkind.docker.1.0.0.1.0.0.ectm"
sha512 = "https://example.com/endkind.docker.1.0.0.1.0.0.sha512"
```

Regeln:
- `version` ist Pflicht pro Modul-Dependency.
- `source.*` optional, aber wenn vorhanden, muss mindestens ein auflösbarer Source-Typ da sein.

## 6.3 Environment-Abhängigkeiten (optional)

Beispielstruktur (dein Modell):

```toml
[requirements.environment.windows.docker_desktop]
version = ">=4.6"

[requirements.environment.windows.docker_desktop.check]
exist = "docker --help"
version = "docker --version"

[requirements.environment.windows.docker_desktop.source]
winget = "Docker.DockerDesktop"
url = "https://desktop.docker.com/win/main/amd64/Docker%20Desktop%20Installer.exe"
homepage = "https://docs.docker.com/desktop/setup/install/windows-install/"
```

Regeln:
- `check.exist` ist Pflicht sobald ein Environment-Requirement definiert ist.
- `source`-Block ist Pflicht sobald ein Environment-Requirement definiert ist.
- Installer-Hinweise (`winget`, `brew`, `deb`, `url`, `homepage`) sind metadata/hints, keine automatische Installation im MVP.

---

## 7) Alias-System

Beispiel:

```toml
[aliases.dps]
exec = "docker ps"
command = true

[aliases.dpsa]
exec = "docker ps -a"
command = false
```

Semantik:
- `exec` Pflicht
- `command` optional, default `false`
- `command=true` macht Alias über `ect <alias> ...` ansprechbar
- Mapping:
  - `ect dps --last 5` → `ect module run endkind.docker docker ps --last 5`

Kollisionsregel:
1. Core-Commands gewinnen immer.
2. Danach echte Modul-IDs.
3. Danach Alias mit `command=true`.

---

## 8) CLI-Kommandos

```bash
ect module install <source> [flags]
ect module list
ect module remove <id>
ect module run <id> <binary> -- [args...]
ect module build <path> [flags]
ect module info <id|file|url>
```

Zusätzlich:

```bash
ect <id> ...
ect <alias> ...   # nur bei command=true
```

Dein gewünschtes Run-Verhalten ist damit direkt enthalten:
- Beispiel: `ect module run endkind.docker docker -- ps -a`

(Alternative Parser-Variante ohne separates `--` ist möglich, muss aber sauber spezifiziert werden.)

---

## 9) Security, TLS, HTTP, Checksum

Default:
- nur HTTPS
- TLS strict
- `.sha512` Pflicht

Override-Flags:

```bash
--allow-insecure-tls
--allow-http
--allow-missing-sha512
--allow-major-compat-mismatch
```

Regeln:
- Bei Nutzung eines Override-Flags klare `UNSAFE`-Warnung.
- Installationseintrag markiert `unsafe_install=true`.

Checksum:
- SHA-512 Datei muss exakt zum Artefakt passen.
- Dateiname der `.sha512` muss ebenfalls zum Schema passen.

---

## 10) Registry (lokal)

Pfade:
- Module: `~/.local/share/enderclitools/modules/<id>/<version>/`
- Registry: `~/.config/enderclitools/modules.toml`

Beispiel:

```toml
schema = "ect.modules.registry.v1"

[[modules]]
id = "endkind.docker"
author = "endkind"
name = "docker"
display_name = "Docker Helpers"
version = "1.0.0"
min_ect_version = "1.0.0"
path = "/home/user/.local/share/enderclitools/modules/endkind.docker/1.0.0"
sha512 = "..."
unsafe_install = false
installed_at = "2026-03-04T21:00:00Z"
source = "github:endkind/ect_docker"
```

---

## 11) Build (`ect module build`)

Ablauf:
1. Manifest laden/validieren
2. Dateinamen aus Manifest deterministisch erzeugen
3. ZIP erzeugen (`.ectm`)
4. SHA-512 berechnen und `.sha512` erzeugen
5. Konsistenzchecks (id/version/min_ect_version)
6. Output im Zielordner

Vorschlagsflags:

```bash
--out-dir <dir>
--platform <platform>      # repeatable
--min-ect <version>        # optional override, sonst aus requirements ableiten
```

---

## 12) Validierungsregeln (hart)

Installation schlägt fehl bei:
- ungültigem Dateinamen
- Dateiname/Manifest-Mismatch (`id`, Versionen)
- fehlendem Pflichtfeld im Manifest
- ungültiger Version/Range
- fehlendem Plattform-Binary
- fehlender/ungültiger SHA-512 (ohne override)
- ECT-Compat-Mismatch (ohne override)

---

## 13) Offene Punkte (für final v1)

1. Exaktes Mapping GitHub-Repo-Name ↔ `name` bei snake/kebab Abweichungen.
2. Sollen Environment-`source` Hinweise später Auto-Install unterstützen?
3. Soll `ect module run <id> <binary>` Pflicht bleiben oder bei single-entrypoint optional werden?
4. Alias-Argumentquoting: shell-like parsing oder strict split?

---

## 14) Kurzfazit

Deine Kernideen sind jetzt sauber in eine strikte, implementierbare Form gegossen:
- Zwangsdateinamen ✅
- Pflicht-Manifest mit klaren Feldern ✅
- Requirements für ect/modules/environment ✅
- Alias-Mechanik ✅
- ZIP/.ectm + `.sha512` ✅

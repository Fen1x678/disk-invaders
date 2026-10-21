<p align="center"><img src="docs/icon.png" width="120" alt="Иконка Disk Invaders"></p>

<h1 align="center">DISK INVADERS</h1>
<p align="center"><b>8-битный чистильщик для 64-битного диска.</b><br>
Находит кэши, мусор от сборок, старые загрузки, дубликаты и огромные файлы на macOS и Windows — и уничтожает их в стиле аркады 80-х.</p>

<p align="center"><img src="docs/demo.gif" alt="Демо" width="760"></p>

> 🇬🇧 [Read in English](README.md)

## Возможности

- **Сканирует всё**: весь компьютер, домашнюю папку или любую папку/диск. Параллельный обход на Rust.
- **9 уровней мусора**: кэш приложений, дев-мусор (`node_modules`, `target`, Xcode DerivedData, Gradle, `.venv`…), логи и дампы, Корзина, установщики, старые загрузки, бэкапы iPhone, дубликаты, большие файлы.
- **Поиск дубликатов**: размер → частичный хэш → полный BLAKE3. Только побайтово одинаковые файлы.
- **Проводник** с treemap-картой: «куда делось место?»
- **🧠 МОЗГ ЗАХВАТЧИКОВ**: маленькая нейросеть, написанная с нуля (без ML-библиотек и облаков), оценивает каждую находку «удалить / проверить / оставить», учится на каждом вашем решении и растёт в уровнях. С бесплатным локальным **голосом** (Ollama) она ещё и общается про ваш диск. [Как это устроено](docs/BRAIN.md).
- **Безопасность**: по умолчанию всё уходит в Корзину (режим «НАВСЕГДА» — без возврата). Rust-модуль защиты не даст удалить системные папки, домашнюю папку, «Документы» и т.п., SSH-ключи и связки ключей.
- **Ретро-дизайн 80-х**: темы synthwave, зелёный фосфор, янтарь и Commodore 64, эффект ЭЛТ-монитора, 8-битные звуки.
- Интерфейс на **русском и английском**.

## 📥 Установка (программировать не нужно)

1. Откройте **[Releases](../../releases/latest)** и скачайте файл для своего компьютера:

   | Компьютер | Какой файл скачать |
   |---|---|
   | 🍎 Mac (Apple Silicon или Intel) | `Disk.Invaders_x.y.z_universal.dmg` |
   | 🪟 Windows 10 / 11 | `Disk.Invaders_x.y.z_x64-setup.exe` |

2. Установите:

<details>
<summary><b>🍎 macOS: по шагам</b></summary>

1. Откройте `.dmg` и перетащите **Disk Invaders** в **Программы**.
2. Запустите из «Программ». Приложение пока не заверено Apple, поэтому в первый раз macOS скажет, что *не может проверить разработчика*:
   - нажмите **Готово**, откройте **Системные настройки → Конфиденциальность и безопасность**, прокрутите вниз и нажмите **Всё равно открыть**;
   - или один раз выполните в Терминале: `xattr -dr com.apple.quarantine "/Applications/Disk Invaders.app"`
3. Для полного сканирования добавьте приложение в **Системные настройки → Конфиденциальность и безопасность → Полный доступ к диску** и перезапустите его.

</details>

<details>
<summary><b>🪟 Windows: по шагам</b></summary>

1. Запустите файл `-setup.exe`.
2. Если появится **«Система Windows защитила ваш компьютер»**: нажмите **Подробнее → Выполнить в любом случае** (приложение пока без подписи).
3. Завершите установку и запустите **Disk Invaders** из меню «Пуск».

</details>

## 🛠 Сборка из исходников

Только если хотите менять код. Все команды выполняются **в папке проекта**, в той, где лежит `package.json`.

<details>
<summary><b>🍎 macOS</b></summary>

1. Установите инструменты (один раз):
   ```sh
   xcode-select --install                                          # компиляторы Apple
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh  # Rust
   ```
   и [Node.js LTS](https://nodejs.org). Закройте и заново откройте Терминал.
2. Проверьте: `node -v` и `cargo -V` выводят версии.
3. Скачайте код и запустите:
   ```sh
   git clone https://github.com/Fen1x678/disk-invaders.git
   cd disk-invaders
   npm install
   npm run app
   ```
   Первая сборка идёт 5–10 минут, следующие быстрые.
4. Своё приложение: `npm run app:build` → `target/release/bundle/macos/Disk Invaders.app`

</details>

<details>
<summary><b>🪟 Windows</b></summary>

1. Установите инструменты (один раз):
   - [Node.js LTS](https://nodejs.org)
   - [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/): отметьте **«Разработка классических приложений на C++»**
   - [Rust](https://rustup.rs): запустите `rustup-init.exe`, на всё жмите Enter
   - WebView2 уже есть в Windows 10/11.

   Закройте и заново откройте командную строку.
2. Проверьте: `node -v` и `cargo -V` выводят версии.
3. Скачайте код и запустите (надёжнее короткий путь без пробелов и кириллицы):
   ```bat
   mkdir C:\dev
   cd C:\dev
   git clone https://github.com/Fen1x678/disk-invaders.git
   cd disk-invaders
   npm install
   npm run app
   ```
   Нет git? **Code → Download ZIP**, распакуйте и перейдите (`cd`) в папку, где лежит `package.json`
   (ZIP часто распаковывается как `disk-invaders-main\disk-invaders-main`).
4. Свой установщик: `npm run app:build` → `target\release\bundle\nsis\`

</details>

<details>
<summary><b>❓ Что-то пошло не так</b></summary>

| Ошибка | Что делать |
|---|---|
| `Could not read package.json` / `ENOENT` | Вы не в той папке. Перейдите в ту, где есть `package.json` (проверить: `dir` / `ls`). |
| `tauri: command not found` / `"tauri" не является командой` | Сначала выполните `npm install`. |
| `cargo: command not found` / `"cargo" не является командой` | Установите Rust и **заново откройте** терминал. |
| `link.exe not found` (Windows) | Установите Visual Studio Build Tools с **«Разработка классических приложений на C++»**. |
| `requires rustc 1.xx` | `rustup update` |
| Пустое окно | Вы открыли dev-сборку напрямую. Запускайте `npm run app` или соберите приложение `npm run app:build`. |

</details>

**Другие способы:**

- **Демо в браузере, без Rust:** `npm install`, затем `npm run dev` (фейковый диск).
- **Только сканер:** `cargo run --release -p invaders-core --example scan -- ~/`
- **Только мозг:** `cargo run --release -p invaders-core --example brain` (сдаёт выпускной экзамен)
- **Тесты ядра:** `cargo test -p invaders-core`

## Планы

- 🧠 **Invader-1.7B**: дообучить голос мозга на знаниях Disk Invaders и выложить как свою открытую модель. См. [BRAIN.md](docs/BRAIN.md).

## Научить мозг

Добавьте примеры в `brain/dataset.csv` (`путь,file|dir,размер_МБ,возраст_дней,delete|review|keep`) или нажмите в приложении **ВЫГРУЗИТЬ МОИ УРОКИ** и пришлите файл в pull request. CI проверит, что мозг сдаёт экзамен (`brain/exam.csv`) не хуже 85%.

## Участие

Pull request'ы приветствуются, в том числе первые в жизни. См. [CONTRIBUTING.md](CONTRIBUTING.md) и [задачи для новичков](docs/GOOD_FIRST_ISSUES.md).

Лицензия: [MIT](LICENSE)

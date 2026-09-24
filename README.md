# Document Archive

Document Archive is a modern, offline-first desktop foundation for storing and searching organizational records such as letters, receipts, invoices, meeting documents, reports, certificates, contracts, scanned images, and PDFs.

## Technology stack

- Tauri 2 desktop runtime
- React 19, TypeScript 6, and Vite 8
- pnpm package manager
- SQLite through the official Tauri SQL plugin
- Rust/Cargo for the native layer
- Git for source control

## Development setup

Requirements:

- Node.js 24+
- pnpm 11+
- Rust 1.77.2+ and Cargo
- Linux Tauri prerequisites: `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libxdo-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, and a font package

Install JavaScript dependencies and run the browser preview:

```bash
pnpm install
pnpm dev
```

Run the desktop application:

```bash
pnpm tauri dev
```

Build the production frontend and desktop bundle:

```bash
pnpm build
pnpm tauri build
```

Other checks:

```bash
pnpm lint
pnpm exec tsc -b
```

## Project structure

```text
src/
├── components/       # Reusable UI components (reserved)
├── pages/            # Feature pages (reserved)
├── layouts/          # Shared layouts (reserved)
├── services/         # Application services (reserved)
├── database/         # SQLite connection and future queries
├── types/            # Shared domain types (reserved)
├── utils/            # Small utilities (reserved)
├── hooks/            # React hooks (reserved)
└── assets/           # Frontend assets
src-tauri/             # Rust/Tauri configuration and native entrypoint
```

## Storage architecture

Actual PDF, image, and other document files will live in the filesystem. SQLite will store metadata and paths; document binaries will not be stored inside SQLite.

The planned application-data layout is:

```text
Application Data/
├── database/
│   └── documents.db
├── storage/
│   ├── letters/       ├── receipts/       ├── bills/
│   ├── meetings/     ├── reports/        ├── certificates/
│   ├── contracts/    ├── payments/       ├── photos/
│   └── other/
├── thumbnails/
├── backups/
└── logs/
```

The SQL connection module uses `sqlite:database/documents.db`. The Tauri SQL plugin resolves this relative path under the app’s OS-specific application-data/config location, not the source repository. The folders above are a future storage design and are not created or populated in Phase 2.

## Database architecture

SQLite is reserved for document metadata: titles, dates, types, reference numbers, people or organizations, tags, OCR text, file paths, and future audit fields. The final schema and migrations are intentionally postponed. Phase 2 only opens and closes the connection to verify the foundation.

## Phase 2 scope

This phase includes the Tauri/React/Vite foundation, the configured SQLite plugin, a small connection test, Git hygiene, a placeholder desktop UI, and the documented storage design.

Intentionally postponed: authentication, roles and permissions, document upload/import, OCR, search implementation, backups, deletion, version history, AI features, cloud synchronization, and mobile applications.

## Git and private data

The repository contains source code and configuration only. Database files, uploaded documents, storage folders, thumbnails, backups, logs, build output, temporary files, and secrets are ignored by `.gitignore`.

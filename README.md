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
├── components/       # Document form, list, details, and filters
├── pages/            # Feature pages (reserved for future extraction)
├── layouts/          # Shared layouts (reserved)
├── services/         # File selection, storage, and opening services
├── database/         # SQLite connection and document repository
├── types/            # Document domain types
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

The SQL connection module uses `sqlite:database/documents.db`. The Tauri SQL plugin resolves this relative path under the app’s OS-specific application-data/config location, not the source repository. On Linux this is currently under `~/.config/com.documentarchive.app/`; the equivalent OS-specific application-data directory is used elsewhere.

At startup, the Rust layer creates the database, storage category, thumbnails, backups, and logs directories. Documents are copied into `storage/<category>/<year>/<month>/` using a generated managed filename. A temporary copy is finalized before its metadata is inserted into SQLite.

## Database architecture

SQLite stores document metadata, including title, date, type, reference number, organization, department, description, tags, original/stored filenames, path, MIME type, size, SHA-256 hash, timestamps, and active/archived status. Migration `src-tauri/migrations/0001_create_documents.sql` creates the `documents` table and indexes for status, type, date, and file hash. Document binaries are never stored in SQLite.

## Phase 3 scope

This phase adds document selection, extension and size validation, SHA-256 hashing, duplicate detection, managed filesystem storage, metadata insertion, document listing, parameterized search/filtering, details, metadata editing, OS-default file opening, and archive status without permanent deletion.

Intentionally postponed: authentication, roles and permissions UI, OCR, backups, deletion, version history, AI features, cloud synchronization, and mobile applications.

## Git and private data

The repository contains source code and configuration only. Database files, uploaded documents, storage folders, thumbnails, backups, logs, build output, temporary files, and secrets are ignored by `.gitignore`.

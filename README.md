# Document Archive

Document Archive is a modern, offline-first desktop foundation for storing and searching organizational records such as letters, receipts, invoices, meeting documents, reports, certificates, contracts, scanned images, and PDFs.

## Phase 1 — Project foundation

The project foundation is complete, including the Tauri desktop shell, React frontend, Rust native layer, and development tooling.

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
- OCR prerequisites: `tesseract-ocr` with the required language data and `poppler-utils` for PDF text extraction and scanned-PDF page rendering

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

## Phase 2 — Database & document storage

### Storage architecture

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

### Database architecture

SQLite stores document metadata, including title, date, type, reference number, organization, department, description, tags, original/stored filenames, path, MIME type, size, SHA-256 hash, timestamps, and active/archived status. Migration `src-tauri/migrations/0001_create_documents.sql` creates the `documents` table and indexes for status, type, date, and file hash. Document binaries are never stored in SQLite.

## Phase 3 — Document management & search

This phase adds document selection, extension and size validation, SHA-256 hashing, duplicate detection, managed filesystem storage, metadata insertion, document listing, parameterized search/filtering, details, metadata editing, OS-default file opening, and archive status without permanent deletion.

Intentionally postponed: deletion, version history, AI features, cloud synchronization, and mobile applications.

## Phase 4 — OCR & document opening

When a document is saved, text-based PDFs are extracted with Poppler. Images and scanned PDFs are processed with the local Tesseract installation; OCR is best-effort, so a missing OCR runtime does not prevent the original document from being archived. Extracted text is stored in the `documents.ocr_text` SQLite column and is included in the existing search field. OCR is local and no document content is sent to an external service.

## Phase 5 — Authentication & roles

The application now requires a local account. On first launch, the setup wizard creates an administrator account. Passwords are hashed and verified in the Rust layer with Argon2id-compatible password hashes; plaintext passwords are never written to SQLite. The signed-in session exists only in application memory and is cleared on sign out.

The local roles are Administrator, Manager, Contributor, and Viewer. Administrators can create additional users from Settings. Viewers can search, inspect, and open documents; contributors and managers can add/edit/archive documents; administrators also manage users. Existing document metadata, filesystem storage, OCR, and search migrations remain unchanged.

## Phase 6 — Security, authorization & audit logging

Rust/Tauri is the authority for authentication and authorization. The React layer holds only an opaque, memory-only session token. Every protected command resolves that token to the active user and current database role before allowing the operation; frontend roles and user IDs are never trusted. Logout clears the backend session and frontend state. The SQL plugin remains available to run startup migrations, but its frontend capability is removed and application repositories use authenticated Tauri commands.

| Role | Documents | Users | Audit logs |
| --- | --- | --- | --- |
| Administrator | Full access | Create, update, delete, role/password changes | Read |
| Manager | Create, edit, delete, view, search, open | None | None |
| Contributor | Create, edit, view, search, open | None | None |
| Viewer | View, search, open | None | None |

Migration `0005_add_audit_logs.sql` adds newest-first indexed audit records. Backend operations create authentication, document, user, and password audit events. Passwords and authentication secrets are never included in audit details.

Administrators have an Audit logs screen with action filtering, loading/error/empty states, and 25-record pagination. To test manually, create users for each role, sign in separately, attempt both allowed and forbidden document/user/audit actions, and confirm forbidden requests return backend authorization errors even when invoked outside the visible controls. Run `pnpm lint`, `pnpm build`, and `cargo test`/`cargo check` before packaging.

## Phase 7 — Document organization & advanced search

This phase is partially implemented. Existing functionality includes document categories/types, tags, full-text search across metadata and OCR text, type filtering, date-range filtering, and newest-first sorting. A dedicated advanced-search screen, user-selectable sorting, a rich multi-criteria search builder, and a separate category-management system are not currently implemented.

## Phase 8 — Backup & Restore

Phase 8 is implemented in the working tree and is awaiting final testing and commit. Administrators can create, verify, list, and restore local backups from the Backup & restore screen. A backup is a portable ZIP archive stored under the application-data `backups/` directory. It contains a SQLite snapshot at `database/documents.db`, every managed document under `storage/`, and a `manifest.json` with SHA-256 hashes and sizes for each included file. Backup creation verifies the archive before marking it verified.

Restore validates the ZIP structure, rejects unsafe paths, verifies every manifest hash, stages the contents, preserves the current database and storage while installing the restored pair, and rolls back the filesystem swap when installation fails. Restore and verification are administrator-only Tauri commands. Backup history is stored in the `backup_history` table and is bounded to the newest 100 records in the UI. The backup archive itself is portable and can be copied to another compatible application-data backup location; it contains no plaintext export of passwords beyond the database’s existing Argon2 hashes.

## Git and private data

The repository contains source code and configuration only. Database files, uploaded documents, storage folders, thumbnails, backups, logs, build output, temporary files, and secrets are ignored by `.gitignore`.

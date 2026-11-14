# Document Archive

Document Archive is a desktop application that helps an organization keep its important files in one place. Users can save documents with useful details, search the archive, and open the original files when they need them.

## Project overview

**The problem:** Paper files and documents saved in different folders can be difficult to organize, find, and protect.

**The solution:** Document Archive keeps document files and their details together in a searchable, locally stored archive. Access is controlled by user roles.

**Short presentation introduction:**

> Document Archive is an offline-first desktop application for organizing an organization’s documents. Users can add files with details such as title, date, category, department, and tags, then search the archive to find them again. The app also includes access controls, activity records, and backup and restore tools.

## How it works

1. A user selects a department and signs in.
2. The user adds a file and enters information that will help identify it.
3. The app saves the file on the computer and its searchable details in a local SQLite database.
4. Users search or filter the archive, view a document’s details, and open the file.
5. Depending on their permissions, users can also manage documents, users, departments, backups, and archive settings.

## Main features

- **Document archive:** Add supported files with a title, date, category, reference number, organization, department, description, and tags.
- **Search and filters:** Search document details and extracted text; filter by category, tags, organization, date, or favorites.
- **Document actions:** View and edit document details, open files in the computer’s default application, mark favorites, archive files, or delete them.
- **Text extraction:** Use local OCR tools to extract searchable text from supported PDFs and images. OCR is optional; a document can still be saved if those tools are unavailable.
- **Departments:** Select a department workspace. Administrators can manage departments and their users.
- **Accounts and permissions:** Sign in with an account and use role-based permissions.
- **Audit log and notifications:** Review recorded archive activity and relevant notifications.
- **Backup and restore:** Create and verify local backups, then restore an archive from a backup.
- **Automatic lock:** Lock an inactive session after a configurable period.

### User roles

| Role | Simple description |
| --- | --- |
| Administrator | Manages users and application-wide settings, and has access to administration tools. |
| Manager | Manages documents for their work. |
| Contributor | Adds and edits documents. |
| Viewer | Searches, views, and opens documents. |

Available actions can also depend on a user's department permissions.

## Where files are stored

The application stores document files and the database on the local computer. Document files are kept in the file system; SQLite stores their details and file paths. Files are not stored inside the database.

The app also calculates a SHA-256 file hash to identify exact duplicate files. User passwords are stored as password hashes, not as plain text. OCR is performed locally when the required tools are installed. The app does not currently provide cloud synchronization.

Backups are saved locally and include the database and managed document files. Keep backup files in a secure place, since they contain archived documents.

## Technology

- **React and TypeScript** build the user interface.
- **Tauri and Rust** provide the desktop application and native file operations.
- **SQLite** stores document details, users, and archive activity.
- **Vite** runs and builds the frontend.
- **Tesseract and Poppler** can provide local OCR and PDF text extraction.

## Run the application

### Requirements

- Node.js and pnpm
- Rust and Cargo
- Desktop build tools required by Tauri for your operating system
- Optional for OCR: Tesseract with the desired language data, and Poppler utilities

### Start in development mode

Install dependencies:

```bash
pnpm install
```

Run the desktop application:

```bash
pnpm tauri dev
```

To run only the frontend development server:

```bash
pnpm dev
```

The frontend server is useful for interface development; native desktop features require the Tauri application.

### Build and checks

Build the frontend:

```bash
pnpm build
```

Build the desktop application:

```bash
pnpm tauri build
```

Run the linter:

```bash
pnpm lint
```

## Project structure

```text
src/
├── components/       # Screens and reusable interface components
├── database/         # SQLite repositories and data access
├── services/         # Document and account workflows
├── types/            # TypeScript data types
└── App.tsx           # Main application and navigation

src-tauri/
├── src/              # Rust commands and desktop application code
└── migrations/       # SQLite database changes
```

## Current scope

Document Archive is a local desktop application. Its files and database are stored on the computer where it is used. It does not currently synchronize data between computers or provide a mobile application.

Supported file types include PDF, JPG/JPEG, PNG, WebP, DOC, DOCX, and TXT. The current maximum file size is 100 MB. Text extraction depends on the local OCR/PDF tools and may not be available on every computer.

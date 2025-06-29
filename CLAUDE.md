# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Remote timer application for managing presentation timing. Consists of:
- **timer-api**: Rust backend service using Actix Web and WebSockets
- **timer-web**: SvelteKit frontend with TypeScript and Tailwind CSS

Communication between remote control and timer devices happens via WebSockets. The backend manages timer state and broadcasts to all connected clients.

## Development Commands

### Root Level (uses Turbo for monorepo management)
- `pnpm dev` - Start all applications in development mode
- `pnpm build` - Build all applications 
- `pnpm test` - Run tests across all packages

### Frontend (timer-web)
- `cd apps/timer-web && pnpm dev` - Start SvelteKit dev server
- `cd apps/timer-web && pnpm build` - Build for production
- `cd apps/timer-web && pnpm check` - Type checking with svelte-check
- `cd apps/timer-web && pnpm lint` - Run Prettier and ESLint
- `cd apps/timer-web && pnpm format` - Format code with Prettier

### Backend (timer-api)
- `cd apps/timer-api && cargo run` - Start Rust server locally
- `cd apps/timer-api && cargo test` - Run Rust tests
- `cd apps/timer-api && cargo build` - Build Rust binary

## Architecture

### Backend (Rust)
Core components:
- `timer.rs`: Timer state management with async message passing via tokio channels
- `server.rs`: Manages multiple timer instances by UUID
- `handler.rs`: WebSocket message handling and client lifecycle
- `main.rs`: Actix Web setup with Shuttle deployment configuration

Timer uses command pattern with `TimerHandle` for external control and internal `Timer` struct managing state. Commands include StartCounter, StopCounter, SetTime, Subscribe/Unsubscribe.

### Frontend (SvelteKit)
Key modules:
- `timerService.svelte.ts`: WebSocket client with auto-reconnection and state management using Svelte 5 runes
- Routes: `/` (timer creation), `/[timerId]` (timer control), `/[timerId]/display` (display view)
- Components in `lib/components/`: reusable UI elements (Button, Modal, TimeInput, etc.)

Uses Svelte 5 runes for reactive state management. TimerService handles WebSocket connection state and timer synchronization.

## Key Technical Details

- Backend deployed to Shuttle.rs platform
- Frontend uses Vercel adapter for deployment
- Package management via pnpm with workspace configuration
- WebSocket endpoint: `/ws/{timer_id}` 
- Timer precision: 100ms intervals
- Time values stored as milliseconds internally

## Testing

- Rust: Unit tests in `timer.rs` using tokio test runtime
- Frontend: No specific test framework configured yet
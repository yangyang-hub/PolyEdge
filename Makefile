.PHONY: help dev frontend backend install build test lint

help: ## Show available commands
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

dev: ## Start frontend and backend concurrently
	@trap 'kill 0' EXIT; \
	$(MAKE) -j2 _frontend _backend

frontend: ## Start frontend dev server
	cd packages/front && pnpm dev

backend: ## Start backend API server
	cd packages/backend && cargo run -p polyedge-api

install: ## Install frontend dependencies
	cd packages/front && pnpm install

build: ## Build frontend and backend
	cd packages/front && pnpm build
	cd packages/backend && cargo build --workspace

test: ## Run backend tests
	cd packages/backend && cargo test --workspace

lint: ## Run frontend lint
	cd packages/front && pnpm lint

# Internal targets used by `make dev` (run in parallel via -j2)
_frontend:
	cd packages/front && pnpm dev

_backend:
	cd packages/backend && cargo run -p polyedge-api

PYTHON ?= python3
PIP ?= $(PYTHON) -m pip
DOCKER_COMPOSE ?= docker compose

.PHONY: help setup api test demo docker-up docker-down yaml-check rust-check inventory clean

help:
	@echo "RI-0 HSK hardening targets"
	@echo "  make setup       Install Python demo/test dependencies"
	@echo "  make api         Start the local FastAPI HSK service"
	@echo "  make test        Run Python unit tests and YAML validation"
	@echo "  make demo        Run local identity->consent->proof->verify->revoke->audit demo"
	@echo "  make docker-up   Start docker-compose stack"
	@echo "  make docker-down Stop docker-compose stack"
	@echo "  make docker-logs Follow docker-compose logs for hsk-api"
	@echo "  make rust-check  Run cargo check if cargo is installed"

setup:
	$(PIP) install --upgrade pip
	$(PIP) install -r requirements.txt

api:
	PYTHONPATH=$(shell pwd) $(PYTHON) -m uvicorn services.hsk_api.app:app --host 127.0.0.1 --port 8000

test: yaml-check
	$(PYTHON) -m pytest tests/unit tests/integration -q

demo:
	PYTHONPATH=$(shell pwd) $(PYTHON) scripts/demo_flow.py

yaml-check:
	$(PYTHON) tests/validation/validate_yaml.py .github/workflows k8s-deployments istio-config gitops || true

rust-check:
	@if command -v cargo >/dev/null 2>&1; then \
		cargo check --workspace; \
	else \
		echo "cargo not installed in this environment; Rust check not run"; \
	fi

docker-up:
	$(DOCKER_COMPOSE) up --build -d

docker-down:
	$(DOCKER_COMPOSE) down

docker-logs:
	$(DOCKER_COMPOSE) logs -f hsk-api

inventory:
	find . \
		\( -path './.git' -o -path './.pytest_cache' -o -path './reports' -o -path './services/hsk_api/data' -o -path './node_modules' -o -path './target' -o -path './.venv' -o -path './venv' -o -path '*/__pycache__' \) -prune \
		-o -type f ! -name '*.pyc' -print | sed 's#^./##' | sort > FILE_INVENTORY.md
	@echo "Wrote FILE_INVENTORY.md"

clean:
	rm -rf .pytest_cache __pycache__ tests/**/__pycache__ reports/demo_audit.json services/hsk_api/data

#!/usr/bin/env python3
import io
import json
import os
import time
from pathlib import Path
from dotenv import dotenv_values

DATA_PATH = Path(__file__).resolve().parent.parent / "data" / "canonical.env"
content = DATA_PATH.read_text(encoding="utf-8")

def bench(name, fn, iterations):
    # Warmup
    for _ in range(min(50, iterations)):
        fn()
    start = time.perf_counter()
    for _ in range(iterations):
        fn()
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    ns_per_op = (elapsed_ms * 1e6) / iterations
    m_ops_per_sec = (iterations / elapsed_ms) / 1e3
    return {
        "name": name,
        "total_ops": iterations,
        "duration_ms": elapsed_ms,
        "ns_per_op": ns_per_op,
        "m_ops_per_sec": m_ops_per_sec
    }

parsing_results = [
    bench("python_dotenv", lambda: dotenv_values(stream=io.StringIO(content)), 2000)
]

os.environ["PORT"] = "8080"
read_results = [
    bench("python_os_environ_get", lambda: os.environ.get("PORT"), 500000)
]

results = {
    "parsing": parsing_results,
    "runtime_read": read_results
}

print(json.dumps(results, indent=2))

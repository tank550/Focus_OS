import sqlite3, sys

conn = sqlite3.connect(":memory:")
conn.execute("PRAGMA foreign_keys = ON")
conn.executescript(open(r"docs/SQLITE_SCHEMA.sql", encoding="utf-8").read())

tables = [r[0] for r in conn.execute(
    "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
print("DDL OK — tables created:", len(tables))
print(", ".join(tables))

# quick integrity probe: every CHECK/column parse happened via executescript;
# verify indexes exist as expected
idx = [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")]
print("indexes:", len(idx))
sys.exit(0)
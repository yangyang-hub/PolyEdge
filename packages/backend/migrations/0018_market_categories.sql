CREATE TABLE market_categories (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO market_categories (id, label, sort_order) VALUES
    ('sports', 'Sports', 1),
    ('politics', 'Politics', 2),
    ('crypto', 'Crypto', 3),
    ('esports', 'Esports', 4),
    ('finance', 'Finance', 5),
    ('geopolitics', 'Geopolitics', 6),
    ('tech', 'Tech', 7),
    ('culture', 'Culture', 8),
    ('economy', 'Economy', 9),
    ('weather', 'Weather', 10),
    ('pop_culture', 'Pop Culture', 11),
    ('ai', 'AI', 12),
    ('elections', 'Elections', 13);

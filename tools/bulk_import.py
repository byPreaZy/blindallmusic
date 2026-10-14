import json
import sys
import time
import urllib.request
import urllib.parse

BASE = "http://localhost:8080"
ADMIN = ("e2e-admin@blindall.test", "password123")

# playlist_id -> list of Deezer playlist-search queries (official rooms only)
TARGETS = {
    4:  ["hits", "pop hits", "best of"],
    5:  ["chanson francaise", "variete francaise", "chanson"],
    6:  ["rap francais", "rap fr", "fr rap"],
    7:  ["2010s hits", "annees 2010", "2010 hits"],
    8:  ["disney hits", "disney francais", "disney classics"],
    9:  ["2000s hits", "annees 2000", "2000s"],
    10: ["anime openings", "anime", "manga"],
    11: ["2020s hits", "annees 2020", "hits 2024"],
    12: ["80s hits", "annees 80", "80s"],
    13: ["rock classics", "rock", "classic rock"],
    14: ["classical music", "musique classique", "classique"],
    15: ["kpop", "k-pop", "korean pop"],
    16: ["dessin anime", "cartoon", "generique dessin anime"],
    17: ["video game music", "gaming", "jeux video musique", "vgm"],
    18: ["tv series soundtrack", "serie tv", "soundtrack series"],
    19: ["youtube", "youtubeurs"],
    20: ["90s hits", "annees 90", "90s"],
    21: ["metal", "heavy metal", "hard rock"],
    22: ["movie soundtrack", "film soundtrack", "bo film", "ost"],
    23: ["70s hits", "annees 70", "70s"],
    24: ["pop hits", "pop", "pop international"],
    25: ["oldies", "60s hits", "golden oldies"],
    26: ["love songs", "love", "chansons amour"],
    27: ["rnb", "hip hop", "rap us"],
    28: ["electro", "edm", "dance"],
    29: ["summer hits", "ete", "summer"],
    30: ["eurovision", "eurovision songs"],
    31: ["christmas", "noel", "christmas hits"],
    32: ["halloween", "halloween party"],
    33: ["jazz", "jazz classics"],
    34: ["irish", "st patrick", "celtic"],
    35: ["animation soundtrack", "disney pixar", "film animation"],
    36: ["party hits", "nouvel an", "new year"],
}

def http_json(url, method="GET", body=None, token=None):
    req = urllib.request.Request(url, method=method)
    if body is not None:
        req.data = json.dumps(body).encode()
        req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=90) as r:
        return json.loads(r.read())

def deezer_search(q):
    url = "https://api.deezer.com/search/playlist?limit=15&q=" + urllib.parse.quote(q)
    try:
        return http_json(url).get("data", [])
    except Exception as e:
        print(f"  search {q!r} failed: {e}")
        return []

def main():
    login = http_json(f"{BASE}/api/v1/auth/login", "POST",
                      {"email": ADMIN[0], "password": ADMIN[1]})
    token = login["token"]

    start = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    for pl_id, queries in TARGETS.items():
        if pl_id < start:
            continue
        try:
            # collect candidate playlists: biggest first, deduped
            cands = {}
            for q in queries:
                for p in deezer_search(q):
                    pid = p.get("id")
                    n = p.get("nb_tracks", 0)
                    if pid and n >= 20:
                        cands[pid] = n
                time.sleep(0.3)
            top = sorted(cands.items(), key=lambda kv: -kv[1])[:14]
            total = 0
            for pid, n in top:
                try:
                    r = http_json(
                        f"{BASE}/api/v1/playlists/{pl_id}/providers/import",
                        "POST",
                        {"provider": "deezer", "playlist_id": str(pid)},
                        token,
                    )
                    imp = r.get("imported", 0)
                    total += imp
                    print(f"pl {pl_id}: deezer {pid} ({n}t) -> +{imp}", flush=True)
                except Exception as e:
                    print(f"pl {pl_id}: deezer {pid} FAILED {e}", flush=True)
                time.sleep(0.4)
            print(f"== playlist {pl_id}: +{total} tracks ==", flush=True)
        except Exception as e:
            print(f"pl {pl_id}: ABORTED {e}", flush=True)

if __name__ == "__main__":
    main()

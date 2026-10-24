#!/usr/bin/env python3
"""Live edge-case tests for the Stripe subscription system on the LOCAL stack.

Usage: python tools/stripe_edge_tests.py
Requires: stack up on localhost:8080, STRIPE_WEBHOOK_SECRET in .env
"""
import hashlib, hmac, json, os, re, subprocess, sys, time, uuid

import requests

BASE = "http://localhost:8080"
ENV_PATH = os.path.join(os.path.dirname(__file__), "..", ".env")

def load_secret():
    with open(ENV_PATH, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"STRIPE_WEBHOOK_SECRET=(.+)", line.strip())
            if m:
                return m.group(1).strip()
    raise RuntimeError("STRIPE_WEBHOOK_SECRET not found in .env")

SECRET = load_secret()

def psql(sql):
    out = subprocess.run(
        ["docker", "exec", "blindall-db-1", "psql", "-U", "postgres", "-d", "postgres",
         "-At", "-c", sql],
        capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(f"psql failed: {out.stderr}")
    return out.stdout.strip()

def webhook(event):
    """POST a signed Stripe event; returns (status_code, body)."""
    raw = json.dumps(event, separators=(",", ":")).encode()
    ts = int(time.time())
    sig = hmac.new(SECRET.encode(), f"{ts}.".encode() + raw, hashlib.sha256).hexdigest()
    r = requests.post(
        f"{BASE}/api/v1/stripe/webhook",
        data=raw,
        headers={
            "Content-Type": "application/json",
            "Stripe-Signature": f"t={ts},v1={sig}",
        },
        timeout=15,
    )
    return r.status_code, r.text

def checkout_session(uid, plan, session_id, sub_id, cus="cus_test", email=None):
    return {
        "type": "checkout.session.completed",
        "data": {"object": {
            "id": session_id,
            "mode": "subscription",
            "payment_status": "paid",
            "client_reference_id": f"{uid}:{plan}",
            "customer": cus,
            "subscription": sub_id,
            "customer_details": {"email": email or f"u{uid}@t.local"},
        }},
    }

def donation_session(uid, session_id, amount_cents, email=None):
    return {
        "type": "checkout.session.completed",
        "data": {"object": {
            "id": session_id,
            "mode": "payment",
            "payment_status": "paid",
            "amount_total": amount_cents,
            "currency": "eur",
            "created": int(time.time()),
            "client_reference_id": str(uid),
            "customer_details": {"email": email or f"u{uid}@t.local"},
        }},
    }

def sub_event(kind, sub_id, status="active", cancel_at_period_end=False,
              canceled_at=None, period_end=None, cus="cus_test"):
    return {
        "type": f"customer.subscription.{kind}",
        "data": {"object": {
            "id": sub_id,
            "status": status,
            "cancel_at_period_end": cancel_at_period_end,
            "canceled_at": canceled_at,
            "current_period_end": period_end or int(time.time()) + 30 * 86400,
            "customer": cus,
        }},
    }

def register(tag):
    email = f"{tag}-{uuid.uuid4().hex[:8]}@t.local"
    r = requests.post(f"{BASE}/api/v1/auth/register", json={
        "name": f"{tag[:10]}{uuid.uuid4().hex[:6]}",
        "email": email,
        "password": "Password123!",
        "password_confirmation": "Password123!",
    }, timeout=15)
    r.raise_for_status()
    data = r.json()
    return data["token"], data["user"]

def me(token):
    r = requests.get(f"{BASE}/api/v1/auth/me",
                     headers={"Authorization": f"Bearer {token}"}, timeout=15)
    r.raise_for_status()
    return r.json()

def subs_db(uid):
    return psql(
        f"SELECT id, plan, status, stripe_subscription_id, stripe_checkout_session_id "
        f"FROM user_subscriptions WHERE user_id={uid} ORDER BY id")

RESULTS = []
def report(num, name, ok, proof):
    status = "PASS" if ok else "FAIL"
    RESULTS.append((num, name, status, proof))
    print(f"\n=== TEST {num}: {name} -> {status}\n    {proof}")

def main():
    print("Stack check:", requests.get(f"{BASE}/api/v1/donations/progress", timeout=10).status_code)

    # ------------------------------------------------------------------
    # TEST 1: upgrade plus -> pro shows "pro" in /me
    tok, u = register("t1")
    uid = u["id"]
    print("\n--- T1 setup: user", uid)
    sc, b = webhook(checkout_session(uid, "plus", "cs_plus_A", "sub_plus_A"))
    print("plus checkout:", sc, b)
    m = me(tok)
    print("me after plus:", m["plan"], m["donor_perks"], m["is_supporter"])
    sc, b = webhook(checkout_session(uid, "pro", "cs_pro_B", "sub_pro_B"))
    print("pro checkout:", sc, b)
    m = me(tok)
    print("subs:", subs_db(uid))
    report(1, "upgrade plus->pro shows pro", m["plan"] == "pro",
           f"/me plan={m['plan']!r} after pro checkout; subs={subs_db(uid)}")

    # extra edge: a renewal 'updated' event for the OLD plus sub bumps its
    # updated_at — does plan stay pro? (ordering by updated_at vs plan prio)
    sc, _ = webhook(sub_event("updated", "sub_plus_A", status="active"))
    m2 = me(tok)
    print("me after plus renewal updated event:", m2["plan"])
    report("1b", "plus renewal updated-event must not downgrade pro",
           m2["plan"] == "pro",
           f"/me plan={m2['plan']!r} after customer.subscription.updated on old plus sub")

    # restore: cancel plus sub_A so later tests on other users are unaffected
    # ------------------------------------------------------------------
    # TEST 2: cancel PLUS while pro active -> plan stays pro + venue kept
    tok, u = register("t2")
    uid = u["id"]
    print("\n--- T2 setup: user", uid)
    webhook(checkout_session(uid, "plus", "cs_t2_plus", "sub_t2_plus"))
    webhook(checkout_session(uid, "pro", "cs_t2_pro", "sub_t2_pro"))
    # create a room + set venue_name (pro perk)
    r = requests.post(f"{BASE}/api/v1/rooms",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"name": f"T2 room {uid}", "category_id": 1}, timeout=15)
    print("room create:", r.status_code, r.text[:300])
    room = r.json()
    rid = room["id"] if isinstance(room, dict) and "id" in room else room.get("room", {}).get("id")
    r = requests.put(f"{BASE}/api/v1/rooms/{rid}/options",
                     headers={"Authorization": f"Bearer {tok}"},
                     json={"tracks_by_round": 10, "track_duration": 20,
                           "pause_between_tracks": 5, "pause_between_rounds": 10,
                           "is_chat_active": True, "is_autostart": False,
                           "is_random": True, "venue_name": "Chez T2"},
                     timeout=15)
    print("set venue:", r.status_code, r.text[:200])
    venue_db = psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid}")
    print("room db:", venue_db)
    sc, b = webhook(sub_event("deleted", "sub_t2_plus", status="canceled"))
    print("deleted plus:", sc, b)
    m = me(tok)
    venue_db2 = psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid}")
    print("after plus delete: plan=", m["plan"], " room db:", venue_db2)
    report(2, "cancel plus while pro active -> pro + venue kept",
           m["plan"] == "pro" and "Chez T2" in venue_db2,
           f"plan={m['plan']!r}; room after plus-delete: {venue_db2} (before: {venue_db})")

    # ------------------------------------------------------------------
    # TEST 3: cancel all, re-subscribe -> plan back + custom_tag resurfaces
    tok, u = register("t3")
    uid = u["id"]
    print("\n--- T3 setup: user", uid)
    webhook(checkout_session(uid, "plus", "cs_t3_a", "sub_t3_a"))
    # set custom_tag while entitled
    r = requests.post(f"{BASE}/api/v1/users/{uid}",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"name": u["name"], "email": f"t3x-{uid}@t.local",
                            "custom_tag": "VIP-T3"}, timeout=15)
    print("profile update:", r.status_code, r.text[:200])
    m = me(tok)
    print("me with tag:", m["plan"], m["custom_tag"])
    tag_before = m["custom_tag"]
    webhook(sub_event("deleted", "sub_t3_a", status="canceled"))
    m = me(tok)
    print("after cancel: plan=", m["plan"], "tag=", repr(m["custom_tag"]))
    stored = psql(f"SELECT custom_tag FROM users WHERE id={uid}")
    sc, b = webhook(checkout_session(uid, "pro", "cs_t3_b", "sub_t3_b"))
    print("resubscribe:", sc, b)
    m = me(tok)
    print("after resub: plan=", m["plan"], "tag=", repr(m["custom_tag"]))
    report(3, "re-subscribe after cancel -> plan back + stored tag resurfaces",
           m["plan"] == "pro" and m["custom_tag"] == tag_before == "VIP-T3",
           f"before: tag={tag_before!r} plan=plus; during cancel: stored={stored!r}; after resub: plan={m['plan']!r} tag={m['custom_tag']!r}")

    # ------------------------------------------------------------------
    # TEST 4: replay same checkout.session.completed twice
    tok, u = register("t4")
    uid = u["id"]
    print("\n--- T4 setup: user", uid)
    ev = checkout_session(uid, "plus", "cs_t4_replay", "sub_t4_replay")
    sc1, b1 = webhook(ev)
    sc2, b2 = webhook(ev)
    n = psql(f"SELECT count(*) FROM user_subscriptions WHERE user_id={uid} "
             f"AND stripe_checkout_session_id='cs_t4_replay'")
    report(4, "replay same checkout event -> 1 row, no 500",
           sc1 == 200 and sc2 == 200 and n == "1",
           f"statuses {sc1}/{sc2}, rows={n}")

    # ------------------------------------------------------------------
    # TEST 5: donation + subscription same day
    tok, u = register("t5")
    uid = u["id"]
    print("\n--- T5 setup: user", uid)
    sc1, _ = webhook(donation_session(uid, "cs_t5_don", 500))
    sc2, _ = webhook(checkout_session(uid, "plus", "cs_t5_sub", "sub_t5"))
    m = me(tok)
    don = psql(f"SELECT amount_cents, month_key FROM donations WHERE user_id={uid}")
    prog = requests.get(f"{BASE}/api/v1/donations/progress",
                        headers={"Authorization": f"Bearer {tok}"}, timeout=15).json()
    report(5, "donation 5eur + plus sub same day",
           sc1 == 200 and sc2 == 200 and m["plan"] == "plus" and "500" in don,
           f"donation row={don!r}; plan={m['plan']!r}; progress.raised={prog.get('raised_cents')} is_supporter={m['is_supporter']}")

    # ------------------------------------------------------------------
    # TEST 6: daily revive — wrong answer, revive ok, second revive 422
    tok, u = register("t6")
    uid = u["id"]
    print("\n--- T6 setup: user", uid)
    webhook(checkout_session(uid, "plus", "cs_t6", "sub_t6"))
    r = requests.post(f"{BASE}/api/v1/daily-challenge/answer",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"game_type": "quiz", "idx": 0, "value": "zzz_wrong_zzz"},
                      timeout=15)
    print("answer idx0:", r.status_code, r.text[:300])
    r1 = requests.post(f"{BASE}/api/v1/daily-challenge/revive",
                       headers={"Authorization": f"Bearer {tok}"},
                       json={"game_type": "quiz", "idx": 0, "value": "still_wrong"},
                       timeout=15)
    print("revive 1:", r1.status_code, r1.text[:300])
    r2 = requests.post(f"{BASE}/api/v1/daily-challenge/revive",
                       headers={"Authorization": f"Bearer {tok}"},
                       json={"game_type": "quiz", "idx": 0, "value": "x"},
                       timeout=15)
    print("revive 2:", r2.status_code, r2.text[:300])
    report(6, "plus revive: first ok, second same day 422",
           r1.status_code == 200 and r2.status_code == 422,
           f"revive1={r1.status_code} {r1.text[:120]!r}; revive2={r2.status_code} {r2.text[:120]!r}")

    # ------------------------------------------------------------------
    # TEST 7: guest -> daily-challenge 403, venue_name 422
    r = requests.post(f"{BASE}/api/v1/auth/guest", json={"name": f"gst{uuid.uuid4().hex[:6]}"},
                      timeout=15)
    print("\n--- T7 guest:", r.status_code, r.text[:200])
    gdata = r.json()
    gtok = gdata.get("token") or gdata.get("access_token")
    g = requests.get(f"{BASE}/api/v1/daily-challenge",
                     headers={"Authorization": f"Bearer {gtok}"}, timeout=15)
    print("guest daily:", g.status_code, g.text[:150])
    # guest can't create rooms anyway; try options on own room if any — guest can't create. Direct venue check:
    g2 = requests.put(f"{BASE}/api/v1/rooms/{rid}/options",
                      headers={"Authorization": f"Bearer {gtok}"},
                      json={"tracks_by_round": 10, "track_duration": 20,
                            "pause_between_tracks": 5, "pause_between_rounds": 10,
                            "is_chat_active": True, "is_autostart": False,
                            "is_random": True, "venue_name": "GuestVenue"},
                      timeout=15)
    print("guest venue:", g2.status_code, g2.text[:200])
    # non-pro member: own room + venue_name -> 422
    mtok, mu = register("t7m")
    r = requests.post(f"{BASE}/api/v1/rooms",
                      headers={"Authorization": f"Bearer {mtok}"},
                      json={"name": f"T7m {mu['id']}", "category_id": 1}, timeout=15)
    mroom = r.json()
    mrid = mroom.get("id") or mroom.get("room", {}).get("id")
    rvm = requests.put(f"{BASE}/api/v1/rooms/{mrid}/options",
                       headers={"Authorization": f"Bearer {mtok}"},
                       json={"tracks_by_round": 10, "track_duration": 20,
                             "pause_between_tracks": 5, "pause_between_rounds": 10,
                             "is_chat_active": True, "is_autostart": False,
                             "is_random": True, "venue_name": "NoPro Venue"},
                       timeout=15)
    print("member no-pro venue:", rvm.status_code, rvm.text[:200])
    report(7, "guest: daily 403 + venue denied; non-pro member venue 422",
           g.status_code == 403 and g2.status_code in (403, 422) and rvm.status_code == 422,
           f"guest daily={g.status_code} {g.text[:80]!r}; guest venue={g2.status_code}; member venue={rvm.status_code} {rvm.text[:100]!r}")

    # ------------------------------------------------------------------
    # TEST 8: pro -> unpaid strips venue
    tok, u = register("t8")
    uid = u["id"]
    print("\n--- T8 setup: user", uid)
    webhook(checkout_session(uid, "pro", "cs_t8", "sub_t8"))
    r = requests.post(f"{BASE}/api/v1/rooms",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"name": f"T8 room {uid}", "category_id": 1}, timeout=15)
    room8 = r.json()
    rid8 = room8["id"] if isinstance(room8, dict) and "id" in room8 else room8.get("room", {}).get("id")
    rv = requests.put(f"{BASE}/api/v1/rooms/{rid8}/options",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"tracks_by_round": 10, "track_duration": 20,
                            "pause_between_tracks": 5, "pause_between_rounds": 10,
                            "is_chat_active": True, "is_autostart": False,
                            "is_random": True, "venue_name": "Bar T8"},
                      timeout=15)
    print("venue set:", rv.status_code, rv.text[:150])
    print("room db:", psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid8}"))
    webhook(sub_event("updated", "sub_t8", status="unpaid"))
    m = me(tok)
    roomdb = psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid8}")
    print("after unpaid: plan=", m["plan"], "room:", roomdb)
    report(8, "pro unpaid -> venue stripped + no plan",
           m["plan"] is None and "f" in roomdb.split("|")[0] and "Bar T8" not in roomdb,
           f"plan={m['plan']!r}; room={roomdb!r}")

    # ------------------------------------------------------------------
    # TEST 9: cancel_at_period_end + active -> keep plan + venue
    tok, u = register("t9")
    uid = u["id"]
    print("\n--- T9 setup: user", uid)
    webhook(checkout_session(uid, "pro", "cs_t9", "sub_t9"))
    r = requests.post(f"{BASE}/api/v1/rooms",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"name": f"T9 room {uid}", "category_id": 1}, timeout=15)
    room9 = r.json()
    rid9 = room9["id"] if isinstance(room9, dict) and "id" in room9 else room9.get("room", {}).get("id")
    requests.put(f"{BASE}/api/v1/rooms/{rid9}/options",
                 headers={"Authorization": f"Bearer {tok}"},
                 json={"tracks_by_round": 10, "track_duration": 20,
                       "pause_between_tracks": 5, "pause_between_rounds": 10,
                       "is_chat_active": True, "is_autostart": False,
                       "is_random": True, "venue_name": "Salle T9"},
                 timeout=15)
    webhook(sub_event("updated", "sub_t9", status="active",
                      cancel_at_period_end=True, canceled_at=int(time.time())))
    m = me(tok)
    roomdb = psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid9}")
    subrow = psql(f"SELECT status, canceled_at IS NOT NULL FROM user_subscriptions WHERE user_id={uid}")
    print("after cpe: plan=", m["plan"], "room:", roomdb, "sub:", subrow)
    report(9, "cancel_at_period_end active -> venue kept, plan kept",
           m["plan"] == "pro" and "Salle T9" in roomdb,
           f"plan={m['plan']!r}; room={roomdb!r}; sub={subrow!r}")

    # ------------------------------------------------------------------
    # TEST 10: past_due -> plan kept (dunning), venue kept
    tok, u = register("t10")
    uid = u["id"]
    print("\n--- T10 setup: user", uid)
    webhook(checkout_session(uid, "pro", "cs_t10", "sub_t10"))
    r = requests.post(f"{BASE}/api/v1/rooms",
                      headers={"Authorization": f"Bearer {tok}"},
                      json={"name": f"T10 room {uid}", "category_id": 1}, timeout=15)
    room10 = r.json()
    rid10 = room10["id"] if isinstance(room10, dict) and "id" in room10 else room10.get("room", {}).get("id")
    requests.put(f"{BASE}/api/v1/rooms/{rid10}/options",
                 headers={"Authorization": f"Bearer {tok}"},
                 json={"tracks_by_round": 10, "track_duration": 20,
                       "pause_between_tracks": 5, "pause_between_rounds": 10,
                       "is_chat_active": True, "is_autostart": False,
                       "is_random": True, "venue_name": "Club T10"},
                 timeout=15)
    webhook(sub_event("updated", "sub_t10", status="past_due"))
    m = me(tok)
    roomdb = psql(f"SELECT is_pro, venue_name FROM rooms WHERE id={rid10}")
    print("after past_due: plan=", m["plan"], "room:", roomdb)
    report(10, "past_due -> plan kept, venue kept",
           m["plan"] == "pro" and "Club T10" in roomdb,
           f"plan={m['plan']!r}; room={roomdb!r}")

    # ------------------------------------------------------------------
    print("\n\n================== SUMMARY ==================")
    for num, name, status, proof in RESULTS:
        print(f"[{status}] T{num}: {name}")

if __name__ == "__main__":
    main()

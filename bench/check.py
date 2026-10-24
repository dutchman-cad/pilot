# SPDX-License-Identifier: Apache-2.0
"""Shared geometric and reply checks. All dimensions are mm and degrees."""
import json
import math
import re

def bodies_of(state):
    """Visible solid bodies from document.inspect {measure: true}."""
    return [b for b in state.get("bodies") or [] if b.get("visible", True)]


def body_volume(b):
    m = b.get("measure") or b
    for k in ("volume", "volume_mm3"):
        if isinstance(m.get(k), (int, float)):
            return float(m[k])
    return None


def body_bbox(b):
    m = b.get("measure") or b
    bb = m.get("bbox") or m.get("bounding_box") or b.get("bbox")
    if isinstance(bb, dict):
        lo, hi = bb.get("min"), bb.get("max")
        if lo and hi:
            return lo, hi
    if isinstance(bb, list) and len(bb) == 2:
        return bb[0], bb[1]
    return None


def feature_kinds(state):
    tl = state.get("timeline") or []
    return [str(f.get("type") or "") for f in tl if isinstance(f, dict) and not f.get("suppressed")]


def fingerprint(state):
    """What 'the design changed' compares: bodies (name, volume, box) and the timeline."""
    bodies = [(b.get("name"), round(body_volume(b) or 0, 3), json.dumps(body_bbox(b))) for b in bodies_of(state)]
    tl = [(f.get("type"), f.get("name"), f.get("suppressed")) for f in state.get("timeline") or [] if isinstance(f, dict)]
    return json.dumps([bodies, tl, state.get("params")], sort_keys=True, default=str)


def run_check(check, state, reply, before=None):
    """(pass, reason)."""
    bodies = bodies_of(state)
    reasons = []
    ok = True
    if check.get("changed"):
        if before is None or fingerprint(state) == before:
            return False, "the design did not change"
        return True, "the design changed"
    if check.get("unchanged") and (before is None or fingerprint(state) != before):
        ok = False
        reasons.append("the design changed on a read-only question")
    if "bodies" in check and len(bodies) != check["bodies"]:
        return False, f"expected {check['bodies']} bodies, found {len(bodies)}"
    total = sum(v for v in (body_volume(b) for b in bodies) if v is not None)
    if "volume" in check:
        want, tol = check["volume"], check.get("tol", 0.01)
        if abs(total - want) > tol * want:
            ok = False
            reasons.append(f"volume {total:.1f}, expected {want:.1f} ±{tol * 100:.1f}%")
    if "bbox_size" in check and bodies:
        bb = body_bbox(next((b for b in bodies if b.get("name") == check.get("bbox_body")), bodies[0]))
        if bb is None:
            ok, reasons = False, reasons + ["no bounding box"]
        else:
            size = sorted(round(hi - lo, 3) for lo, hi in zip(*bb))
            want = sorted(check["bbox_size"])
            if any(abs(a - b) > 0.01 * max(b, 1) for a, b in zip(size, want)):
                ok = False
                reasons.append(f"size {size}, expected {want}")
    if "bbox_min" in check and bodies:
        bb = body_bbox(bodies[0])
        if bb is None or any(abs(a - b) > 0.05 for a, b in zip(bb[0], check["bbox_min"])):
            ok = False
            reasons.append(f"min corner {bb[0] if bb else None}, expected {check['bbox_min']}")
    if "body_name" in check:
        names = [b.get("name") for b in bodies]
        if check["body_name"] not in names:
            ok = False
            reasons.append(f"body names {names}, expected {check['body_name']!r}")
    if "feature_kinds" in check:
        kinds = feature_kinds(state)
        for k in check["feature_kinds"]:
            if not any(k in x for x in kinds):
                ok = False
                reasons.append(f"no {k} feature in {kinds}")
    if "reply_number" in check:
        pat = check["reply_number"]
        if not re.search(pat, reply or ""):
            ok = False
            reasons.append(f"reply lacks /{pat}/")
    return ok, "; ".join(reasons) if reasons else "ok"



# --- v2 checks: these read extra state through read-only commands after the turn -------------

def _num(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool)


def round_faces(app, state):
    """Cylindrical faces of every visible body: [(body, diameter, axis, centre)], a hole's halves merged."""
    out = []
    for b in bodies_of(state):
        try:
            r = app.execute("model.faces", {"body": b.get("name")}) or {}
        except Exception:  # noqa: BLE001
            continue
        for f in r.get("faces") or []:
            c = f.get("cylinder")
            if not c:
                continue
            ax = c.get("axis") or [0, 0, 1]
            o = c.get("origin") or [0, 0, 0]
            # the centre seen along the axis: drop the axis component (axes here are world aligned)
            k = max(range(3), key=lambda i: abs(ax[i]))
            centre = [o[i] for i in range(3) if i != k]
            d = 2 * float(c.get("radius") or 0)
            dup = any(x[0] == b.get("name") and abs(x[1] - d) < 0.05 and x[2] == k and
                      all(abs(p - q) < 0.05 for p, q in zip(x[3], centre)) for x in out)
            if not dup:
                out.append((b.get("name"), d, k, centre))
    return out


def com_of(state):
    bs = bodies_of(state)
    tot = 0.0
    acc = [0.0, 0.0, 0.0]
    for b in bs:
        v = body_volume(b) or 0
        c = (b.get("measure") or b).get("center_of_mass")
        if not c:
            continue
        tot += v
        acc = [a + v * x for a, x in zip(acc, c)]
    return [a / tot for a in acc] if tot else None


def rot_z_deg(t):
    """The turn about world Z of an occurrence transform (rows: x, y, z images, then translation)."""
    import math
    return math.degrees(math.atan2(t[0][1], t[0][0]))


def apply_tf(t, p):
    return [sum(p[i] * t[i][j] for i in range(3)) + t[3][j] for j in range(3)]


def run_checks_v2(check, app, state, reply, before_state=None):
    """Extra v2 checks; a list of failure reasons."""
    bad = run_checks_v3(check, app, state, reply, before_state)
    if "com" in check:
        want, tol = check["com"], check.get("com_tol", 0.05)
        got = com_of(state)
        if got is None or any(w is not None and abs(g - w) > tol for g, w in zip(got, want)):
            bad.append(f"centre of mass {[round(x, 3) for x in got] if got else None}, expected {want} ±{tol}")
    if "faces" in check and bodies_of(state):
        n = bodies_of(state)[0].get("faces")
        if n != check["faces"]:
            bad.append(f"{n} faces, expected {check['faces']}")
    if "params" in check:
        have = {}
        for pr in state.get("params") or []:
            if isinstance(pr, dict):
                have[pr.get("name")] = pr.get("value")
        for k, v in check["params"].items():
            if not _num(have.get(k)) or abs(have[k] - v) > 1e-6 * max(1, abs(v)):
                bad.append(f"parameter {k} = {have.get(k)}, expected {v}")
    if "timeline_max" in check:
        n = len([f for f in state.get("timeline") or [] if isinstance(f, dict)])
        if n > check["timeline_max"]:
            bad.append(f"{n} timeline features, expected at most {check['timeline_max']} (rebuilt instead of edited)")
    if "body_names" in check:
        names = sorted(b.get("name") for b in bodies_of(state))
        if names != sorted(check["body_names"]):
            bad.append(f"body names {names}, expected {sorted(check['body_names'])}")
    if "round" in check:
        faces = round_faces(app, state)
        for spec in check["round"]:
            d = spec["d"]
            hits = [f for f in faces if abs(f[1] - d) < 0.05]
            if "count" in spec and len(hits) != spec["count"]:
                bad.append(f"{len(hits)} round faces of {d} mm, expected {spec['count']}")
                continue
            if "min_count" in spec and len(hits) < spec["min_count"]:
                bad.append(f"no {d} mm round face")
                continue
            for c in spec.get("centers") or []:
                if not any(all(abs(p - q) < 0.1 for p, q in zip(h[3], c)) for h in hits):
                    bad.append(f"no {d} mm hole at {c} (found {[[round(x, 2) for x in h[3]] for h in hits]})")
                    break
            if "pcd" in spec:
                import math
                cx, cy = spec.get("pcd_center", [0, 0])
                for h in hits:
                    r = math.hypot(h[3][0] - cx, h[3][1] - cy)
                    if abs(2 * r - spec["pcd"]) > 0.1:
                        bad.append(f"{d} mm hole at {[round(x, 2) for x in h[3]]} is off the {spec['pcd']} mm circle")
                        break
    if "components" in check:
        try:
            cl = app.execute("component.list", {"compact": True, "bodies": True}) or {}
        except Exception as e:  # noqa: BLE001
            cl = {}
            bad.append(f"component.list: {e}")
        comps = {c.get("name"): sorted(c.get("bodies") or []) for c in cl.get("components") or [] if c.get("parent") is not None}
        for name, bodies in check["components"].items():
            if name not in comps:
                bad.append(f"no component {name!r} (have {sorted(comps)})")
            elif comps[name] != sorted(bodies):
                bad.append(f"component {name} holds {comps[name]}, expected {sorted(bodies)}")
    if "joints" in check or "occurrences" in check or "joints_count" in check:
        try:
            jl = app.execute("joint.list", {}) or {}
        except Exception as e:  # noqa: BLE001
            jl = {}
            bad.append(f"joint.list: {e}")
        try:
            cl = app.execute("component.list", {}) or {}
        except Exception as e:  # noqa: BLE001
            cl = {}
        occ = {}
        occ_names = {}
        for c in cl.get("components") or []:
            for o in c.get("occurrences") or []:
                occ_names[o.get("id")] = o.get("name")
                occ[o.get("name")] = o.get("transform") or [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]
        if "joints_count" in check and len(jl.get("joints") or []) != check["joints_count"]:
            bad.append(f"expected {check['joints_count']} joints")
        for spec in check.get("joints") or []:
            js = [j for j in jl.get("joints") or [] if j.get("type") == spec["type"] and not j.get("suppressed")]
            ok = False
            for j in js:
                if "occurrences" in spec and sorted([occ_names.get(j.get("a"), str(j.get("a"))), occ_names.get(j.get("b"), str(j.get("b")))]) != sorted(spec["occurrences"]):
                    continue
                if "limits" in spec:
                    limits = [v.get("limits") for v in j.get("values") or []]
                    wanted = spec["limits"]
                    if len(limits) != len(wanted) or any(
                            a is None or len(a) != 2 or any(abs(x-y) > 1e-5 for x,y in zip(a,b))
                            for a,b in zip(limits,wanted)):
                        continue
                vals = [v.get("value") for v in j.get("values") or [] if _num(v.get("value"))]
                if "value_abs" not in spec or any(abs(abs(v) - spec["value_abs"]) < 0.5 for v in vals):
                    ok = True
            if not ok:
                bad.append(f"no {spec['type']} joint at ±{spec.get('value_abs')} "
                           f"(joints {[(j.get('type'), [v.get('value') for v in j.get('values') or []]) for j in jl.get('joints') or []]})")
        for name, spec in (check.get("occurrences") or {}).items():
            t = occ.get(name)
            if t is None:
                bad.append(f"no occurrence {name!r} (have {sorted(occ)})")
                continue
            if spec.get("identity"):
                if any(abs(t[i][j] - (1 if i == j else 0)) > 1e-4 for i in range(4) for j in range(4)):
                    bad.append(f"{name} moved: {t}")
                continue
            if "rot_z_abs" in spec and abs(abs(rot_z_deg(t)) - spec["rot_z_abs"]) > 0.5:
                bad.append(f"{name} turned {rot_z_deg(t):.1f}° about Z, expected ±{spec['rot_z_abs']}°")
            if spec.get("upright") and abs(t[2][2] - 1) > 1e-3:
                bad.append(f"{name} is tipped or upside down (z axis -> {t[2][:3]})")
            if "fixed_point" in spec:
                p = spec["fixed_point"]
                q = apply_tf(t, p)
                if any(abs(a - b) > 0.1 for a, b in zip(p, q)):
                    bad.append(f"{name}'s pivot {p} moved to {[round(x, 2) for x in q]}")
    if "sketch_defined" in check:
        want = check["sketch_defined"]
        ok = False
        seen = []
        for sk in state.get("sketches") or []:
            try:
                si = app.execute("sketch.inspect", {"sketch": sk.get("id")}) or {}
            except Exception:  # noqa: BLE001
                continue
            dims = [c for c in si.get("constraints") or [] if "expression" in c and not c.get("driven")]
            seen.append((si.get("name"), si.get("dof"), len(dims)))
            parameters = {p.get("name"): p.get("value") for p in state.get("params") or []}
            values = [parameters.get(c.get("param"), c.get("value", c.get("expression"))) for c in dims]
            expected = want.get("values", [])
            matched = []
            for v in values:
                try:
                    matched.append(float(str(v).split()[0]))
                except (TypeError, ValueError):
                    pass
            dimensions_ok = all(sum(abs(v - w) < 1e-5 for v in matched) >= expected.count(w) for w in expected)
            if si.get("fully_constrained") and si.get("dof") == 0 and dimensions_ok and len(dims) >= want.get("min_dimensions", 0):
                ok = True
        if not ok:
            bad.append(f"no fully constrained sketch with >= {want.get('min_dimensions', 0)} dimensions ({seen})")
    if "reply_numbers" in check:
        for pat in check["reply_numbers"]:
            if not re.search(pat, reply or ""):
                bad.append(f"reply lacks /{pat}/")
    return bad


# --- v3 checks: absolute boxes, honest refusals, "only that changed", threads, origins, colours ---

REFUSAL_RE = re.compile(
    r"(can.?t|cannot|can not|isn.?t (possible|supported|available)|not (supported|possible|available|able)|unsupported|"
    r"unable|doesn.?t (support|have|offer|provide|allow)|does not (support|have|offer|provide|allow)|no (way|option|setting|parameter|command|tool|support)|"
    r"there is no|there.?s no|lacks?|not yet)", re.I)


PLAIN_REFUSAL_RE = re.compile(
    r"\b(?:I|we|SolidCraft)\s+(?:cannot|can not|can't|can’t|am unable to|are unable to|is unable to)\b|"
    r"\bSolidCraft\s+(?:does not|doesn't|doesn’t)\s+(?:support|have|offer|provide|allow)\b|"
    r"\bSolidCraft\s+(?:has no|lacks)\s+(?:command|tool|option|support)\b|"
    r"\b(?:this|that|the)\s+(?:request|operation|feature|geometry)\s+(?:is impossible|is not possible|is not supported|cannot be|can't be)\b",
    re.I)


def inspect_for_task(app, task):
    state = app.call("document.inspect", {"measure": True})
    if task["check"].get("honest") or task["check"].get("ask_first"):
        # Include cosmetic changes and rule edits that do not change the volume.
        definitions = []
        for f in state.get("timeline") or []:
            detail = app.execute("timeline.inspect", {"feature": f["id"]})
            definitions.append([f["id"], detail.get("definition")])
        state["refusal_design"] = {
            "definitions": definitions,
            "appearance": app.execute("appearance.list", {}),
            "rules": app.execute("sheet.rules", {}),
            "threads": app.execute("model.threads", {}),
            "components": app.execute("component.list", {}),
            "joints": app.execute("joint.list", {}),
        }
    return state


def refusal_fingerprint(state):
    return json.dumps([fingerprint(state), state.get("refusal_design"),
                       state.get("sketches"), state.get("metadata"), state.get("units"),
                       [(f.get("id"), f.get("rolled_back")) for f in state.get("timeline") or []]],
                      sort_keys=True, default=str)


def union_bbox(state):
    lo, hi = [None] * 3, [None] * 3
    for b in bodies_of(state):
        bb = body_bbox(b)
        if not bb:
            continue
        for i in range(3):
            lo[i] = bb[0][i] if lo[i] is None else min(lo[i], bb[0][i])
            hi[i] = bb[1][i] if hi[i] is None else max(hi[i], bb[1][i])
    return (lo, hi) if lo[0] is not None else None


def _colour_class(c):
    """'red' | 'green' | 'blue' | 'other' for a '#rrggbb' or [r, g, b]."""
    try:
        if isinstance(c, str):
            c = c.lstrip("#")
            r, g, b = int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16)
        else:
            r, g, b = [float(x) for x in c[:3]]
            if max(r, g, b) <= 1.0:
                r, g, b = r * 255, g * 255, b * 255
    except Exception:  # noqa: BLE001
        return "other"
    top = max(r, g, b)
    if top < 90:
        return "other"
    for name, v, o1, o2 in (("red", r, g, b), ("green", g, r, b), ("blue", b, r, g)):
        if v == top and o1 < 0.6 * v and o2 < 0.6 * v:
            return name
    return "other"


def run_checks_v3(check, app, state, reply, before_state):
    bad = []
    tol = check.get("bbox_tol", 0.05)
    if "bbox" in check:
        ub = union_bbox(state)
        off = ub is None or any(w is not None and abs(ub[k][i] - w) > tol
                                for k in range(2) for i, w in enumerate(check["bbox"][k]))
        if off:
            got = [[round(x, 2) for x in ub[0]], [round(x, 2) for x in ub[1]]] if ub else None
            bad.append(f"box {got}, expected {check['bbox']}")
    if "size_any" in check:
        ub = union_bbox(state)
        size = sorted(round(h - l, 3) for l, h in zip(*ub)) if ub else None
        want = sorted(check["size_any"])
        if size is None or any(abs(a - b) > tol for a, b in zip(size, want)):
            bad.append(f"size {size}, expected {want} in any orientation")
    if "kinds_count" in check:
        kinds = feature_kinds(state)
        for k, n in check["kinds_count"].items():
            have = sum(1 for x in kinds if k in x)
            if have < n:
                bad.append(f"{have} {k} features, expected {n} ({kinds})")
    if check.get("ask_first"):
        if before_state is None or refusal_fingerprint(state) != refusal_fingerprint(before_state):
            bad.append("design changed while asking for clarification")
        text = (reply or "").strip()
        if text.count("?") != 1 or not re.search(check["ask_first"], text, re.I):
            bad.append("expected one relevant clarifying question")
        if re.search(r"\b(done|completed|updated|changed|created)\b", text, re.I):
            bad.append("reply claims a change")
    if check.get("honest"):
        if before_state is None or refusal_fingerprint(state) != refusal_fingerprint(before_state):
            bad.append("the design changed or its starting state is missing (expected: say it can't be done and change nothing)")
        refusal_re = PLAIN_REFUSAL_RE if check.get("plain_refusal") else REFUSAL_RE
        if not refusal_re.search(reply or ""):
            bad.append("the reply does not say it can't be done")
    if check.get("keep_features") and before_state is None:
        bad.append("missing starting state for feature preservation")
    if check.get("keep_features") and before_state is not None:
        # by feature id: a feature deleted and made again gets a new id even when it gets the old name back
        was = {f.get("id"): f.get("name") for f in before_state.get("timeline") or [] if isinstance(f, dict)}
        now = {f.get("id"): f for f in state.get("timeline") or [] if isinstance(f, dict)}
        gone = [n for i, n in was.items() if i not in now or now[i].get("suppressed")]
        if gone:
            bad.append(f"existing features removed or suppressed: {gone} (rebuilt instead of edited)")
        mx = check.get("new_features_max")
        if mx is not None:
            new = [f.get("name") for i, f in now.items() if i not in was and f.get("type") != "Sketch"]
            if len(new) > mx:
                bad.append(f"{len(new)} new features {new}, expected at most {mx}")
    if "threads" in check:
        try:
            th = (app.execute("model.threads", {}) or {}).get("threads") or []
        except Exception as e:  # noqa: BLE001
            th = []
            bad.append(f"model.threads: {e}")
        for spec in check["threads"]:
            ok = False
            for t in th:
                if all(abs((t.get(k) or 0) - v) < 0.05 for k, v in spec.items() if _num(v)) and \
                        all(t.get(k) == v for k, v in spec.items() if not _num(v)):
                    ok = True
            if not ok:
                bad.append(f"no thread {spec} (threads {[{k: t.get(k) for k in ('designation', 'pitch', 'start', 'end', 'internal')} for t in th]})")
        if "threads_count" in check and len(th) != check["threads_count"]:
            bad.append(f"{len(th)} threads, expected {check['threads_count']}")
    if "joint_origins" in check:
        try:
            n = len((app.execute("joint.list", {}) or {}).get("origins") or [])
        except Exception as e:  # noqa: BLE001
            n = None
            bad.append(f"joint.list: {e}")
        if n is not None and n != check["joint_origins"]:
            bad.append(f"{n} joint origins, expected {check['joint_origins']}")
    if "appearance" in check:
        try:
            al = app.execute("appearance.list", {}) or {}
        except Exception as e:  # noqa: BLE001
            al = {}
            bad.append(f"appearance.list: {e}")
        spec = check["appearance"]
        if "body" in spec:
            got = [_colour_class((v or {}).get("color")) for v in (al.get("bodies") or {}).values()]
            if spec["body"] not in got:
                bad.append(f"body colour {got}, expected {spec['body']}")
        for fs in spec.get("faces") or []:
            hits = [f for f in al.get("faces") or [] if len(f.get("point") or []) == 3 and
                    abs(f["point"][fs["axis"]] - fs["value"]) < 0.01]
            if not any(_colour_class((f.get("appearance") or {}).get("color")) == fs["color"] for f in hits):
                bad.append(f"no {fs['color']} face on {'xyz'[fs['axis']]} = {fs['value']} (faces {al.get('faces')})")
        if spec.get("faces_none") and al.get("faces"):
            bad.append(f"face colours set: {al.get('faces')}")
    return bad



class Recorded:
    """Read-only command replies captured with a final state."""
    def __init__(self, queries):
        self.queries = queries

    def execute(self, command, params):
        key = query_key(command, params)
        if key not in self.queries:
            raise ValueError(f"missing recorded query: {command}")
        return self.queries[key]


def query_key(command, params):
    return json.dumps([command, params], sort_keys=True, separators=(",", ":"))


def evaluate(check, state, reply="", before_state=None, queries=None):
    """Score a document plus recorded read-only query replies; return (pass, reason)."""
    def finite(value):
        if isinstance(value, float):
            return math.isfinite(value)
        if isinstance(value, dict):
            return all(finite(v) for v in value.values())
        if isinstance(value, list):
            return all(finite(v) for v in value)
        return True

    if not finite([state, before_state, queries, check]):
        return False, "non-finite measurement"
    if not isinstance(state, dict):
        return False, "missing document"
    if any(f.get("error") for f in state.get("timeline", []) if isinstance(f, dict)):
        return False, "timeline contains a failed feature"
    ok, reason = run_check(check, state, reply,
                           fingerprint(before_state) if before_state is not None else None)
    bad = run_checks_v2(check, Recorded(queries or {}), state, reply, before_state)
    return ok and not bad, "; ".join(([reason] if reason != "ok" else []) + bad) or "ok"

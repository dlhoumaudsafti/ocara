#!/usr/bin/env python3
"""Graphiques et tableaux de docs/benchmarking.md depuis `make perf` et `make perf-extra`.

    make perf > perf.log ; make perf-extra > extra.log
    python3 ci/perf_report.py perf.log extra.log

Régénère docs/images/benchmark-{moyenne,memoire,demarrage,http,classement}.svg
et met à jour, dans docs/benchmarking.md, la date du relevé, les tableaux de
temps, de mémoire, de démarrage et de serveur HTTP, le classement et les
textes alternatifs des graphiques. Moyennes : voir docs/benchmarking.md.
"""

import datetime
import math
import re
import sys
from pathlib import Path

LANGS = ["Ocara", "Python", "PHP", "Perl", "C", "C++", "Rust"]
INTERPRETED = {"Python", "PHP", "Perl"}
COLOR = {"Ocara": "s0", "Python": "s1", "PHP": "s2", "Perl": "s3", "C": "s4", "C++": "s5", "Rust": "s6"}
DOC = Path("docs/benchmarking.md")
IMAGES = Path("docs/images")
STYLE = ("<style>svg { font-family: system-ui, -apple-system, 'Segoe UI', sans-serif; } .surface { fill: #fcfcfb; } "
         ".ink { fill: #0b0b0b; } .ink2 { fill: #52514e; } .grid { stroke: #e4e3df; } .ref { stroke: #52514e; } "
         ".s0 { fill: #2a78d6; } .s1 { fill: #eb6834; } .s2 { fill: #1baf7a; } .s3 { fill: #eda100; } "
         ".s4 { fill: #e87ba4; } .s5 { fill: #008300; } .s6 { fill: #6250d6; } "
         "@media (prefers-color-scheme: dark) { .surface { fill: #1a1a19; } .ink { fill: #f0efec; } "
         ".ink2 { fill: #c3c2b7; } .grid { stroke: #3a3936; } .ref { stroke: #c3c2b7; } .s0 { fill: #3987e5; } "
         ".s1 { fill: #d95926; } .s2 { fill: #199e70; } .s3 { fill: #c98500; } .s4 { fill: #d55181; } "
         ".s5 { fill: #008300; } .s6 { fill: #9085e9; } }</style>")


# ── Lecture des journaux ─────────────────────────────────────────────────────

def measures(line):
    """`Ocara 0.11 s | Python 0.52 | C —` → {langage: valeur ou None}."""
    values = {}
    for part in line.split(" : ", 1)[1].split("|"):
        name, raw = part.split()[:2]
        values[name] = None if raw == "—" else float(raw)
    return values


def read_logs(perf_path, extra_path):
    ansi = re.compile(r"\x1b\[[0-9;]*m")
    verdict = re.compile(r" — (plus lent|moins bon) .*$")
    times, memory, extra = {}, {}, {}
    for line in ansi.sub("", Path(perf_path).read_text()).splitlines():
        found = re.match(r"(OK|FAIL)\s+(\S+) \(n=\d+\) : ", line)
        if found:
            times[found.group(2)] = measures(verdict.sub("", line))
    for line in ansi.sub("", Path(extra_path).read_text()).splitlines():
        if " : Ocara " not in line:
            continue
        values = measures(verdict.sub("", line))
        found = re.match(r"(OK|FAIL)\s+mémoire (\S+)", line)
        if found:
            memory[found.group(2)] = {k: v and v / 1024 for k, v in values.items()}
        elif "démarrage" in line:
            extra["startup"] = values
        elif "serveur HTTP" in line:
            extra["http"] = values
    return times, memory, extra


# ── Moyennes ─────────────────────────────────────────────────────────────────

def geomean(values):
    values = list(values)
    return math.exp(sum(math.log(v) for v in values) / len(values))


def averages(times, memory, extra):
    """Temps relatif (et nombre de mesures), mémoire, démarrage, débit HTTP et indice global."""
    floor = 0.005
    result = {}
    shared = [row for row in memory.values() if all(row.get(lang) is not None for lang in LANGS)]
    for lang in LANGS:
        ratios = [max(row[lang], floor) / max(row["Ocara"], floor) for row in times.values() if row.get(lang) is not None]
        result[lang] = {
            "time": geomean(ratios), "count": len(ratios),
            "memory": geomean(row[lang] for row in shared),
            "startup": extra["startup"].get(lang), "http": extra["http"].get(lang),
        }
    ocara = result["Ocara"]
    for lang, avg in result.items():
        criteria = [avg["time"], avg["memory"] / ocara["memory"], avg["startup"] / ocara["startup"]]
        if avg["http"]:
            criteria.append(ocara["http"] / avg["http"])
        avg["index"], avg["criteria"] = geomean(criteria), len(criteria)
    return result, len(shared)


# ── Formats ──────────────────────────────────────────────────────────────────

def fr(value, digits):
    return f"{value:.{digits}f}".replace(".", ",")


def thousands(value):
    return f"{round(value):,}".replace(",", " ")


def marked(values, digits, higher_is_better=False):
    """Cellules d'une ligne : en gras Ocara meilleur que tout interprété et
    chaque langage meilleur qu'Ocara, souligné en gras un égal d'Ocara."""
    shown = {lang: None if v is None else round(v, digits) for lang, v in values.items()}
    better = (lambda a, b: a > b) if higher_is_better else (lambda a, b: a < b)
    ocara = shown["Ocara"]
    ocara_best = all(shown.get(o) is None or better(ocara, shown[o]) for o in INTERPRETED)

    def cell(lang):
        v = shown.get(lang)
        if v is None:
            return "—"
        text = fr(v, digits) if digits else str(round(v))
        if (lang == "Ocara" and ocara_best) or (lang != "Ocara" and better(v, ocara)):
            return f"**{text}**"
        if lang != "Ocara" and v == ocara:
            return f"<u>**{text}**</u>"
        return text

    return [cell(lang) for lang in LANGS]


# ── Graphiques ───────────────────────────────────────────────────────────────

def bar_chart(name, title, subtitle, footer, rows, ref, tick_format):
    """`rows` : (étiquette, langage, valeur, nombre affiché, texte de la
    barre), dans l'ordre d'affichage."""
    top = max(row[2] for row in rows)
    scale = 400 / top
    grid_bottom = 64 + 38 * len(rows) - 8
    height = grid_bottom + 72
    step = 10 ** math.floor(math.log10(top))
    ref_x = 150 + ref * scale
    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 720 {height}" width="720" height="{height}" role="img" aria-labelledby="t d">',
        f'<title id="t">{title}</title>',
        f'<desc id="d">{subtitle}. ' + "; ".join(f"{lang} {number}" for _, lang, _, number, _ in rows) + ".</desc>",
        STYLE,
        f'<rect class="surface" width="720" height="{height}" rx="8"/>',
        f'<text class="ink" x="24" y="30" font-size="16" font-weight="600">{title}</text>',
        f'<text class="ink2" x="24" y="50" font-size="12">{subtitle}</text>',
    ]
    tick = 0
    while tick <= top * 1.02:
        x = 150 + tick * scale
        out.extend([
            f'<line class="grid" x1="{x:.1f}" y1="58" x2="{x:.1f}" y2="{grid_bottom}" stroke-width="1"/>',
            f'<text class="ink2" x="{x:.1f}" y="{grid_bottom + 16}" font-size="11" text-anchor="middle">{tick_format(tick)}</text>',
        ])
        tick += step
    out.append(f'<line class="ref" x1="{ref_x:.1f}" y1="54" x2="{ref_x:.1f}" y2="{grid_bottom + 4}" stroke-width="1.5" stroke-dasharray="4 3"/>')
    for i, (label, lang, value, _, short) in enumerate(rows):
        y = 64 + 38 * i
        width = value * scale
        weight = ' font-weight="600"' if lang == "Ocara" else ""
        x = 150 + width + 8
        # Étiquette qui chevaucherait la ligne d'Ocara : placée après elle.
        if lang != "Ocara" and x < ref_x + 12 and x + 6.5 * len(short) > ref_x:
            x = ref_x + 12
        out.extend([
            f'<text class="ink" x="138" y="{y + 16:.1f}" font-size="13" text-anchor="end"{weight}>{label}</text>',
            f'<path class="{COLOR[lang]}" d="M150,{y} h{width - 4:.1f} a4,4 0 0 1 4,4 v16 a4,4 0 0 1 -4,4 h-{width - 4:.1f} z">'
            f'<title>{lang} : {short}</title></path>',
            f'<text class="ink" x="{x:.1f}" y="{y + 16:.1f}" font-size="12"{weight}>{short}</text>',
        ])
    out.append(f'<text class="ink2" x="360.0" y="{height - 16}" font-size="11" text-anchor="middle">{footer}</text>')
    out.append("</svg>")
    (IMAGES / f"benchmark-{name}.svg").write_text("\n".join(out) + "\n")


def charts(avg, shared):
    def order(key, reverse=False):
        return sorted((lang for lang in LANGS if avg[lang][key]), key=lambda lang: avg[lang][key], reverse=reverse)

    def int_tick(t):
        return str(round(t))

    def row(lang, key, number, short, label=None):
        return label or lang, lang, avg[lang][key], number, short

    rows = [row(lang, "time", fr(avg[lang]["time"], 2), f'{fr(avg[lang]["time"], 2)} × ({avg[lang]["count"]} mesures)') for lang in order("time")]
    bar_chart("moyenne", "Temps relatif moyen par langage (Ocara = 1)",
              "Moyenne géométrique du rapport à Ocara sur les mesures de make perf ; plus court = plus rapide",
              "Rapport moyen au temps d'Ocara (ligne pointillée : Ocara)", rows, 1, int_tick)
    rows = [row(lang, "memory", fr(avg[lang]["memory"], 1), f'{fr(avg[lang]["memory"], 1)} Mo') for lang in order("memory")]
    bar_chart("memoire", "Mémoire résidente maximale moyenne (Mo)",
              f"Moyenne géométrique sur les {shared} mesures exécutées par les sept langages ; plus court = plus économe",
              "Mo (ligne pointillée : Ocara)", rows, avg["Ocara"]["memory"], int_tick)
    rows = [row(lang, "startup", fr(avg[lang]["startup"], 2), f'{fr(avg[lang]["startup"], 2)} ms') for lang in order("startup")]
    bar_chart("demarrage", "Démarrage d'un programme vide (ms)",
              "Durée moyenne d'un lancement sur 50 (make perf-extra) ; plus court = plus rapide",
              "ms (ligne pointillée : Ocara)", rows, avg["Ocara"]["startup"], int_tick)
    rows = [row(lang, "http", thousands(avg[lang]["http"]), f'{thousands(avg[lang]["http"])} req/s') for lang in order("http", reverse=True)]
    bar_chart("http", "Serveur HTTP (requêtes par seconde)",
              "4 clients en connexions persistantes ; plus long = meilleur ; C, C++, Rust : pas de serveur standard",
              "requêtes/s (ligne pointillée : Ocara)", rows, avg["Ocara"]["http"], thousands)
    rows = []
    for rank, lang in enumerate(order("index"), 1):
        criteria = "" if avg[lang]["criteria"] == 4 else f' ({avg[lang]["criteria"]} critères sur 4)'
        rows.append(row(lang, "index", fr(avg[lang]["index"], 2), f'{fr(avg[lang]["index"], 2)}{criteria}', f"{rank}. {lang}"))
    bar_chart("classement", "Classement final : indice global par langage (Ocara = 1)",
              "Moyenne géométrique des rapports à Ocara : temps, mémoire, démarrage, serveur HTTP ; plus court = meilleur",
              "Indice global (ligne pointillée : Ocara)", rows, 1, int_tick)
    return order


# ── Document ─────────────────────────────────────────────────────────────────

def split_row(line):
    return [c.strip() for c in re.split(r"(?<!\\)\|", line.strip().strip("|"))]


def update_rows(text, header, values_of):
    """Remplace les cellules des lignes du tableau `header` ; `values_of(libellé)` → cellules ou None."""
    lines = text.split("\n")
    start = next(i for i, line in enumerate(lines) if line.startswith(header))
    for i in range(start + 2, len(lines)):
        if not lines[i].startswith("|"):
            break
        cells = split_row(lines[i])
        new = values_of(cells[0])
        if new:
            lines[i] = "| " + " | ".join([cells[0]] + new) + " |"
    return "\n".join(lines)


def update_doc(times, memory, extra, avg, shared, order):
    text = DOC.read_text()
    text = re.sub(r"Mesures du \d{4}-\d{2}-\d{2}", f"Mesures du {datetime.date.today().isoformat()}", text)
    text = re.sub(r"sur les \d+\nmesures que les sept", f"sur les {shared}\nmesures que les sept", text)

    def bench(label):
        found = re.search(r"\(`([^`]+)`\)$", label) or re.fullmatch(r"`([^`]+)`", label)
        return found.group(1) if found else None

    text = update_rows(text, "| Mesure (temps en s", lambda label: bench(label) in times and marked(times[bench(label)], 2))
    text = update_rows(text, "| Mesure (mémoire", lambda label: bench(label) in memory and marked(memory[bench(label)], 1))

    def extra_row(label):
        if label.startswith("Démarrage"):
            return marked(extra["startup"], 2)
        if label.startswith("Serveur HTTP"):
            return marked(extra["http"], 0, higher_is_better=True)
        return None

    text = update_rows(text, "| Mesure (`make perf-extra`)", extra_row)

    rows = []
    for rank, lang in enumerate(order("index"), 1):
        a = avg[lang]
        cells = [str(rank), lang, fr(a["index"], 2), fr(a["time"], 2), fr(a["memory"], 1), fr(a["startup"], 2),
                 thousands(a["http"]) if a["http"] else "—"]
        if lang == "Ocara":
            cells = [cells[0]] + [f"**{c}**" for c in cells[1:]]
        rows.append("| " + " | ".join(cells) + " |")
    text = re.sub(r"(\| Rang \| Langage [^\n]*\n\|[-|]+\|\n)(?:\|[^\n]*\n)+", lambda m: m.group(1) + "\n".join(rows) + "\n", text)

    alts = {
        "moyenne": "Temps relatif moyen par langage, Ocara = 1 : " + " ; ".join(f'{l} {fr(avg[l]["time"], 2)}' for l in order("time")),
        "memoire": "Mémoire résidente maximale moyenne en Mo : " + " ; ".join(f'{l} {fr(avg[l]["memory"], 1)}' for l in order("memory")),
        "demarrage": "Démarrage d'un programme vide en ms : " + " ; ".join(f'{l} {fr(avg[l]["startup"], 2)}' for l in order("startup")),
        "http": "Serveur HTTP en requêtes par seconde : " + " ; ".join(f'{l} {thousands(avg[l]["http"])}' for l in order("http", reverse=True)),
        "classement": "Classement final, indice global (Ocara = 1, plus petit = meilleur) : "
                      + " ; ".join(f'{r}. {l} {fr(avg[l]["index"], 2)}' for r, l in enumerate(order("index"), 1)),
    }
    for name, alt in alts.items():
        text = re.sub(rf"!\[[^\]]*\]\(images/benchmark-{name}\.svg\)", f"![{alt}](images/benchmark-{name}.svg)", text)
    DOC.write_text(text)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    times, memory, extra = read_logs(sys.argv[1], sys.argv[2])
    avg, shared = averages(times, memory, extra)
    order = charts(avg, shared)
    update_doc(times, memory, extra, avg, shared, order)
    for lang in LANGS:
        a = avg[lang]
        print(f'{lang:7} temps {fr(a["time"], 2)} ({a["count"]})  mémoire {fr(a["memory"], 1)} Mo  indice {fr(a["index"], 2)}')


if __name__ == "__main__":
    main()

// Glisser-déposer : composants de la palette (nœud neuf) et nœuds de
// l'aperçu ou de la structure (déplacement). Une cible est une place dans
// une liste d'enfants `{ parent, index }` ou un emplacement unique `{ slot }`
// (`layout` d'un écran vide, gabarit `item` d'une liste).

import { S, getIn, startsWith, template, insertNode, moveNode, placeNode, isContainer, axisOf } from './model.js';
import { nodeAt } from './canvas.js';

let payload = null; // { type } : composant neuf ; { from } : nœud déplacé

export function startDrag(ev, data) {
    payload = data;
    ev.dataTransfer.effectAllowed = data.from ? 'move' : 'copy';
    ev.dataTransfer.setData('text/plain', data.type || JSON.stringify(data.from));
    document.body.classList.add('d-dragging');
}

export function endDrag() {
    payload = null;
    hideLine();
    document.body.classList.remove('d-dragging');
}

export const dragging = () => payload;

/** La cible est-elle permise (pas de nœud déposé dans lui-même) ? */
function allowed(target) {
    if (!payload || !target) { return false; }
    if (!payload.from) { return true; }
    const place = target.slot || target.parent;
    return !startsWith(place, payload.from);
}

export function drop(target) {
    if (!allowed(target)) { endDrag(); return; }
    const { from, type } = payload;
    if (target.slot) {
        placeNode(target.slot, from ? getIn(S.model, from) : template(type), from);
    } else if (from) {
        moveNode(from, target.parent, target.index);
    } else {
        insertNode(target.parent, target.index, template(type));
    }
    endDrag();
}

// ── indicateur ─────────────────────────────────────────────────────────────

const stage = () => document.getElementById('stage');

/** Rectangle de `e` dans le repère défilant de l'aperçu. */
export function stageRect(e) {
    const r = e.getBoundingClientRect();
    const s = stage().getBoundingClientRect();
    return { x: r.left - s.left + stage().scrollLeft, y: r.top - s.top + stage().scrollTop, w: r.width, h: r.height };
}

function showLine(rect, box) {
    const line = document.getElementById('dropLine');
    line.hidden = false;
    line.className = box ? 'd-drop d-drop-box' : 'd-drop';
    Object.assign(line.style, { left: rect.x + 'px', top: rect.y + 'px', width: Math.max(rect.w, 2) + 'px', height: Math.max(rect.h, 2) + 'px' });
}

export function hideLine() {
    const line = document.getElementById('dropLine');
    if (line) { line.hidden = true; }
}

// ── cible dans l'aperçu ────────────────────────────────────────────────────

const pathOf = e => JSON.parse(e.dataset.path);
const kids = e => [...e.children].filter(c => c.dataset.path);

/** Rang d'insertion dans le conteneur `e` au point (x, y), et le trait à montrer. */
function slotIn(e, node, x, y) {
    const children = kids(e);
    const axis = axisOf(node);
    const box = stageRect(e);
    if (!children.length) { return { index: 0, rect: box, box: true }; }
    const rects = children.map(c => c.getBoundingClientRect());
    let index = children.length;
    for (let i = 0; i < rects.length; i++) {
        const r = rects[i];
        const before = axis === 'row' ? x < r.left + r.width / 2
            : axis === 'grid' ? (y < r.top || (y <= r.bottom && x < r.left + r.width / 2))
                : y < r.top + r.height / 2;
        if (before) { index = i; break; }
    }
    const ref = stageRect(children[Math.min(index, children.length - 1)]);
    const after = index === children.length;
    const rect = axis === 'row' || axis === 'grid'
        ? { x: after ? ref.x + ref.w + 1 : ref.x - 2, y: ref.y, w: 3, h: ref.h }
        : { x: ref.x, y: after ? ref.y + ref.h + 1 : ref.y - 2, w: ref.w, h: 3 };
    return { index, rect };
}

/** Cible sous le point (x, y) de l'aperçu, ou `null`. */
function canvasTarget(x, y) {
    const hit = document.elementFromPoint(x, y);
    if (!hit || !document.getElementById('canvas').contains(hit)) { return null; }
    const slot = hit.closest('[data-slot]');
    if (slot) { return { slot: JSON.parse(slot.dataset.slot), rect: stageRect(slot), box: true }; }
    let e = nodeAt(x, y);
    if (!e && hit.closest('.oc-screen')) {
        // Sous le dernier nœud de l'écran : à la fin du layout.
        const root = document.querySelector('#canvas .oc-screen > [data-path]');
        const node = root && getIn(S.model, pathOf(root));
        if (node && isContainer(node)) {
            const s = slotIn(root, node, x, Number.MAX_SAFE_INTEGER);
            return { parent: [...pathOf(root), 'children'], ...s };
        }
    }
    while (e) {
        const path = pathOf(e);
        const node = getIn(S.model, path);
        const parentEl = e.parentElement && e.parentElement.closest('[data-path]');
        const parentNode = parentEl && getIn(S.model, pathOf(parentEl));
        const inList = typeof path[path.length - 1] === 'number' && parentNode && isContainer(parentNode);
        if (isContainer(node)) {
            // Près du bord d'un conteneur placé dans une liste : à côté de lui, pas dedans.
            const r = e.getBoundingClientRect();
            const edge = Math.min(8, r.width / 4, r.height / 4);
            const nearEdge = x < r.left + edge || x > r.right - edge || y < r.top + edge || y > r.bottom - edge;
            if (!(inList && nearEdge)) {
                const s = slotIn(e, node, x, y);
                return { parent: [...path, 'children'], ...s };
            }
        }
        if (inList) {
            const s = slotIn(parentEl, parentNode, x, y);
            return { parent: [...pathOf(parentEl), 'children'], ...s };
        }
        e = parentEl;
    }
    return null;
}

export function installCanvasDrop() {
    const st = stage();
    st.addEventListener('dragover', ev => {
        if (!payload) { return; }
        const target = canvasTarget(ev.clientX, ev.clientY);
        if (!allowed(target)) { hideLine(); ev.dataTransfer.dropEffect = 'none'; return; }
        ev.preventDefault();
        ev.dataTransfer.dropEffect = payload.from ? 'move' : 'copy';
        showLine(target.rect, target.box);
    });
    st.addEventListener('dragleave', ev => { if (!st.contains(ev.relatedTarget)) { hideLine(); } });
    st.addEventListener('drop', ev => {
        ev.preventDefault();
        drop(canvasTarget(ev.clientX, ev.clientY));
    });
    document.addEventListener('dragend', endDrag);
}

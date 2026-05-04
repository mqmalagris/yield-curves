"""
Render the PCHIP-vs-natural-cubic-spline overshoot demo plot.

Output: assets/pchip-vs-spline.png

Use case for the plot:
    A central bank holds rates flat for several quarters, then hikes,
    then holds again. Anchors look like a step function with spaced
    pillars. Natural cubic spline cannot represent this without
    overshoot/undershoot — it dips below the lower plateau and pokes
    above the upper plateau between anchors. PCHIP preserves
    monotonicity locally and stays inside [13.5, 14.5] everywhere.

Run:
    python -m pip install --user matplotlib scipy numpy
    python scripts/plot_pchip_vs_spline.py
"""

from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
from scipy.interpolate import CubicSpline, PchipInterpolator


# Anchor points: BCB-like flat / hike / flat scenario.
# x = years to maturity, y = zero rate (%)
ANCHORS_X = np.array([0.5, 1.0, 1.5, 2.0, 3.0, 5.0])
ANCHORS_Y = np.array([13.5, 13.5, 13.5, 13.5, 14.5, 14.5])

# Fine grid for evaluation.
GRID = np.linspace(ANCHORS_X.min(), ANCHORS_X.max(), 600)

# Natural cubic spline (boundary condition: zero second derivative at ends).
spline = CubicSpline(ANCHORS_X, ANCHORS_Y, bc_type="natural")
spline_y = spline(GRID)

# PCHIP (Fritsch-Carlson monotone cubic Hermite).
pchip = PchipInterpolator(ANCHORS_X, ANCHORS_Y)
pchip_y = pchip(GRID)


# ---------------------------------------------------------------------------
# Plot
# ---------------------------------------------------------------------------

# Use a single figure with two stacked panels: full curve + zoomed inset of
# the overshoot region.
plt.style.use("seaborn-v0_8-whitegrid")

fig, (ax_main, ax_zoom) = plt.subplots(
    nrows=2,
    ncols=1,
    figsize=(9, 7),
    gridspec_kw={"height_ratios": [3, 2]},
)

# Plateau guide lines.
for level in (13.5, 14.5):
    ax_main.axhline(level, color="lightgrey", linewidth=0.8, linestyle=":")
    ax_zoom.axhline(level, color="lightgrey", linewidth=0.8, linestyle=":")

# Curves.
ax_main.plot(
    GRID, spline_y,
    color="#d62728", linewidth=2.0, label="Natural cubic spline",
)
ax_main.plot(
    GRID, pchip_y,
    color="#1f77b4", linewidth=2.0, label="PCHIP (Fritsch-Carlson)",
)

# Anchors.
ax_main.scatter(
    ANCHORS_X, ANCHORS_Y,
    color="black", s=40, zorder=5, label="Market anchors",
)

ax_main.set_title(
    "Cubic spline vs PCHIP on a flat→hike→flat curve",
    fontsize=14, fontweight="bold",
)
ax_main.set_xlabel("Maturity (years)")
ax_main.set_ylabel("Zero rate (%)")
ax_main.legend(loc="lower right", framealpha=0.95)

# Auto-fit ylim to show the spline overshoot/undershoot fully, with padding.
y_lo = min(float(spline_y.min()), float(pchip_y.min()), float(ANCHORS_Y.min())) - 0.30
y_hi = max(float(spline_y.max()), float(pchip_y.max()), float(ANCHORS_Y.max())) + 0.30
ax_main.set_ylim(y_lo, y_hi)

# Annotate the overshoot.
overshoot_x = GRID[np.argmax(spline_y)]
overshoot_y = float(spline_y.max())
ax_main.annotate(
    f"spline overshoots\nto {overshoot_y:.2f}%",
    xy=(overshoot_x, overshoot_y),
    xytext=(overshoot_x - 1.6, overshoot_y - 0.05),
    arrowprops={"arrowstyle": "->", "color": "#d62728"},
    color="#d62728",
    fontsize=10,
    ha="left",
)

# Annotate the undershoot.
undershoot_x = GRID[np.argmin(spline_y)]
undershoot_y = float(spline_y.min())
ax_main.annotate(
    f"spline undershoots\nto {undershoot_y:.2f}%",
    xy=(undershoot_x, undershoot_y),
    xytext=(undershoot_x + 0.4, undershoot_y - 0.30),
    arrowprops={"arrowstyle": "->", "color": "#d62728"},
    color="#d62728",
    fontsize=10,
)

# Zoom panel: focus on transition region.
zoom_mask = (GRID >= 1.5) & (GRID <= 3.5)
ax_zoom.plot(GRID[zoom_mask], spline_y[zoom_mask], color="#d62728", linewidth=2.0)
ax_zoom.plot(GRID[zoom_mask], pchip_y[zoom_mask], color="#1f77b4", linewidth=2.0)
ax_zoom.scatter(
    ANCHORS_X[(ANCHORS_X >= 1.5) & (ANCHORS_X <= 3.5)],
    ANCHORS_Y[(ANCHORS_X >= 1.5) & (ANCHORS_X <= 3.5)],
    color="black", s=40, zorder=5,
)
ax_zoom.set_title("Zoom: transition region (1.5y–3.5y)", fontsize=11)
ax_zoom.set_xlabel("Maturity (years)")
ax_zoom.set_ylabel("Zero rate (%)")

fig.tight_layout()

OUTPUT_DIR = Path(__file__).resolve().parent.parent / "assets"
OUTPUT_DIR.mkdir(exist_ok=True)
output_path = OUTPUT_DIR / "pchip-vs-spline.png"
fig.savefig(output_path, dpi=150, bbox_inches="tight")
print(f"wrote {output_path}")

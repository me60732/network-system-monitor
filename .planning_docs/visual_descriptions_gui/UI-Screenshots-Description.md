# COSMIC System Monitor UI Screenshots - Detailed Descriptions

This document provides detailed textual descriptions of the COSMIC System Monitor applet UI screenshots for reference by models without image recognition capabilities.

---

## Screenshot 1: CPU Temperature Settings Panel

**Layout:** Dark themed settings panel with white text on dark gray background.

**Header:**
- Back button (< Back) in top-left corner
- Title: "CPU Temperature" in large white text

**Description Text:**
"For Intel processors shows single highest temperature found across all sensors/cores."

**Toggle Options (right-aligned toggles):**
- Show chart: **ON** (cyan toggle)
- Show value: OFF (gray toggle)
- Show label: OFF (gray toggle)  
- Show icon: **ON** (cyan toggle)

**Dropdown Menus:**
- Temperature unit: "Celsius" (dropdown arrow)
- Chart type: "Ring" (dropdown arrow)

**Button:**
- "Colors" button (dark gray, right-aligned)

**Input Field:**
- Minimum temperature: text input showing "0"

**Visual Element (left side):**
- Large circular ring chart showing "35°" in red text
- Below chart: "35°C" label

---

## Screenshot 2: CPU Load Average Settings Panel

**Layout:** Dark themed settings panel, similar design to Screenshot 1.

**Header:**
- Back button (< Back) in top-left corner
- Title: "CPU Load Average" in large white text

**Toggle Options (right-aligned toggles):**
- Show chart: **ON** (cyan toggle)
- Show value: OFF (gray toggle)
- Show label: OFF (gray toggle)
- Show icon: **ON** (cyan toggle)

**Dropdown Menu:**
- Chart type: "Ring" (dropdown arrow)

**Button:**
- "Colors" button (dark gray, right-aligned)

**Visual Element (left side):**
- Large circular display showing "3.03" in white text
- Below: "3.03%" label

---

## Screenshot 3: Disk Load Settings Panel

**Layout:** Dark themed settings panel with multiple subsections.

**Header:**
- Back button (< Back) in top-left corner
- Title: "Disk load" in large white text

**Top-Level Toggle Options:**
- Combine disk Write and Read: OFF (gray toggle)
- Show label: OFF (gray toggle)
- Show icon: OFF (gray toggle)

**Section: "Disk write in bytes per second"**
- Small square chart thumbnail (orange line graph on dark background)
- Toggle options:
  - Show chart: OFF (gray toggle)
  - Show value: OFF (gray toggle)
- "Colors" button (dark gray)
- Display text: "W 49.1 KB/s"

**Section: "Disk read in bytes per second"**
- Small square chart thumbnail (empty/minimal activity, yellow accent on dark background)
- Toggle options:
  - Show chart: OFF (gray toggle)
  - Show value: OFF (gray toggle)
- "Colors" button (dark gray)
- Display text: "R 0.00 B/s"

---

## Screenshot 4: Main System Monitor Menu/Overview

**Layout:** Dark themed main menu panel showing system metrics overview.

**Header:**
- Button at top: "COSMIC System Monitor ⧉" (external link icon)

**Menu Item:**
- "General settings" with right arrow (>)

**Metric Rows (each with value on right and arrow >):**
1. **CPU:** 0.60%
2. **CPU Temperature:** 35°C
3. **Memory:** 29.7 GB / 43.7 GB / 125.6 GB
4. **Network:** ↓ 1.67 KB/s ↑ 1.50 KB/s
5. **Disk:** W 2.28 GB/s ↑ 81.2 MB/s
6. **GPU:** 0.00% 3.19 GB / 23.99 GB 41°C

All rows are clickable menu items with dark gray background.

---

## Screenshot 5: General Settings Panel

**Layout:** Dark themed settings panel with multiple configuration options.

**Header:**
- Back button (< Back) in top-left corner
- Title: "General settings" in large white text

**Version Info:**
- "Minimon version 1.1.2 for COSMIC."
- "Tip" badge (red circle with "?" icon) on right

**Configuration Options:**

**Refresh rate (seconds):**
- Decrement button (-)
- Value: 1.00
- Increment button (+)

**Value size:**
- Decrement button (-)
- Value: 11
- Increment button (+)

**Monospace font for values:**
- Checkbox (unchecked)

**Panel spacing:**
- Text: "Small" on left
- Slider control (cyan dot positioned mid-range)
- Text: "Large" on right

**System Monitor:**
- Dropdown arrow (▼)

**Content order:**
- Section with up/down arrows (▲ ▼) for each item:
  - CPU
  - CPU Temperature
  - GPU
  - Memory
  - Network
  - Disk

Each item can be reordered using the arrow buttons.

---

## Screenshot 6: Graphics (GPU) Settings Panel

**Layout:** Dark themed settings panel with cyan accent border.

**Header:**
- Back button (< Back) in top-left corner
- Title: "Graphics" in large white text

**GPU Info:**
- Text: "NVIDIA GeForce RTX 4090" in white

**Toggle Options:**
- Show label: OFF (gray toggle)
- Show icon: **ON** (cyan toggle)

**Section: "GPU load"**

**Visual Element (left side):**
- Large circular ring chart showing "2.00" in white text
- Below: "2.00%" label

**Toggle Options:**
- Show chart: **ON** (cyan toggle)
- Show value: OFF (gray toggle)

**Dropdown Menu:**
- Chart type: "Ring" (dropdown arrow)

**Button:**
- "Colors" button (dark gray, right-aligned)

**Section: "GPU Temperature"**

**Visual Element (left side):**
- Large circular ring chart showing "40°" in red text
- Below: "40°C" label

**Toggle Options:**
- Show chart: **ON** (cyan toggle)
- Show value: OFF (gray toggle)

**Dropdown Menus:**
- Temperature unit: "Celsius" (dropdown arrow)
- Chart type: "Ring" (dropdown arrow)

**Button:**
- "Colors" button (dark gray, right-aligned)

**Input Field:**
- Minimum temperature: text input showing "0"

---

## Screenshot 7: Memory Usage Settings Panel

**Layout:** Dark themed settings panel.

**Header:**
- Back button (< Back) in top-left corner
- Title: "Memory Usage" in large white text

**Toggle Options:**
- Show chart: **ON** (cyan toggle)
- Show allocated on chart: OFF (gray toggle)

**Info Text:**
"Allocated = total minus free. Includes system cache and buffers, which improve performance and are resized/released as needed."

**Visual Element (left side):**
- Large circular ring chart showing "30.0" in white text with cyan progress ring
- Below: "29.9 GB" label

**Toggle Options:**
- Show value: OFF (gray toggle)
- Show label: OFF (gray toggle)
- Show icon: **ON** (cyan toggle)
- As percentage: OFF (gray toggle)

**Dropdown Menu:**
- Chart type: "Ring" (dropdown arrow)

**Button:**
- "Colors" button (dark gray, right-aligned)

---

## Screenshot 8: Network Load Settings Panel

**Layout:** Dark themed settings panel.

**Header:**
- Back button (< Back) in top-left corner
- Title: "Network load" in large white text

**Top-Level Toggle Options:**
- Combine download and upload: **ON** (cyan toggle)
- Show bandwidth in bytes: **ON** (cyan toggle)
- Show label: OFF (gray toggle)
- Show icon: OFF (gray toggle)

**Section: "Network load in bytes per second"**

**Visual Element (left side):**
- Small square chart thumbnail showing green spike/bar on dark background

**Toggle Options:**
- Show chart: OFF (gray toggle)
- Show value: **ON** (cyan toggle)
- Use adaptive scale: **CHECKED** (cyan checkmark)

**Display Text:**
- "↓ 3.15 KB/s"
- "↑ 1.96 KB/s"

**Button:**
- "Colors" button (dark gray)

---

## Screenshot 9: Panel/Taskbar Compact View

**Layout:** Horizontal strip showing compact metric indicators on dark background.

**Metrics (left to right):**

1. **CPU Icon** (chip symbol): 0.90
2. **Temperature Icon** (thermometer): 34° (red circle indicator)
3. **Memory Icon** (RAM bars): 3.00
4. **GPU Temperature Icon**: 41° (red circle indicator)
5. **GPU Load Icon**: 3.17
6. **Network Icon** (bars): 29.4
7. **Network Bandwidth:**
   - "↓ 4.49 KB/s" (green)
   - "↑ 1.61 KB/s" (cyan)

Each metric has an icon on the left and numerical value on the right, displayed in a compact inline format suitable for a desktop panel/taskbar.

---

## Design Patterns Observed

**Color Scheme:**
- Background: Dark gray/charcoal (#1a1a1a or similar)
- Primary text: White
- Accent color: Cyan/turquoise (for active toggles, checkboxes, progress rings)
- Inactive toggles: Gray
- Warning/temperature indicators: Red circles

**Typography:**
- Sans-serif font throughout
- Large titles for panel headers
- Monospace option available for numeric values

**UI Components:**
- Toggle switches: Pill-shaped with sliding circle indicator
- Dropdown menus: Right-aligned arrow indicators
- Buttons: Rounded rectangles with dark gray background
- Chart visualizations: Circular ring charts predominant
- Input fields: Dark with light border

**Navigation:**
- Consistent back button (< Back) in top-left
- Arrow indicators (>) for navigable menu items
- Up/down arrows (▲ ▼) for reorderable lists

**Information Density:**
- Main menu shows overview with key metrics
- Detail panels provide extensive customization per metric
- Panel view shows ultra-compact representation for taskbar/panel integration

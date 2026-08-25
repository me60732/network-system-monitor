# COSMIC System Monitor UI - SVG Representations

This folder contains SVG representations of the minimon applet UI screenshots. These SVGs are both visually renderable and semantically readable by AI models without image recognition capabilities.

## SVG Files

1. **[01-cpu-temperature.svg](01-cpu-temperature.svg)** - CPU Temperature Settings Panel
   - Shows ring chart with 35°C reading
   - Toggle options for chart/value/label/icon display
   - Temperature unit dropdown and chart type selector
   - Minimum temperature input field

2. **[02-cpu-load-average.svg](02-cpu-load-average.svg)** - CPU Load Average Settings Panel
   - Ring chart showing 3.03% load
   - Toggle controls for display options
   - Chart type dropdown

3. **[03-disk-load.svg](03-disk-load.svg)** - Disk Load Settings Panel
   - Separate sections for disk write and disk read
   - Mini chart thumbnails showing activity graphs
   - Write: 49.1 KB/s, Read: 0.00 B/s
   - Toggle options for combining reads/writes

4. **[04-main-menu.svg](04-main-menu.svg)** - Main System Monitor Menu/Overview
   - COSMIC System Monitor header button
   - Clickable rows for each metric category:
     - CPU: 0.60%
     - CPU Temperature: 35°C
     - Memory: 29.7 GB / 43.7 GB / 125.6 GB
     - Network: ↓ 1.67 KB/s ↑ 1.50 KB/s
     - Disk: W 2.28 GB/s ↑ 81.2 MB/s
     - GPU: 0.00% 3.19 GB / 23.99 GB 41°C

5. **[05-general-settings.svg](05-general-settings.svg)** - General Settings Panel
   - Refresh rate and value size controls
   - Panel spacing slider
   - Content order with reorderable items (CPU, CPU Temperature, GPU, Memory, Network, Disk)
   - Each item has up/down arrow controls

6. **[06-graphics-gpu.svg](06-graphics-gpu.svg)** - Graphics (GPU) Settings Panel
   - NVIDIA GeForce RTX 4090 header
   - GPU load ring chart: 2.00%
   - GPU temperature ring chart: 40°C
   - Separate toggle controls for load and temperature displays
   - Chart type and temperature unit dropdowns

7. **[07-memory-usage.svg](07-memory-usage.svg)** - Memory Usage Settings Panel
   - Ring chart showing 30.0 (29.9 GB)
   - Info text about allocated memory calculation
   - Toggle for showing allocated on chart
   - As percentage toggle option

8. **[08-network-load.svg](08-network-load.svg)** - Network Load Settings Panel
   - Combine download/upload toggle
   - Show bandwidth in bytes toggle
   - Mini chart thumbnail with green activity bars
   - Download: ↓ 3.15 KB/s (green)
   - Upload: ↑ 1.96 KB/s (cyan)
   - Adaptive scale checkbox

9. **[09-panel-compact.svg](09-panel-compact.svg)** - Panel/Taskbar Compact View
   - Horizontal layout showing all metrics inline
   - Icons for each metric type
   - Compact numerical values
   - Temperature indicators with red circle badges
   - Network bandwidth with color-coded arrows

## Design Patterns

### Color Scheme
- **Background**: `#1a1a1a` (dark charcoal)
- **Primary text**: `#fff` (white)
- **Secondary text**: `#aaa`, `#ccc` (light gray variations)
- **Accent (active)**: `#00bcd4` (cyan/turquoise)
- **Inactive**: `#555`, `#999` (gray)
- **Warning/Hot**: `#ff4444` (red)
- **Success/Download**: `#4ade80` (green)
- **Alert/Write**: `#ff8800` (orange)

### UI Components
- **Toggle switches**: Pill-shaped with sliding circle indicator (45px × 24px)
- **Ring charts**: 50px radius, 12px stroke width
- **Buttons**: Rounded rectangles (4px radius) with `#333` background
- **Dropdowns**: Dark background with down arrow (▼) indicator
- **Input fields**: Dark with light border (`#444`)

### Typography
- **Headers**: 24px bold sans-serif
- **Body text**: 13-14px sans-serif
- **Values**: 12-14px monospace option available
- **Large values in charts**: 32px bold

## Usage with AI Models

These SVG files contain semantic XML structure that can be parsed by models without image recognition:
- All text is readable as `<text>` elements
- Colors are specified as hex codes
- Positions are explicit coordinates
- UI components are described through element groupings and attributes

To use with a model, you can:
1. Open the SVG file in a text editor
2. Copy the entire XML content
3. Paste into the model's context
4. The model can read the structure, text, colors, and layout

Alternatively, modern AI models with vision capabilities can render and view these SVGs directly as images.

## Reference Implementation

These designs are based on the [minimon-applet](https://github.com/cosmic-utils/minimon-applet) for the COSMIC desktop environment, which provides a system resource monitor in the panel with detailed settings for each metric type.

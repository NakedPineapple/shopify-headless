# Admin Panel Design System

Design guidelines for the Naked Pineapple admin panel. Focused on clarity, efficiency, and professional aesthetics.

---

## Design Direction: "Midnight Lagoon"

### The Feeling

The sun has set, but you're still on the island. The beach is quiet now—the crowds have gone, leaving only the soft rhythm of waves against the shore. Moonlight spills across the water, painting everything in deep blues and silver. The air is still warm. A gentle breeze carries the scent of salt and plumeria.

You're sitting at a table on the terrace of a boutique resort, laptop open, the lagoon stretching out before you. Tiki torches flicker in the distance. The coral accents of daylight have softened but haven't disappeared—they glow like embers, like the last warmth of sunset still clinging to the horizon.

This is the same tropical paradise, just after dark. The energy has shifted from vibrant celebration to calm productivity. The same natural beauty, experienced in stillness. The pineapple is still your symbol—but now it's silhouetted against a star-filled sky.

### Core Principles

- **Clarity** — Like moonlight on water, information surfaces with gentle contrast
- **Calm Focus** — The quiet hours when the real work gets done
- **Tropical Continuity** — Still part of the Naked Pineapple world, just the night shift
- **Warm Darkness** — Not cold or sterile; deep blues with warmth underneath
- **Easy on the Eyes** — Dark backgrounds for those late-night sessions

### Atmosphere

Deep lagoon blues. Moonlit surfaces. The coral accent glowing like a torch in the darkness. It's professional but never corporate—you're still barefoot, still island time, just working after hours.

The darkness isn't absence of the brand; it's the brand at rest. The same confidence, the same warmth, the same celebration of authenticity—just quieter. More focused. Ready to get things done while the storefront sleeps.

### Light Mode: "Morning Shade"

For those who prefer working in the light, the admin offers a companion mode—but it's not the full tropical blaze of the storefront.

This is early morning on the island. The sun is up but still low, the heat hasn't arrived yet. You're working from a shaded cabana, cool tile beneath your feet, a ceiling fan turning lazily overhead. The ocean is visible through white linen curtains. There's coffee instead of cocktails.

The palette is lighter but restrained—clean whites and soft grays rather than warm creams and sand. The coral accents are present but not dominant. It's professional tropical: the resort's back office, not the pool deck. Crisp, breathable, focused.

**Light mode characteristics:**
- Clean whites and cool grays (not warm cream)
- Coral used sparingly for actions and accents
- More neutral than the storefront's golden warmth
- Airy and open, but business-ready
- The shade, not the sun

### Relationship to Storefront

The admin is the same island, experienced at a different hour:

| Storefront | Admin (Dark) | Admin (Light) |
|------------|--------------|---------------|
| Golden hour sunshine | Moonlit lagoon | Morning shade |
| Vibrant coral | Coral embers | Coral accents |
| Warm cream sand | Deep blue waters | Cool white linen |
| Bustling energy | Quiet focus | Calm productivity |
| The beach | The moonlit lagoon | The shaded cabana |

The customer sees the beach at golden hour. The admin sees the same island—just from a quieter spot.

---

## Color System

### Design Philosophy

The admin uses **semantic color tokens** that automatically adapt to light/dark mode. This mirrors the storefront's approach but with the "Lagoon" palette instead of "Tropical Luxe".

**Use semantic tokens** (`bg-background`, `text-foreground`, `border-border`) rather than raw colors. This ensures automatic theme switching and consistency.

### The Lagoon Scale (oklch)

The foundation palette using oklch for perceptual uniformity:

```css
--lagoon-950: oklch(0.18 0.04 240);   /* Deep lagoon */
--lagoon-900: oklch(0.22 0.045 238);  /* Moonlit surface */
--lagoon-800: oklch(0.28 0.05 235);   /* Deeper water */
--lagoon-700: oklch(0.35 0.055 232);  /* Wave lines */
--lagoon-600: oklch(0.45 0.055 228);  /* Shallows */
--lagoon-500: oklch(0.55 0.05 225);   /* Muted ocean */
--lagoon-400: oklch(0.70 0.04 220);   /* Distant shore */
--lagoon-300: oklch(0.80 0.03 215);   /* Morning mist */
--lagoon-200: oklch(0.90 0.015 210);  /* Cool border */
--lagoon-100: oklch(0.96 0.008 220);  /* Soft shade */
--lagoon-50:  oklch(0.98 0.005 220);  /* Cool white */
```

### Semantic Tokens

These tokens auto-switch between light and dark mode:

| Token | Light Mode | Dark Mode | Usage |
|-------|------------|-----------|-------|
| `background` | lagoon-50 | lagoon-950 | Page background |
| `foreground` | lagoon-950 | moonlit white | Primary text |
| `card` | white | lagoon-900 | Cards, panels |
| `card-foreground` | lagoon-950 | moonlit white | Text on cards |
| `muted` | lagoon-100 | lagoon-800 | Subtle surfaces |
| `muted-foreground` | lagoon-500 | lagoon-400 | Secondary text |
| `border` | lagoon-200 | lagoon-700 | Borders, dividers |
| `input` | lagoon-100 | lagoon-800 | Form inputs |
| `ring` | coral | coral | Focus rings |
| `primary` | coral | coral (brighter) | Primary actions |
| `secondary` | teal | teal (brighter) | Secondary actions |
| `destructive` | red | red | Destructive actions |
| `success` | green | green | Success states |
| `warning` | amber | amber | Warning states |

### Sidebar Tokens

| Token | Light Mode | Dark Mode | Usage |
|-------|------------|-----------|-------|
| `sidebar` | lagoon-100 | lagoon-900 | Sidebar background |
| `sidebar-foreground` | lagoon-700 | lagoon-300 | Nav item text |
| `sidebar-muted` | lagoon-500 | lagoon-400 | Section labels |
| `sidebar-border` | lagoon-200 | lagoon-700 | Dividers |
| `sidebar-accent` | lagoon-200 | lagoon-800 | Hover states |

### Using Colors in Templates

```html
<!-- DO: Use semantic tokens -->
<div class="bg-background text-foreground border-border">
<button class="bg-primary text-primary-foreground">
<p class="text-muted-foreground">

<!-- DON'T: Use raw lagoon colors (except in sidebar) -->
<div class="bg-lagoon-900 text-lagoon-100">
```

### Brand Accents

| Purpose | Token | Notes |
|---------|-------|-------|
| **Primary** | `primary` | Coral - buttons, active states |
| **Secondary** | `secondary` | Teal - links, secondary actions |
| **Destructive** | `destructive` | Red - delete, errors |
| **Success** | `success` | Green - confirmations |
| **Warning** | `warning` | Amber - caution states |

---

## Typography

### Font Families

| Role | Font | Usage |
|------|------|-------|
| **Display** | Playfair Display | Brand logo only (sidebar) |
| **UI** | DM Sans | All UI text, headings, body |

### Type Scale

| Element | Classes | Size |
|---------|---------|------|
| Page title | `text-xl font-semibold` | 20px |
| Section heading | `text-lg font-medium` | 18px |
| Card title | `text-base font-medium` | 16px |
| Body text | `text-sm` | 14px |
| Labels/muted | `text-xs` | 12px |
| Sidebar section | `text-xs font-medium uppercase tracking-wider` | 12px |

### Text Colors

```html
<!-- Primary text -->
<p class="text-foreground">...</p>

<!-- Secondary/muted text -->
<p class="text-muted-foreground">...</p>

<!-- Labels -->
<span class="text-xs font-medium text-muted-foreground uppercase tracking-wider">...</span>
```

---

## Layout

### Page Structure

```
┌─────────────────────────────────────────────────────────┐
│ Sidebar (fixed, 256px / 64px collapsed)                 │
│ ┌─────┐                                                 │
│ │Logo │  ┌─────────────────────────────────────────────┐│
│ ├─────┤  │ Header (sticky, blur backdrop)              ││
│ │ Nav │  ├─────────────────────────────────────────────┤│
│ │     │  │                                             ││
│ │     │  │ Main Content (p-6)                          ││
│ │     │  │                                             ││
│ ├─────┤  │                                             ││
│ │User │  ├─────────────────────────────────────────────┤│
│ └─────┘  │ Footer                                      ││
│          └─────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

### Spacing

| Context | Value | Tailwind |
|---------|-------|----------|
| Page padding | 24px | `p-6` |
| Card padding | 16-24px | `p-4` to `p-6` |
| Section gap | 24px | `space-y-6` |
| Element gap | 12-16px | `gap-3` to `gap-4` |
| Tight gap | 8px | `gap-2` |

### Sidebar

- **Width:** 256px expanded, 64px collapsed
- **Background:** `bg-sidebar` (follows theme)
- **Text:** `text-sidebar-foreground`
- **Active state:** `bg-primary text-primary-foreground`
- **Hover state:** `bg-sidebar-accent`
- **Transition:** `transition: width 0.2s ease-in-out`

### Header

```html
<header class="sticky top-0 z-30 bg-card/80 backdrop-blur-sm border-b border-border">
```

---

## Components

### Cards

```html
<div class="card p-6">
    <h3 class="text-base font-medium text-foreground mb-4">Title</h3>
    <!-- content -->
</div>

<!-- Or with explicit classes -->
<div class="bg-card text-card-foreground rounded-lg border border-border p-6">
    ...
</div>
```

### Buttons

**Primary:**
```html
<button class="bg-coral text-white py-2 px-4 rounded-md hover:bg-coral/90 transition-colors font-medium">
    Action
</button>
```

**Secondary/Outline:**
```html
<button class="btn btn-outline">Cancel</button>

<!-- Or explicit -->
<button class="border border-border text-foreground py-2 px-4 rounded-md hover:bg-accent transition-colors">
    Cancel
</button>
```

**Ghost:**
```html
<button class="btn btn-ghost p-2">
    <i class="ph ph-icon text-xl"></i>
</button>

<!-- Or explicit -->
<button class="text-muted-foreground hover:text-foreground p-2 transition-colors">
    <i class="ph ph-icon text-xl"></i>
</button>
```

### Form Inputs

```html
<input type="text" class="input" placeholder="Placeholder...">

<!-- Or explicit -->
<input type="text"
       class="w-full px-4 py-2 bg-input text-foreground border-0 rounded-lg text-sm
              focus:ring-2 focus:ring-ring focus:bg-card transition-colors
              placeholder:text-muted-foreground"
       placeholder="Placeholder...">
```

### Tables

```html
<table class="table">
    <thead>
        <tr>
            <th>Column</th>
        </tr>
    </thead>
    <tbody>
        <tr>
            <td>Data</td>
        </tr>
    </tbody>
</table>

<!-- The .table class applies: -->
<!-- thead: bg-muted -->
<!-- th: text-muted-foreground uppercase tracking-wider -->
<!-- tbody: divide-y divide-border -->
<!-- tr:hover: bg-accent/50 -->
<!-- td: text-foreground -->
```

### Navigation Items

```html
<!-- The .nav-item class handles styling automatically -->
<a href="/path" class="nav-item">
    <i class="ph ph-icon text-xl"></i>
    <span class="sidebar-text">Label</span>
</a>

<!-- Active state -->
<a href="/path" class="nav-item active">
    <i class="ph ph-icon text-xl"></i>
    <span class="sidebar-text">Label</span>
</a>

<!-- Or with explicit Jinja conditional -->
<a href="/path" class="nav-item {% if active %}bg-primary text-primary-foreground{% endif %}">
    ...
</a>
```

### Status Badges

```html
<!-- Success -->
<span class="px-2 py-1 text-xs font-medium rounded-full bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
    Active
</span>

<!-- Warning -->
<span class="px-2 py-1 text-xs font-medium rounded-full bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400">
    Pending
</span>

<!-- Error -->
<span class="px-2 py-1 text-xs font-medium rounded-full bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400">
    Failed
</span>
```

---

## Icon System

**Library:** Phosphor Icons

### Navigation Icons

| Section | Icon |
|---------|------|
| Dashboard | `ph-squares-four` |
| Products | `ph-package` |
| Collections | `ph-folders` |
| Orders | `ph-receipt` |
| Customers | `ph-users` |
| Inventory | `ph-warehouse` |
| Discounts | `ph-tag` |
| Gift Cards | `ph-gift` |
| Payouts | `ph-wallet` |
| AI Chat | `ph-chats-circle` |
| Settings | `ph-gear` |
| Admin Users | `ph-shield-check` |
| Shopify | `ph-storefront` |

### UI Icons

| Action | Icon |
|--------|------|
| Search | `ph-magnifying-glass` |
| Notifications | `ph-bell` |
| User | `ph-user` |
| Sign out | `ph-sign-out` |
| Collapse | `ph-caret-left` / `ph-caret-right` |
| Theme (system) | `ph-monitor` |
| Theme (light) | `ph-sun` |
| Theme (dark) | `ph-moon` |

### Icon Sizing

| Context | Size | Class |
|---------|------|-------|
| Sidebar nav | 20px | `text-xl` |
| Header actions | 20px | `text-xl` |
| Inline with text | 16px | `text-base` |
| Small indicators | 12px | `text-xs` |

---

## Animations & Transitions

### Standard Transition

```css
transition-colors  /* For color changes */
transition: width 0.2s ease-in-out  /* Sidebar collapse */
```

### Hover States

- Buttons: Slight color shift or opacity change
- Nav items: Background color change
- Cards: No hover effect (static)
- Table rows: Subtle background highlight

### Loading States

```html
<!-- Spinner -->
<div class="spinner h-5 w-5"></div>

<!-- Skeleton -->
<div class="skeleton h-4 w-full"></div>
```

---

## Theme Toggle

Supports three modes: **System** → **Light** → **Dark** → **System**

```javascript
// Stored in localStorage as 'admin-theme'
// Values: 'light', 'dark', or removed (system)
```

### Icon States

| Mode | Icon |
|------|------|
| System | `ph-monitor` |
| Light | `ph-sun` |
| Dark | `ph-moon` |

---

## Responsive Behavior

### Breakpoints

| Name | Width | Usage |
|------|-------|-------|
| sm | 640px | Show/hide elements |
| md | 768px | Search bar visibility |
| lg | 1024px | Full layouts |
| xl | 1280px | Wider tables |

### Mobile Considerations

- Sidebar collapses to icon-only on mobile
- Search hidden on mobile (md:block)
- User info hidden on mobile (sm:block)
- Tables scroll horizontally

---

## Third-Party Integrations

### Quill Editor (Rich Text)

Styled with semantic tokens:
- Toolbar: `bg-muted`, `border-border`
- Container: `bg-card`, `border-border`
- Editor text: `text-foreground`
- Placeholder: `text-muted-foreground`
- Icons use `--muted-foreground`, highlight to `--foreground` on hover

### SortableJS (Drag & Drop)

```css
.sortable-ghost { opacity: 0.3; }
.sortable-drag { cursor: grabbing; }
[data-media-id] img { cursor: grab; }
[data-media-id] img:active { cursor: grabbing; }
```

---

## Accessibility

- Focus rings: `focus:ring-2 focus:ring-coral`
- Color contrast: WCAG AA compliant
- Keyboard navigation: Tab through interactive elements
- Screen reader: Semantic HTML, ARIA labels where needed
- Reduced motion: Respect `prefers-reduced-motion`

---

## File Structure

```
crates/admin/
├── static/
│   ├── css/
│   │   └── input.css          # Tailwind config + custom styles
│   ├── fonts/
│   │   ├── fonts.css          # Font-face declarations
│   │   ├── dm-sans-*.woff2
│   │   └── playfair-display-*.woff2
│   ├── vendor/
│   │   ├── htmx.min.js
│   │   ├── quill.min.js
│   │   ├── quill.snow.css
│   │   ├── sortable.min.js
│   │   └── phosphor-icons*.css
│   └── js/
│       └── webauthn.js
└── templates/
    ├── layouts/
    │   └── base.html          # Main layout with sidebar
    └── [feature]/
        └── *.html             # Feature templates
```

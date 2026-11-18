# Theme format

A theme is one JSON document: tokens with declared types and values, modifiers whose contexts change token values,
the order those apply in, and roles that map the names a library reads onto the theme's tokens. This page states the
format normatively. `nsh-theme` (`crates/nsh-theme`) is its one implementation: reading with diagnostics, checking,
writing, editing, layering, references and role checks. nsh-core, the toolchain and the CLI call it and never
re-implement its rules. A rule changes here and in the crate in one commit.

The page's tagged examples are tests: nsh-theme's test group `spec` reads this page and runs each by its info string.

| Info string | The block |
|---|---|
| `json theme-example` | The complete example (section 13). It reads, resolves at its defaults and checks with `check_roles` with no diagnostic. |
| `json theme` | A document that reads, resolves at its defaults and checks with no diagnostic. |
| `json theme-invalid <code>` | A document that read, resolved at its defaults and checked gives exactly one diagnostic, of `<code>`. |
| `json theme-roles` | Role declarations `{ name, type, doc?, required? }`: `check_roles` checks the blocks after it against them. |
| `json theme-patch` | A patch, read against the complete example, with no diagnostic. |

## 1. Top-level members

A document is a JSON object:

| Member | What |
|---|---|
| `$schema` | Ignored. |
| `name` | A string. |
| `tokens` | The tokens by name (section 2). |
| `modifiers` | The modifiers by name (section 5). |
| `resolutionOrder` | The order the modifiers apply in (section 6). |
| `roles` | Role name → token name (section 7). |

Any other member gives the warning `unknown-member` and is ignored. A section of the wrong JSON type gives
`bad-value` and is ignored.

Text that is not JSON, or JSON that is not an object, gives exactly one error `not-a-document` and an empty document.
JSON nested 128 levels deep or more is refused the same way.

```json theme-invalid not-a-document
["tokens"]
```

An object with a top-level `cssVars` member and no `tokens` member is the old format. It gives exactly one error
`legacy-format`, whose message names `nsh theme convert` (section 12), and an empty document. A `cssVars` member
beside `tokens` is an `unknown-member` warning. The content of a top-level `cssVars` is never read, with `tokens` or
without: it is only checked to be JSON, so a duplicate member inside it, or a second `cssVars`, is not reported.

```json theme-invalid legacy-format
{ "cssVars": { "light": { "background": "oklch(1 0 0)" } } }
```

A JSON object anywhere in the document (but inside a top-level `cssVars`, which is not read) with a member name
written twice gives the error `duplicate-member` where the second is written, and the first is kept.

```json theme-invalid duplicate-member
{
  "tokens": {
    "surface-app": { "kind": "color", "value": "#ffffff" },
    "surface-app": { "kind": "color", "value": "#000000" }
  }
}
```

```json theme-invalid unknown-member
{ "tokens": {}, "colors": { "primary": "#0090ff" } }
```

JSON member order never decides anything: the sections are read in a fixed order (tokens first), whatever order they
are written in. The order of the members of `tokens`, `modifiers`, each modifier's `contexts` and `roles` is kept, for
writing and for the tie rule of cycles (section 4).

## 2. The token entry

A token entry is keyed by its name. A name is a non-empty string without `{` or `}`; `/`, `.` and spaces are allowed.
Its members:

| Member | What |
|---|---|
| `kind` (required) | A value type's name (section 3). |
| `value` (required) | A value of that type, or a reference (section 4). |
| `path` (optional) | A string, unique across the document's tokens. |
| `doc` (optional) | A string. |

An unknown entry member gives an `unknown-member` warning and is ignored. These give an error, and the entry is left
out:

| Condition | Code |
|---|---|
| A bad name | `bad-name` |
| A missing or unknown `kind` | `bad-kind` |
| A missing `value` | `missing-value` |
| An entry that is not an object, a `path` or `doc` that is not a string, or a value that doesn't fit its kind | `bad-value` |
| A `path` an earlier token already has | `duplicate-path` |

An entry that depends on a token left out for its own error (a context entry for it, a reference to it, a role mapped
onto it) is left out with it, without a diagnostic of its own: the token's diagnostic explains it.

```json theme-invalid bad-name
{ "tokens": { "surface{app}": { "kind": "color", "value": "#ffffff" } } }
```

```json theme-invalid bad-kind
{ "tokens": { "space-3": { "kind": "dimension", "value": 12 } } }
```

```json theme-invalid missing-value
{ "tokens": { "space-3": { "kind": "number" } } }
```

```json theme-invalid duplicate-path
{
  "tokens": {
    "surface-app": { "kind": "color", "value": "#ffffff", "path": "surface.app" },
    "surface-card": { "kind": "color", "value": "#f9f9f9", "path": "surface.app" }
  }
}
```

## 3. Value types

The value types are a closed list; a new one is a change of the format, never a reader's guess. A token's type is
always declared by its `kind`, never inferred from its value.

| Type | JSON forms | nsh-theme |
|---|---|---|
| `color` | `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, in any letter case | `Color { r, g, b, a }` |
| `number` | A JSON number. Whether it means points or a unitless value is the reader's business. | `f64` |
| `font` | A non-empty string, a CSS font-family list (`"Inter, system-ui, sans-serif"`), kept whole | `Font` |
| `string` | A JSON string | `String` |
| `boolean` | `true` or `false` | `bool` |
| `duration` | A string `<n>ms` or `<n>s`, `n` ≥ 0 (digits, an optional fraction), held as milliseconds | `Duration` |
| `easing` | `[x1, y1, x2, y2]` with x1 and x2 in [0, 1], or `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` | `Easing::{Bezier, Keyword}` |
| `typography` | An object, below | `Typography` |
| `shadow` | An array of one or more layers, below | `Shadow(Vec<ShadowLayer>)` |
| `gradient` | `{ "type", "angle", "stops" }`, below | `Gradient { kind, stops }` |
| `blur` | `{ "type", "radius" }`, below | `Blur { kind, radius }` |
| `border` | `{ "color", "width", "style" }`, below | `Border { color, width, style }` |
| `transition` | `{ "duration", "delay", "easing" }`, below | `Transition { duration, delay, easing }` |

Colors are hex only: no CSS color syntax reaches the format. The easing keywords are CSS's standard curves.

```json theme
{
  "tokens": {
    "gray-1": { "kind": "color", "value": "#fcfcfc" },
    "gray-12-translucent": { "kind": "color", "value": "#202020CC" },
    "overlay": { "kind": "color", "value": "#0008" },
    "white": { "kind": "color", "value": "#fff" },
    "space-3": { "kind": "number", "value": 12 },
    "opacity-disabled": { "kind": "number", "value": 0.5 },
    "font-inter": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
    "label-confirm": { "kind": "string", "value": "OK" },
    "motion-reduced": { "kind": "boolean", "value": false },
    "duration-fast": { "kind": "duration", "value": "150ms" },
    "duration-slow": { "kind": "duration", "value": "0.4s" },
    "ease-standard": { "kind": "easing", "value": [0.2, 0, 0, 1] },
    "ease-exit": { "kind": "easing", "value": "ease-in" }
  }
}
```

**Compounds.** Each member of a compound is a literal or a reference to a token of the member's type (section 4). An
unknown member gives an `unknown-member` warning and is ignored. A missing required member, or a member of the wrong
type, gives `bad-value`, and the token is left out. Ranges (a weight from 1 to 1000, a position from 0 to 1, a width
or radius ≥ 0, x1 and x2 of an easing in [0, 1]) are checked on literals; a member reference is checked by its
target's type.

**`typography`** is an object:

- `fontSize`: a number, in points, required;
- `fontFamily`: a font;
- `fontWeight`: a number from 1 to 1000;
- `lineHeight`, in one of three forms: a literal number, a multiple of the size (`1.5`); a literal string `<n>px`,
  points (`"20px"`); or a reference to a `number` token, read as points like every other length
  (`LineHeight::Px`). A multiple is written only as a literal;
- `letterSpacing`: a number, in points.

```json theme
{
  "tokens": {
    "leading-body": { "kind": "number", "value": 20 },
    "text-body": {
      "kind": "typography",
      "value": { "fontFamily": "Inter, system-ui, sans-serif", "fontSize": 14, "fontWeight": 400, "lineHeight": "{leading-body}", "letterSpacing": 0 }
    },
    "text-title": { "kind": "typography", "value": { "fontSize": 20, "fontWeight": 600, "lineHeight": 1.2 } },
    "text-caption": { "kind": "typography", "value": { "fontSize": 12, "lineHeight": "16px" } }
  }
}
```

**`shadow`** is an array of one or more layers, the first on top. Each layer has `color` (required); `x`, `y`, `blur`
and `spread` (numbers in points, default 0); and `inset` (a boolean, default false).

```json theme
{
  "tokens": {
    "shadow-card": {
      "kind": "shadow",
      "value": [{ "color": "#0000001a", "y": 1, "blur": 3 }, { "color": "#0000000f", "y": 1, "blur": 2, "spread": -1 }]
    },
    "shadow-well": { "kind": "shadow", "value": [{ "color": "#0000000d", "y": 2, "blur": 4, "inset": true }] }
  }
}
```

**`gradient`** is `{ "type": "linear" | "radial" | "conic", "angle", "stops" }`. `angle` is in degrees: for a linear
gradient its direction (default 180, downwards), for a conic one its start (default 0); a radial gradient (an ellipse
to the farthest corner, at the center) takes none, and an `angle` there is an unknown member. `stops` has two or more
`{ "color", "position" }`, where `position` is optional and between 0 and 1.

```json theme
{
  "tokens": {
    "gradient-hero": {
      "kind": "gradient",
      "value": { "type": "linear", "angle": 135, "stops": [{ "color": "#0090ff", "position": 0 }, { "color": "#00a2c7", "position": 1 }] }
    },
    "gradient-glow": { "kind": "gradient", "value": { "type": "radial", "stops": [{ "color": "#ffffff" }, { "color": "#ffffff00" }] } },
    "gradient-spinner": { "kind": "gradient", "value": { "type": "conic", "angle": 90, "stops": [{ "color": "#0090ff" }, { "color": "#0090ff00" }] } }
  }
}
```

**`blur`** is `{ "type": "layer" | "background", "radius" }`, with `radius` a number ≥ 0. A layer blur blurs the view,
a background blur what is behind it.

```json theme
{
  "tokens": {
    "blur-glass": { "kind": "blur", "value": { "type": "background", "radius": 16 } },
    "blur-disabled": { "kind": "blur", "value": { "type": "layer", "radius": 2 } }
  }
}
```

**`border`** is `{ "color", "width", "style" }`, all required, with `width` a number ≥ 0 and `style` one of `solid`,
`dashed`, `dotted`.

```json theme
{ "tokens": { "border-card": { "kind": "border", "value": { "color": "#e8e8e8", "width": 1, "style": "solid" } } } }
```

**`transition`** is `{ "duration", "delay", "easing" }`: `duration` required, `delay` defaulting to `0ms` and
`easing` to `ease`.

```json theme
{
  "tokens": {
    "motion-press": { "kind": "transition", "value": { "duration": "100ms", "easing": [0.2, 0, 0, 1] } },
    "motion-tooltip": { "kind": "transition", "value": { "duration": "150ms", "delay": "0.5s", "easing": "ease-out" } }
  }
}
```

A value that doesn't fit its type is `bad-value`:

```json theme-invalid bad-value
{ "tokens": { "surface-app": { "kind": "color", "value": "white" } } }
```

**Writing** (`write`) gives two-space indentation, the members in their order, references as `{name}`, and canonical
forms: colors as lowercase `#rrggbb`, or `#rrggbbaa` below full opacity; durations as `<n>ms`; whole numbers without
a fraction (`12`, never `12.0`); easings and `lineHeight` in the form they were read. Compound members at their
default are left out. `read(&write(&doc))` gives `doc` back with no diagnostic, and writing a read document twice
gives the same bytes.

## 4. References

A reference is a string value of exactly `{name}`, with `name` a token name. It may be a token's whole value, a
compound's member, or a context's or patch's value. Its target must have the type the position needs:

- for a whole value, the token's own type;
- for a member, the member's type: color, number, font, duration or easing. A typography's `lineHeight` reference
  targets a `number` token, read as points.

At read time a reference to an undeclared token is `reference-missing`, and one whose target has the wrong type
`reference-type`; the entry is left out.

```json theme
{
  "tokens": {
    "gray-12": { "kind": "color", "value": "#202020" },
    "space-1": { "kind": "number", "value": 4 },
    "text-primary": { "kind": "color", "value": "{gray-12}" },
    "text-heading": { "kind": "color", "value": "{text-primary}" },
    "border-strong": { "kind": "border", "value": { "color": "{text-primary}", "width": "{space-1}", "style": "solid" } }
  }
}
```

```json theme-invalid reference-missing
{ "tokens": { "surface-app": { "kind": "color", "value": "{gray-1}" } } }
```

```json theme-invalid reference-type
{
  "tokens": {
    "space-3": { "kind": "number", "value": 12 },
    "surface-app": { "kind": "color", "value": "{space-3}" }
  }
}
```

**After the layers.** References resolve once, after every layer (section 9), against the final values, so an
override of a primitive reaches every token that refers to it. A chain (`a → {b}`, `b → {c}`) resolves through.

**Cycles.** When the layers form a cycle, the cycle's entry from the highest layer is left out, with
`reference-cycle` naming the cycle and that entry, and its token takes the next layer down's value. This repeats until
no cycle remains.

- Layers rank as section 9 orders them: a patch over a context over a token's own value; a later modifier in
  `resolutionOrder`, and a later patch, rank higher.
- Within one layer, the entry of the token that comes last in that layer's order is left out.
- A token that refers to a cycle's token then resolves through the remaining values. No other token is left out.
- A token whose own value is the cycle's lost entry has no value left, and neither do the tokens that refer to it.

A cycle may exist only under some picks: `resolve` reports it under the selection that makes it. Here the default
context of `elevation` closes one:

```json theme-invalid reference-cycle
{
  "tokens": {
    "gray-1": { "kind": "color", "value": "#fcfcfc" },
    "surface-app": { "kind": "color", "value": "{surface-card}" },
    "surface-card": { "kind": "color", "value": "{gray-1}" }
  },
  "modifiers": {
    "elevation": { "default": "raised", "contexts": { "flat": {}, "raised": { "surface-card": "{surface-app}" } } }
  },
  "resolutionOrder": ["elevation"]
}
```

`surface-card`'s entry in `raised` ranks above `surface-app`'s own value, so it is left out: `surface-card` reads
`{gray-1}`, and `surface-app` resolves through it.

## 5. Modifiers and contexts

`modifiers` maps a modifier's name to `{ "default", "contexts" }`. `contexts` maps a context's name to
`{ token name → value }`. A context only changes values: it adds no names, and an empty context changes nothing.
Which context each modifier shows is the selection's: the picked context, or else the `default`.

- A modifier with fewer than two contexts gives `too-few-contexts`, and one without a `default`, or whose `default`
  names none of its contexts, gives `bad-default`. In each case the modifier is left out; when both hold, both are
  reported (removing the default's context from a two-context modifier gives both).
- An entry for a token the document doesn't declare gives `undeclared-token`, and the entry is left out.
- An entry whose value doesn't fit the token's type gives `bad-value`. The entry is left out and the context stays.

```json theme
{
  "tokens": {
    "gray-1": { "kind": "color", "value": "#fcfcfc" },
    "gray-12": { "kind": "color", "value": "#202020" },
    "space-3": { "kind": "number", "value": 12 },
    "surface-app": { "kind": "color", "value": "{gray-1}" },
    "text-primary": { "kind": "color", "value": "{gray-12}" }
  },
  "modifiers": {
    "colorMode": { "default": "light", "contexts": { "light": {}, "dark": { "gray-1": "#111111", "gray-12": "#eeeeee" } } },
    "density": { "default": "regular", "contexts": { "regular": {}, "compact": { "space-3": 8 } } }
  },
  "resolutionOrder": ["colorMode", "density"]
}
```

```json theme-invalid undeclared-token
{
  "tokens": { "gray-1": { "kind": "color", "value": "#fcfcfc" } },
  "modifiers": {
    "colorMode": { "default": "light", "contexts": { "light": {}, "dark": { "gray-1": "#111111", "gray-2": "#191919" } } }
  },
  "resolutionOrder": ["colorMode"]
}
```

```json theme-invalid too-few-contexts
{
  "tokens": {},
  "modifiers": { "colorMode": { "default": "light", "contexts": { "light": {} } } },
  "resolutionOrder": ["colorMode"]
}
```

```json theme-invalid bad-default
{
  "tokens": {},
  "modifiers": { "colorMode": { "default": "system", "contexts": { "light": {}, "dark": {} } } },
  "resolutionOrder": ["colorMode"]
}
```

**Light and dark are an ordinary modifier.** A theme that has dark values has a modifier for them (by convention
`colorMode`, `light` and `dark`), and nothing else about it is special. What follows from that:

- The color scheme (the app's, the system's, a `ThemeScope`'s `appearance`) drives `dark:` variants, `colorScheme()`,
  adaptive style values and native chrome only, never token values.
- A `ThemeScope` shows the theme's dark values by also picking the theme's light/dark modifier in its `modifiers`;
  its `appearance` alone changes only its color scheme.
- A service that follows the system's appearance, or the app's own `setModifier`, keeps the theme's light/dark
  modifier in step with the color scheme.

**Values that depend on two modes** (an accent's high-contrast shade, say) take one of two forms. Combined contexts
name each pair:

```json theme
{
  "tokens": {
    "blue-9": { "kind": "color", "value": "#0090ff" },
    "blue-11": { "kind": "color", "value": "#0d74ce" },
    "cyan-9": { "kind": "color", "value": "#00a2c7" },
    "cyan-11": { "kind": "color", "value": "#107d98" },
    "intent-primary-bg": { "kind": "color", "value": "{blue-9}" }
  },
  "modifiers": {
    "accent": {
      "default": "blue",
      "contexts": {
        "blue": { "intent-primary-bg": "{blue-9}" },
        "blue-high-contrast": { "intent-primary-bg": "{blue-11}" },
        "cyan": { "intent-primary-bg": "{cyan-9}" },
        "cyan-high-contrast": { "intent-primary-bg": "{cyan-11}" }
      }
    }
  },
  "resolutionOrder": ["accent"]
}
```

Or a separate modifier's contexts point at other tokens through references, which resolve after both modifiers
applied (section 4):

```json theme
{
  "tokens": {
    "blue-9": { "kind": "color", "value": "#0090ff" },
    "blue-11": { "kind": "color", "value": "#0d74ce" },
    "cyan-9": { "kind": "color", "value": "#00a2c7" },
    "cyan-11": { "kind": "color", "value": "#107d98" },
    "accent-solid": { "kind": "color", "value": "{blue-9}" },
    "accent-solid-strong": { "kind": "color", "value": "{blue-11}" },
    "intent-primary-bg": { "kind": "color", "value": "{accent-solid}" }
  },
  "modifiers": {
    "accent": {
      "default": "blue",
      "contexts": {
        "blue": { "accent-solid": "{blue-9}", "accent-solid-strong": "{blue-11}" },
        "cyan": { "accent-solid": "{cyan-9}", "accent-solid-strong": "{cyan-11}" }
      }
    },
    "contrast": {
      "default": "normal",
      "contexts": { "normal": {}, "high": { "intent-primary-bg": "{accent-solid-strong}" } }
    }
  },
  "resolutionOrder": ["accent", "contrast"]
}
```

## 6. `resolutionOrder`

`resolutionOrder` is an array of modifier names: the order their selected contexts apply in, a later one winning when
two set one token. It is required when `modifiers` has any member, and lists each modifier exactly once.

| Condition | Code | What happens |
|---|---|---|
| A modifier missing from it | `order-missing` | The modifier is left out. |
| `resolutionOrder` missing, with modifiers present | `order-missing`, once per modifier | Every modifier is left out. |
| A name that isn't a modifier | `order-unknown` | The item is ignored. |
| A second listing of a name | `order-duplicate` | The item is ignored. |
| An item that is not a string | `bad-value` | The item is ignored. |

A modifier left out for its own error (`too-few-contexts`, `bad-default`) still counts as a modifier here: its item is
dropped without a diagnostic of its own.

```json theme-invalid order-missing
{
  "tokens": {},
  "modifiers": { "colorMode": { "default": "light", "contexts": { "light": {}, "dark": {} } } }
}
```

```json theme-invalid order-unknown
{
  "tokens": {},
  "modifiers": { "colorMode": { "default": "light", "contexts": { "light": {}, "dark": {} } } },
  "resolutionOrder": ["colorMode", "density"]
}
```

```json theme-invalid order-duplicate
{
  "tokens": {},
  "modifiers": { "colorMode": { "default": "light", "contexts": { "light": {}, "dark": {} } } },
  "resolutionOrder": ["colorMode", "colorMode"]
}
```

## 7. Roles

`roles` maps a role's name to a token's name: the names a library reads (`primary`, `background`) onto the theme's
semantic tokens (`intent-primary-bg`, `surface-app`). A mapping onto an undeclared token gives
`role-token-missing`, and the mapping is left out.

**What a library registers.** A role is a `RoleDeclaration { name, value_type, doc, required }`
(`nsh_theme::RoleDeclaration`; nsh-core's `RoleSpec`). nsh-core's `roles!` takes `pub NAME: Type = "name"`, with an
optional `required` after the name, and generates a typed `Role<V>` constant per role and `all()`, the declarations
with their docs. `Registry::roles(specs)` lists a UI set's roles: a role two sets list with the same type is listed
once; with different types it gives the warning `role-conflict` (an empty pointer, a message naming the role, both
types and both sets) and keeps the first. The core's own roles are declarations too, none required: `popover`,
`popover-foreground`, `primary`, `accent`, `accent-foreground`, `muted-foreground`, `border` and `destructive`
(colors), and `font-sans` (a font).

**How a role resolves** (`resolve_role`):

1. Its layers, highest first: the patches' `roles` (a later patch over an earlier one), then the document's `roles`.
2. A mapping refused at its layer (`role-token-missing`, or `role-type`) is dropped from that layer, and the role
   resolves through the next layer down.
3. With no mapping left, the role reads the token of its own name.
4. Only when the token reached that way has the wrong type does the role give `role-type` and read nothing.

**Which declaration a mapping is checked against** (`check_roles`):

- a declaration that names its role;
- else a pattern whose prefix the role starts with (`RolePattern`: a prefix and the value types it accepts; nsh-core
  passes Tailwind's namespace table, so `radius-card` must map onto a number). A token type the pattern doesn't accept
  gives `role-type`, and step 2 applies;
- else a mapping onto a color token is known, since Tailwind's bare color form reads any name;
- else the mapping gives `role-unknown`, a warning, and is kept: another library may read it.

A `required` role that resolves to no token with a value gives the warning `role-unmapped`. `check_roles` does not
report `role-token-missing` again: `read` and `Patch::check` do.

The blocks from here to section 13 are checked against this library's declarations:

```json theme-roles
[
  { "name": "primary", "type": "color", "doc": "The main action's background." },
  { "name": "primary-foreground", "type": "color", "doc": "Text and icons on `primary`." },
  { "name": "background", "type": "color", "doc": "The app's background." },
  { "name": "foreground", "type": "color", "doc": "The app's text." },
  { "name": "font-sans", "type": "font", "doc": "The interface's font family." }
]
```

```json theme
{
  "tokens": {
    "blue-9": { "kind": "color", "value": "#0090ff" },
    "white": { "kind": "color", "value": "#ffffff" },
    "font-inter": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
    "intent-primary-bg": { "kind": "color", "value": "{blue-9}" },
    "intent-primary-fg": { "kind": "color", "value": "{white}" },
    "surface-card": { "kind": "color", "value": "{white}" },
    "font-ui": { "kind": "font", "value": "{font-inter}" }
  },
  "roles": {
    "primary": "intent-primary-bg",
    "primary-foreground": "intent-primary-fg",
    "font-sans": "font-ui",
    "card": "surface-card"
  }
}
```

`card` is no declared role, and maps onto a color: it is known. A mapping onto an undeclared token, a token of the
wrong type, and an undeclared role onto a token that is not a color:

```json theme-invalid role-token-missing
{
  "tokens": { "blue-9": { "kind": "color", "value": "#0090ff" } },
  "roles": { "primary": "intent-primary-bg" }
}
```

```json theme-invalid role-type
{
  "tokens": { "surface-app": { "kind": "color", "value": "#ffffff" } },
  "roles": { "font-sans": "surface-app" }
}
```

```json theme-invalid role-unknown
{
  "tokens": { "space-3": { "kind": "number", "value": 12 } },
  "roles": { "gutter": "space-3" }
}
```

## 8. The override patch

A patch is `{ "values": { token name → value }, "roles": { role → token name } }`. `read_patch` reads it and never
panics (`not-a-document`, `duplicate-member`, `unknown-member`, and `bad-value` for a section or role mapping of the
wrong JSON type); `write_patch` writes it as `write` writes a document.

A patch is stored as given: its values are read against the active document's types. `Patch::check(&document)` is
its one validator. It runs when a patch or an update arrives, and its diagnostics are logged and returned:

- `undeclared-token`: a value for a name the document doesn't declare. A patch never adds a name;
- `bad-value`: a value that doesn't fit the token's type;
- `reference-missing` and `reference-type`: a reference in a value, as in a document;
- `role-token-missing`: a role mapping onto an undeclared token;
- `unknown-member`: an unknown member of a compound value (a warning).

`resolve` applies the same rule, skipping exactly the entries `check` refuses. So an override of a name document B
lacks is skipped while B is active, and applies again when document A is active again.

The app has one patch: `themeOverrides`' entries, then every `update_tokens` / `updateTheme` merged into it in call
order, a later entry replacing an earlier one for the same token or role (`Patch::merge`). `reset_tokens` removes
names from its `values` and `roles`; a reset never copies values.

```json theme-patch
{
  "values": { "blue-9": "#3e63dd", "space-3": 10 },
  "roles": { "primary": "intent-primary-bg-hover" }
}
```

## 9. The layer table

`resolve(&document, &selection, &patches)` is the one implementation of the stack. It applies, in order:

| Layer | What |
|---|---|
| 1 | The document's token values. |
| 2 | The picked contexts, in `resolutionOrder`: for each modifier, the context the `Selection` names, or else its `default`. A picked context that doesn't exist gives the warning `unknown-context`, and the default is used. |
| 3 | The app's overrides: `themeOverrides`, then `update_tokens` / `updateTheme` in call order (the app's one patch). |
| 4 | The host's overrides (the host's patch). |
| 5 | References, resolved once (section 4). |

A higher layer wins. Overrides always win over a picked context, and a patched token keeps the patch's value after
another context is picked. `resolve` gives the values with no reference left, the resolved role mapping and its
diagnostics; `Resolved::changed` lists exactly the token names whose values differ and the role names whose resolved
token or value differs, which is the change set the runtime hands on. It runs once per change, never per read, and
costs O(tokens + references + patch entries).

**A `ThemeScope`** resolves its subtree with the same stack: its `modifiers` picks over the surface's (or the outer
scope's) selection, and its patch (`theme`) appended after every outer patch, the host's included. An outer scope's
patch and the host's overrides survive an inner scope's pick. A scope's bad patch entries are reported and left out.

## 10. The active document

One theme document is active at a time, by this precedence:

1. the app's theme: `Root::theme(json)` or `nsh.json`'s `"theme"` (the app's built-in document), replaced at run
   time by TS `setTheme(doc)` or Rust `set_theme(Some(json))`;
2. else the document of the first UI set, in the host's set order, that registers `Registry::theme_document(json)`.
   Each other set's document gives the warning `document-unused`;
3. else the document from `Registry::fallback_theme_document(json)`.

Documents are never merged: a token of an unused document is not readable. `set_theme(json: Option<&str>)` (and the
module context's `cx.set_theme(..)`) applies in order with the app's other theme actions and returns and logs the
read diagnostics. `setTheme(null)` and `set_theme(None)` return to the app's built-in document when there is one, else
to the sets' precedence.

A text whose `read` gives `not-a-document` or `legacy-format` is never activated and never counts in the precedence:
`setTheme` and `set_theme` keep the active document and return the diagnostic, and a set's or the fallback's
registration of such a text is skipped.

Activating another document keeps the selection for the modifiers and contexts that exist in the new document (the
others go back to their defaults) and keeps the app's patch as given: entries for names the new document lacks are
skipped and reported, and apply again if a document that has those names is activated again. Activating a document,
including an edited version of the active one, hands on a change set like any other change.

## 11. Diagnostics

Each diagnostic has a `severity` (error or warning), a stable kebab-case `code`, a `pointer` (an RFC 6901 JSON
pointer into the text as written, so tools can show the line) and a `message` (an English sentence naming the entry).
One condition has one code, reported by one function:

| Condition | Code | Severity | Reported by |
|---|---|---|---|
| text not JSON, or not an object | `not-a-document` | error | `read`, `read_patch` |
| top-level `cssVars` and no `tokens` | `legacy-format` | error | `read` |
| a duplicate member name (not inside a top-level `cssVars`, whose content is never read) | `duplicate-member` | error | `read`, `read_patch` |
| an unknown member | `unknown-member` | warning | `read`, `read_patch`, `Patch::check` (a compound's) |
| a bad token name | `bad-name` | error | `read`, `check` |
| a missing or unknown `kind` | `bad-kind` | error | `read` |
| a token without `value` | `missing-value` | error | `read` |
| a section, entry or compound member of the wrong JSON type or shape, a compound missing a required member, or a value that doesn't fit its type | `bad-value` | error | `read`, `check`, `read_patch`, `Patch::check` |
| a duplicate `path` | `duplicate-path` | error | `read`, `check` |
| a context entry or patch value for an undeclared token | `undeclared-token` | error | `read`, `check`, `Patch::check` |
| a modifier with fewer than two contexts | `too-few-contexts` | error | `read`, `check` |
| a modifier without `default`, or with a `default` that names no context | `bad-default` | error | `read`, `check` |
| a modifier left out of `resolutionOrder`, or `resolutionOrder` missing (once per modifier) | `order-missing` | error | `read`, `check` |
| a `resolutionOrder` item that names no modifier | `order-unknown` | error | `read`, `check` |
| a second listing in `resolutionOrder` | `order-duplicate` | error | `read`, `check` |
| a selected context that doesn't exist | `unknown-context` | warning | `resolve`, the core's `set_modifier` |
| a selected modifier that doesn't exist | `unknown-modifier` | warning | the core's `set_modifier` |
| a reference to an undeclared token | `reference-missing` | error | `read`, `check`, `resolve`, `Patch::check` |
| a reference whose target has the wrong type | `reference-type` | error | `read`, `check`, `Patch::check` |
| a cycle the layers form | `reference-cycle` | error | `resolve` |
| a role mapping onto an undeclared token (a document's or a patch's) | `role-token-missing` | error | `read`, `check`, `Patch::check` |
| a role whose token has the wrong type | `role-type` | error | `check_roles` |
| a required role that resolves to nothing | `role-unmapped` | warning | `check_roles` |
| a mapping for a role nothing declares or matches | `role-unknown` | warning | `check_roles` |
| a role two sets register with different value types | `role-conflict` | warning | `Registry::roles` |
| a set's document that another document's precedence leaves unused | `document-unused` | warning | `Registry` (the core's activation) |

`check(&document)` applies `read`'s rules to a document edited in memory, with the codes and pointers `read` would
give for its `write`; the rules of the JSON text itself cannot fail there.

The pointers: a token `/tokens/<name>`, a context entry `/modifiers/<modifier>/contexts/<context>/<token>`, a modifier
`/modifiers/<name>`, a `resolutionOrder` item `/resolutionOrder/<index>`, a role `/roles/<role>`, a patch's value
`/values/<token>`, a role's own-name token `/tokens/<role>`; an unknown or duplicate member points at the member
itself, and the whole text is the empty pointer.

**Errors leave the entry out, warnings keep it.** Nothing else happens on a diagnostic: no panic, no refusal of the
whole document. The runtime leaves the entry out and logs each diagnostic once per load, as
`log::warn!(target: "nsh::theme", "{code} {pointer}: {message}")`. TS `setTheme` and `updateTheme` return them, and
the bundler turns them into bundle warnings. `nsh theme check` prints each as
`<severity> <code> <pointer>: <message>` and exits 1 on any error or `role-unmapped`, else 0.

## 12. The conversion from `cssVars`

`nsh theme convert <old.json> [--over <base.json>]` (nsh-core's `theme_convert::convert`) writes an old `cssVars`
theme in this format. Its rules:

1. **Tokens.** Every name in `cssVars.theme`, `light` and `dark`, and every declared token, becomes a token. Number,
   radius, spacing, text, weight, tracking, leading and blur become `number`; color, font, shadow, gradient, string,
   boolean, duration and easing stay; a typography becomes a `typography` compound built from the size and its
   `--line-height`, `--letter-spacing` and `--font-weight` parts, which are not tokens of their own; a name with no
   declaration gets the kind the old reader would guess. Every declared token's `path` and `doc` are copied.
2. **Values.** A token's value is the `theme` value, else the `light` value, in this format's forms: colors as
   `#rrggbb`, or `#rrggbbaa` below full opacity, each channel rounded half away from zero; lengths as points; shadows
   and gradients as structured values.
3. **Appearance.** When any value differs between `light` and `dark`, the converter adds the modifier `colorMode`:
   default `light`, an empty context `light`, and a context `dark` with those names' dark values. It comes first in
   `resolutionOrder`. A source that already has a modifier named `colorMode` gives an error and is not converted.
4. **Old modifiers** keep their names, context names and defaults, after `colorMode`, in file order. An old context's
   value that is the same in both appearances becomes the entry as it is; one that differs becomes a reference
   `{<token>.<modifier>.<context>}` to a helper token holding the light value, which `colorMode`'s `dark` context
   sets to the dark value.
5. **`var()`.** A whole value `var(--x)` becomes `{x}`, with `x` renamed to its token when `x` is a role. Any other
   `var()` is copied as the old reader copied it.
6. **Roles** are copied.
7. **Over a base.** With `--over`, an old app theme that extended a library's old default becomes one complete
   document: the base's tokens and the app's together, the app's winning by name.
8. **What the format can't express** is reported and left out.

The output reads with no error, and for every combination (every appearance, with every choice of one context per old
modifier) its resolved values equal the old reader's values for the same names.

## 13. A complete example

Semantic tokens (`surface-app`, `text-primary`, `intent-primary-bg`) stand on primitives (`gray-1`, `blue-9`). The
`colorMode` modifier sets primitives only; the `accent` modifier points the intents at primitives through references
only; `density` sets numbers, which a typography's `lineHeight` follows. `roles` map a library's role names onto the
semantic tokens, never onto palette colors. Under `colorMode: dark` and `accent: cyan`, `intent-primary-bg-hover` is
the dark value of `cyan-10`, `#23afd0`.

The library its roles serve registers:

```json theme-roles
[
  { "name": "primary", "type": "color", "doc": "The main action's background.", "required": true },
  { "name": "primary-foreground", "type": "color", "doc": "Text and icons on `primary`.", "required": true },
  { "name": "background", "type": "color", "doc": "The app's background.", "required": true },
  { "name": "foreground", "type": "color", "doc": "The app's text.", "required": true },
  { "name": "card", "type": "color", "doc": "A card's background." },
  { "name": "border", "type": "color", "doc": "Dividers and outlines." },
  { "name": "muted-foreground", "type": "color", "doc": "Secondary text." },
  { "name": "font-sans", "type": "font", "doc": "The interface's font family." },
  { "name": "text-body", "type": "typography", "doc": "Running text." }
]
```

```json theme-example
{
  "$schema": "https://nsh.dev/schema/theme.json",
  "name": "Example",
  "tokens": {
    "gray-1": { "kind": "color", "value": "#fcfcfc", "path": "palette.gray.1" },
    "gray-2": { "kind": "color", "value": "#f9f9f9", "path": "palette.gray.2" },
    "gray-6": { "kind": "color", "value": "#dcdcdc", "path": "palette.gray.6" },
    "gray-11": { "kind": "color", "value": "#646464", "path": "palette.gray.11" },
    "gray-12": { "kind": "color", "value": "#202020", "path": "palette.gray.12" },
    "blue-9": { "kind": "color", "value": "#0090ff", "path": "palette.blue.9" },
    "blue-10": { "kind": "color", "value": "#0588f0", "path": "palette.blue.10" },
    "cyan-9": { "kind": "color", "value": "#00a2c7", "path": "palette.cyan.9" },
    "cyan-10": { "kind": "color", "value": "#0797b9", "path": "palette.cyan.10" },
    "white": { "kind": "color", "value": "#ffffff", "path": "palette.white" },
    "font-inter": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
    "space-3": { "kind": "number", "value": 12, "doc": "The base gap, in points." },
    "leading-body": { "kind": "number", "value": 20 },

    "surface-app": { "kind": "color", "value": "{gray-1}", "path": "surface.app", "doc": "The app's background." },
    "surface-card": { "kind": "color", "value": "{gray-2}", "path": "surface.card" },
    "border-subtle": { "kind": "color", "value": "{gray-6}", "path": "border.subtle" },
    "text-primary": { "kind": "color", "value": "{gray-12}", "path": "text.primary" },
    "text-muted": { "kind": "color", "value": "{gray-11}", "path": "text.muted" },
    "intent-primary-bg": { "kind": "color", "value": "{blue-9}", "path": "intent.primary.bg" },
    "intent-primary-bg-hover": { "kind": "color", "value": "{blue-10}", "path": "intent.primary.bg-hover" },
    "intent-primary-fg": { "kind": "color", "value": "{white}", "path": "intent.primary.fg" },
    "font-ui": { "kind": "font", "value": "{font-inter}" },
    "type-body": {
      "kind": "typography",
      "value": { "fontFamily": "{font-ui}", "fontSize": 14, "fontWeight": 400, "lineHeight": "{leading-body}" }
    },
    "shadow-card": {
      "kind": "shadow",
      "value": [{ "color": "#0000001a", "y": 1, "blur": 3 }, { "color": "#0000000f", "y": 1, "blur": 2, "spread": -1 }]
    },
    "border-card": { "kind": "border", "value": { "color": "{border-subtle}", "width": 1, "style": "solid" } },
    "motion-press": { "kind": "transition", "value": { "duration": "100ms", "easing": [0.2, 0, 0, 1] } }
  },
  "modifiers": {
    "colorMode": {
      "default": "light",
      "contexts": {
        "light": {},
        "dark": {
          "gray-1": "#111111",
          "gray-2": "#191919",
          "gray-6": "#3a3a3a",
          "gray-11": "#b4b4b4",
          "gray-12": "#eeeeee",
          "blue-10": "#3b9eff",
          "cyan-10": "#23afd0"
        }
      }
    },
    "accent": {
      "default": "blue",
      "contexts": {
        "blue": { "intent-primary-bg": "{blue-9}", "intent-primary-bg-hover": "{blue-10}" },
        "cyan": { "intent-primary-bg": "{cyan-9}", "intent-primary-bg-hover": "{cyan-10}" }
      }
    },
    "density": {
      "default": "regular",
      "contexts": { "regular": {}, "compact": { "space-3": 8, "leading-body": 18 } }
    }
  },
  "resolutionOrder": ["colorMode", "accent", "density"],
  "roles": {
    "primary": "intent-primary-bg",
    "primary-foreground": "intent-primary-fg",
    "background": "surface-app",
    "foreground": "text-primary",
    "card": "surface-card",
    "border": "border-subtle",
    "muted-foreground": "text-muted",
    "font-sans": "font-ui",
    "text-body": "type-body"
  }
}
```

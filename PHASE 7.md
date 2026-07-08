# PHASE 7

# PROFESSIONAL SCIENTIFIC WORKBENCH

ROLE

You are the Principal Rust Software Architect, Desktop UX Architect, Performance Engineer, and Scientific Visualization Engineer for Phylon.

The biological simulation is now mature.

STOP adding biological features.

STOP expanding the simulation.

The objective of this phase is to transform Phylon from a research prototype into a professional desktop application suitable for long-term scientific research.

Blender, Unreal Engine, Houdini, Godot, VS Code and ParaView should be treated as UX references only.

DO NOT clone Blender.

Build the best Computational Biology Research IDE.

------------------------------------------------------------

PRIMARY OBJECTIVE

This phase focuses entirely on

• Desktop UX
• Workbench
• Maintainability
• Performance
• Code Architecture
• Refactoring
• Design System
• Research Productivity

No biology.

No simulation expansion.

------------------------------------------------------------

STEP 1

Before changing anything

Audit the entire repository.

Audit

UI

UX

Rendering

Architecture

Performance

Large files

Dead code

Duplicate code

Large match statements

Large systems

Large renderers

Large UI files

Every TODO

Every FIXME

Every panel

Every dialog

Every menu

Every toolbar

Every context menu

Every keyboard shortcut

Every workspace

Every visible control

Nothing should be assumed.

Produce an implementation roadmap first.

------------------------------------------------------------

GOALS

1.

Complete Professional Workbench

Implement

Docking

Undocking

Split panels

Merge panels

Tabbed editors

Floating panels (if supported)

Panel pinning

Panel hiding

Panel restore

Workspace presets

Custom workspaces

Persistent layouts

Session restore

Research layouts

Teaching layouts

Presentation layouts

Debug layouts

Evolution layouts

Analytics layouts

Every workspace should expose only relevant tools.

------------------------------------------------------------

1.

Professional Design System

Audit

Typography

Spacing

Padding

Margins

Icons

Colors

Animations

Focus

Hover

Selection

Status indicators

Tooltips

Notifications

Dialogs

Empty states

Loading states

Everything must use semantic design tokens.

No magic numbers.

------------------------------------------------------------

1.

Functional Completeness

Every visible feature must work.

Audit

Buttons

Menus

Toolbar

Inspector

Charts

Panels

Dialogs

Shortcuts

Tree views

Docking

Workspace switching

Nothing should exist only visually.

If something cannot work

remove it until implemented.

------------------------------------------------------------

1.

Repository Modernization

Every file larger than roughly 500–700 lines must be reviewed.

Split only when it improves architecture.

Prefer

feature-oriented modules

single responsibility

clear ownership

high cohesion

low coupling

Avoid mechanical file splitting.

------------------------------------------------------------

1.

Rendering Cleanup

Separate

Viewport rendering

Overlay rendering

Selection

Highlight

Labels

Particles

Biological VFX

Inspector rendering

Charts

Debug rendering

No mixed responsibilities.

------------------------------------------------------------

1.

Performance

Profile before optimizing.

Measure

GPU

CPU

Memory

Allocations

ECS queries

Frame time

UI redraws

Rendering

Only optimize measured bottlenecks.

------------------------------------------------------------

1.

Research Productivity

Implement

Command Palette

Global Search

Quick Actions

Bookmarks

Recent Organisms

Recent Experiments

History

Undo

Redo

Workspace presets

Keyboard-first workflows

------------------------------------------------------------

IMPLEMENTATION STRATEGY

Phase A

Repository audit

↓

Phase B

Architecture report

↓

Phase C

Refactoring roadmap

↓

Phase D

Implementation

Implement one milestone only.

After every milestone

Summarize

Files changed

Performance impact

Verification

Remaining work

Then STOP.

Wait for approval.

------------------------------------------------------------

SUCCESS CRITERIA

The application should feel comparable in engineering quality to professional desktop software.

Researchers should comfortably use it for many hours.

The repository should be significantly easier to maintain.

No large God files.

No dead UI.

No duplicated rendering.

No inconsistent design.

Do NOT begin coding.

Produce the roadmap first.

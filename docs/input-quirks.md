# Paint.NET 5.1.12 input quirks

Updated 2026-10-18. These behaviors come from the supplied executable's managed
code, rather than assumptions about common image-editor shortcuts. They are
ported into `src/app/input.rs` and the canvas/workspace integration.

| Behavior | Frozen reference evidence |
| --- | --- |
| Tool keys cycle only within two seconds of the previous accepted tool key. A gap strictly greater than two seconds restarts the group. Changing key groups also starts at their first tool. | `Tool.OnKeyPress(KeyPressEventArgs)`, `lastToolSwitch`, `toolSwitchReset` |
| Shift reverses the complete tool order, including the initial choice: Shift+S starts at Magic Wand; Shift+O starts at Shapes. S cycles Rectangle, Lasso, Ellipse, Magic Wand; M cycles pixel/selection movement; O cycles Line/Curve and Shapes. | `Tool.OnKeyPress`, `DocumentWorkspace` static tool catalog |
| A tool key cannot interrupt a held stroke, but a matching key still refreshes the cycle timer. Tools marked to skip themselves are not reselected. | `Tool.OnKeyPress`, `IsPointerPressed`, `ToolInfo.SkipIfActiveOnHotKey` |
| C toggles which color the Colors controls edit; X swaps the two colors while preserving that choice. Shift+X also swaps. These keys do not intercept text input or Ctrl+C/Ctrl+X. | `Tool.OnTranslateToolConfigStripHotKey`, `ColorsForm.ToggleWhichUserColor`, `SwapUserColors`, `SetUserColors` |
| Brackets change brush width by one; Ctrl+brackets change it by five. Fractions survive, and the value clamps to 1–2,000. Brush, Line/Curve and Shapes share the setting, including toolbar edits and restored preferences. | `Tool.OnToolConfigStripHotKey`, `ToolConfigStrip.AddToBrushSize`, `AppSettings.ToolsSection.BrushSection` |
| Comma, period and slash cycle Line/Curve's start cap, dash and end cap. Caps follow numeric enum order: Flat, Arrow, Filled Arrow, Rounded, Flat. Dashes cycle Solid, Dash, Dot, Dash Dot, Dash Dot Dot. | `Tool.OnTranslateToolConfigStripHotKey`, `EnumSplitButton.OnButtonClick`, `LineCurveCap`, `DashStyle` |
| A and Shift+A cycle the built-in shape catalog forward and backward, wrapping at Rectangle/Heart. Changing options updates live content without adding separate undo entries. | `ShapesToolBase.OnToolConfigStripHotKey`, `ShapeManager.GetShapeInfos`, `PdnShapeFactory.GetShapeIDs` |
| Enter deselects an ordinary selection when no drawing/content operation owns it. Text, live Line/Curve and floating transforms retain their own Enter behavior. | `Tool.OnKeyPress`; existing tool-specific handlers |
| Space temporarily pans with a left drag; a middle-button drag pans without selecting the Pan tool. Pointer press/move/release positions are processed in native event order, so coalesced events retain their endpoints. Temporary panning changes neither document pixels nor undo history and stops on release. | `Tool.KeyDown`, `KeyUp`, `PointerPressed`, `PointerMoved`, `PointerReleased` |

On macOS the existing platform Command mapping also applies to bracket steps.
The Windows reference itself uses Control.

## Verification and remaining limits

Focused Rust tests cover the exact timeout boundary, reverse/wrapped groups,
ignored-stroke timer refresh, original option orders, fractional/clamped widths,
and temporary-pan capture/release. The isolated X11 `quirks` scope exercises the
actual native keyboard/pointer paths, live previews, text isolation and undo.

This does not certify all original input behavior. OS cursor movement and its
arrow-key acceleration, complete keyboard viewport navigation, custom shape
catalogs, all shape pen styles, native IME/tablet workflows, exact cursors,
per-monitor scaling and every mixed-button gesture remain open. Mid-stroke
brush-setting changes still require reference rasterization evidence. The
existing Pan tool's complete scrolling behavior is a separate parity gate.

The proprietary assemblies/decompilation remain in ignored `reference/` and are
not included in the Rust runtime or development packages.

# Emulation box: controls and organization

Reviewed 2026-10-21 against the owner's proposed equipment list. This is a
planning inventory, not hardware detection or evidence that these peripherals
have passed emulator acceptance on this Linux host.

## Use the equipment already owned first

Xbox Series pads cover ordinary gamepad mappings. The Switch pad is a spare;
the Hori Fighting Stick Alpha, G920 and HOTAS/pedals are candidates for arcade,
racing and flight profiles respectively. Choose one existing mouse for desktop
navigation and supported pointer/touch emulation. No additional controller is
required before testing those paths.

This does not establish complete gameplay coverage. Pressure-sensitive buttons,
motion/gyro, pointer tracking, unusual accessories, wheel force feedback and
per-game mappings need separate support and testing. A mouse can approximate
some pointer/touch input; it does not reproduce every original controller.
Confirm the Hori platform variant and HOTAS model, Linux device access, axes,
buttons and force feedback where applicable before calling a profile supported.

## Organization is optional convenience

- A dual charging dock can keep two pads ready. Check exact controller, battery
  pack and battery-door compatibility; docks are not interchangeable with every
  rechargeable pack. PowerA's [dual-station manual](https://www.powera.com/siteassets/user-manuals/xbx/xbox-charging/powera_dual_charging_station_xbox-manual_2020_1644275497.pdf)
  describes its included battery and charge-through door requirements.
- Wall mounts or a shelf clear the desk. Quantity depends on the actual pad
  count; neither mounts nor a dock are required for gameplay.
- Add a powered USB hub only after identifying a port/power problem. It does not
  automatically fix radio interference, driver issues or controller enumeration.
- Fixed ports and labels help troubleshooting but do not guarantee Player 1
  assignment. Record USB IDs with `lsusb` and inspect axes/buttons with an
  available controller tester; do not install extra tools merely for inventory.

Suggested arrangement: Player 1 pad and mouse at the desk, fight stick brought
out as needed, charging pads together, spare pads on a shelf/mounts, wheel and
HOTAS placed to suit their mounts and pedals.

## Buy for a demonstrated need

A preferred D-pad or six-button layout can justify one retro pad, but ergonomics
are personal. Do not prescribe both an M30 and an SN30 model without a reason.
SN30 Pro+ is a Bluetooth controller that can also be used over USB, rather than
an exclusively wired model ([manufacturer FAQ](https://support.8bitdo.com/faq/sn30-pro-plus.html)).
Specify the exact product/model before recommending a purchase; labels such as
“8BitDo N64 pad” do not establish compatibility with the intended Linux setup.

Real Wii motion/pointer input needs a compatible remote, a Bluetooth connection
path and an infrared source for pointing; a sensor bar alone is not a Bluetooth
adapter. Dolphin supports real/emulated remotes and distinct Bluetooth
passthrough configurations ([official controller guide](https://en.dolphin-emu.org/docs/guides/configuring-controllers/)).
Some games also need a Nunchuk or MotionPlus. A light gun is another optional,
separately validated path; mouse mapping is not equivalent to its aiming feel.

## Marquee follow-up: input profiles

These are planned requirements, not implemented capabilities:

1. Inventory connected device identity, model, connection mode and available
   axes/buttons. Distinguish identical pads with a user assignment; never rely
   exclusively on a transient device index or USB port.
2. Save profiles per emulator/adapter, system and device class: gamepad, arcade,
   wheel/pedals and HOTAS. Add per-game overrides for exceptional controls.
3. Provide an input test and explicit player assignment, dead zones/calibration,
   binding labels and a safe return/menu binding. Confirm before replacing an
   existing emulator configuration and preserve manual edits.
4. Handle unplug/reconnect and missing preferred devices with a visible fallback
   choice. Respect exclusive devices and avoid intercepting gameplay controls
   while Marquee is in the background.
5. Verify actual boot/input, multiplayer order, save/load, exit and focus return.
   Test force feedback and specialized input independently. Start with owned
   Xbox pads and existing PS1/PS2 adapters before adding hardware breadth.

This extends the controller/save acceptance work in [EMULATORS](EMULATORS.md).
It does not displace the direct-store implementation order in [ROADMAP](ROADMAP.md).

# theme — the drip's colour scheme, as data.
#
# A palette goes in, herdr's theme tokens come out. The palette is an attrset
# of colour name -> hex in exactly the shape nix-env/lib/palette.nix produces
# (the Catppuccin names, plus the semantic role aliases layered over them), so
# handing our real palette to a fleet host is one line:
#
#   services.herdr-drip.plugins.theme.palette =
#     inputs.nix-env.lib.${system}.palette;
#
# and a host with its own colours passes a different attrset rather than
# forking the config. Nothing below knows a Catppuccin from a Gruvbox: the
# token names are herdr's, the colour names are the palette's, and the mapping
# between them is the only opinion in this file.
#
# WHAT A PALETTE CANNOT SAY. herdr's two chrome planes accept `reset` — not a
# colour but the terminal's own background, i.e. transparency — and a palette
# has no way to express that: every value in one is a hue, and "no hue" is not
# a darker or lighter member of the set. So `transparentChrome` is a flag on
# `mkTheme` rather than a rung, and it is ON by default because transparent
# chrome is the drip's appearance. Before it existed this file could not emit
# the scheme its own config/herdr.toml carried, whatever palette it was handed,
# and every host reading the generated config painted an opaque panel and
# sidebar that the workstation did not have.
#
# WHY THE DEFAULT IS VENDORED. herdr-drip cannot take nix-env as a flake input:
# nix-env depends on nix-claude-drip, which depends on THIS repo, so the input
# would close a cycle. Our palette is nonetheless the DEFAULT — a host that
# sets nothing comes up in our colour scheme, the same way the curated
# config/herdr.toml is a default rather than a mandate — so the rungs `mkTheme`
# reads are copied below. Only those rungs, not the whole palette, and they are
# upstream Catppuccin Mocha values under upstream's own names, so a palette
# passed in overwrites every one of them by name.
let
  # Catppuccin Mocha. Same names as nix-env's palette, same values.
  mocha = {
    # Neutrals (dark -> light).
    mantle = "#181825";
    base = "#1E1E2E";
    surface0 = "#313244";
    surface1 = "#45475A";
    overlay0 = "#6C7086";
    overlay1 = "#7F849C";
    subtext0 = "#A6ADC8";
    text = "#CDD6F4";

    # Accents.
    mauve = "#CBA6F7";
    green = "#A6E3A1";
    yellow = "#F9E2AF";
    red = "#F38BA8";
    peach = "#FAB387";
    blue = "#89B4FA";
    teal = "#94E2D5";
    lavender = "#B4BEFE";
  };
in
{
  # The role alias matters as much as the rungs, and this one is the single
  # place the drip departs from the palette it otherwise follows: nix-env's
  # `accent` points at GREEN, and herdr's accent here is LAVENDER.
  #
  # The reason is that herdr spends `green` on a meaning of its own. It is the
  # done/idle mark on an agent row (see the state colours below), so an accent
  # of the same hue paints the active pane border, the selection and "this
  # agent has finished" in one colour — the two things you scan a sidebar for,
  # made indistinguishable. Lavender is the palette's own rung, so this is
  # still a re-point rather than a new colour; it costs the agreement with
  # zellij's green `ribbon_selected`, which is a real loss and a smaller one
  # than the collision.
  #
  # It is a DEFAULT, not a rule: a host passing a palette whose `accent` role
  # says otherwise gets that, exactly as before.
  defaultPalette = mocha // {
    accent = mocha.lavender;
  };

  # The herdr settings a palette implies: `[theme.custom]` plus the legacy
  # `ui.accent`. Returned as Nix values for the module to merge into
  # `settings`, so it lands in the generated config.toml like any other key.
  #
  # Every token here is one herdr 0.8.0 accepts — the set is closed
  # (src/config/theme.rs, `CustomThemeColors`), and `herdr config check`
  # reports anything outside it as an unknown key. Notably ABSENT, because
  # herdr has no slot for them: a second selection surface (Catppuccin's
  # surface2), the darkest neutral (crust), and the accents herdr never
  # names — rosewater, flamingo, pink, maroon, sky, sapphire, lavender. No
  # slot is not the same as unused: lavender has no token of its own and is
  # still what `accent` resolves to under the default palette.
  #
  # `transparentChrome` is the one input that is not a colour — see the note
  # at the top of this file for why it cannot be one.
  mkTheme =
    {
      palette,
      transparentChrome ? true,
    }:
    let
      # Role first, base name second. nix-env's palette carries both, so the
      # fallback is for a palette handed in as bare Catppuccin rungs with no
      # role layer — it still themes herdr, it just cannot re-point a role.
      pick = role: name: palette.${role} or palette.${name};

      accent = pick "accent" "green";

      # `reset` is herdr's spelling of "no colour here"; the parser takes it
      # wherever a token takes a hex (src/config/theme.rs), and it is what
      # herdr already ships `sidebar_bg` as.
      chrome = if transparentChrome then "reset" else pick "bg_alt" "mantle";
    in
    {
      theme.custom = {
        # Highlights and active borders. See defaultPalette above.
        inherit accent;

        # The two chrome planes. herdr's `panel_bg` is the tab bar, floating
        # panels, overlays and modals; `sidebar_bg` is the desktop sidebar,
        # and herdr's own default for it is `Reset` — the terminal's own
        # background — rather than a colour.
        #
        # The drip keeps both on `Reset`, which is `transparentChrome`. herdr
        # is not the outermost thing on the screen here: it sits inside zellij
        # inside a terminal that has its own background (and, on a
        # transparent one, whatever is behind that). Painting `mantle` across
        # the tab bar and the sidebar puts a near-black slab over it that
        # differs from the surrounding shell by a shade or two — the seam is
        # visible and buys nothing, whereas inheriting means there is no seam
        # to get wrong. A host that wants opaque chrome sets
        # `transparentChrome = false` and gets `bg_alt`, which is the shade
        # the zellij theme paints its own chrome with (`text_unselected` /
        # `list_unselected` background = mantle).
        panel_bg = chrome;
        sidebar_bg = chrome;

        # The surface ramp: separators, then the background of a selected or
        # focused row, then hover/active. herdr's names for the bottom rung
        # differ from Catppuccin's (`surface_dim` is `base`, one below
        # `surface0`), so what is preserved here is the ROLE of each rung, not
        # the spelling.
        #
        # `surface0` takes `bg_alt` (mantle) rather than `bg_surface`, and
        # that makes the ramp non-monotonic on purpose: with the chrome planes
        # transparent it is the only opaque plane the sidebar and the tab bar
        # have left, so it is a chrome shade doing chrome's job — the
        # unselected tab, the selected row — and must read as slightly DARKER
        # than the terminal behind it, not as the mid-grey slab `surface0`
        # (#313244) would put there. `surface_dim` stays on `bg`, so
        # separators sit a rung lighter than the plane they divide.
        surface_dim = pick "bg" "base";
        surface0 = pick "bg_alt" "mantle";
        surface1 = palette.surface1;

        # Text hierarchy: muted (numbers, secondary info), brighter muted,
        # subdued labels, and the main foreground.
        overlay0 = palette.overlay0;
        overlay1 = palette.overlay1;
        subtext0 = pick "secondary" "subtext0";
        text = pick "primary" "text";

        # Agent and notification states. These are name-for-name: herdr spells
        # its state colours in Catppuccin's vocabulary, and so does the
        # palette, so `green` means green in both. That also keeps an override
        # honest — re-tinting `green` in the palette moves herdr's done/idle
        # marks and zellij's `emphasis_2` together, which is the whole point of
        # a palette both read.
        #
        # It does mean our SEMANTIC roles are not consulted here: `success` is
        # teal in the palette while herdr's success colour is green, and
        # `branch` is lavender while herdr renders branch names with `mauve`.
        # Following the roles would leave herdr with two greens and no mauve.
        mauve = palette.mauve; # branch names, special labels
        green = palette.green; # done / idle
        yellow = palette.yellow; # working / running
        red = palette.red; # needs attention / blocked
        peach = palette.peach; # interrupted / warning
        blue = palette.blue; # unseen / done notifications
        teal = palette.teal; # notification accent, unseen markers
      };

      # The older spelling of the same colour. herdr consults `ui.accent` ONLY
      # while `theme.custom.accent` is unset (app/mod.rs, `legacy_accent`), so
      # this can never fight the token above — it is kept in step so that a
      # host reading either key sees one answer, and so that dropping the
      # custom block does not silently revert the accent to stock blue.
      ui.accent = accent;
    };
}

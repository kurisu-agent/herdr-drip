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
  };
in
{
  # The role alias matters as much as the rungs: nix-env's `accent` points at
  # green, and that is the single place this theme departs from herdr's stock
  # Catppuccin (whose accent is blue). It is also what makes herdr's active
  # borders agree with zellij's, where `ribbon_selected` and `frame_selected`
  # are green — the two multiplexers stack on one screen, and an accent that
  # disagreed would be the most visible seam between them.
  defaultPalette = mocha // {
    accent = mocha.green;
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
  # names — rosewater, flamingo, pink, maroon, sky, sapphire, lavender.
  mkTheme =
    palette:
    let
      # Role first, base name second. nix-env's palette carries both, so the
      # fallback is for a palette handed in as bare Catppuccin rungs with no
      # role layer — it still themes herdr, it just cannot re-point a role.
      pick = role: name: palette.${role} or palette.${name};

      accent = pick "accent" "green";
    in
    {
      theme.custom = {
        # Highlights and active borders. See defaultPalette above.
        inherit accent;

        # The two chrome planes. herdr's `panel_bg` is the tab bar, floating
        # panels, overlays and modals; `sidebar_bg` is the desktop sidebar and
        # ships as `Reset` (the terminal's own background) rather than a
        # colour. Both take `bg_alt` — one chrome plane, which is how the
        # zellij theme has it too (`text_unselected` / `list_unselected`
        # background = mantle), so the rail and the topbar above it are the
        # same shade instead of two near-misses.
        panel_bg = pick "bg_alt" "mantle";
        sidebar_bg = pick "bg_alt" "mantle";

        # The surface ramp, dim -> bright: separators, then the background of
        # a selected or focused row, then hover/active. herdr's names for the
        # bottom rung differ from Catppuccin's (`surface_dim` is `base`, one
        # below `surface0`), so the ordering is what is preserved here, not
        # the spelling.
        surface_dim = pick "bg" "base";
        surface0 = pick "bg_surface" "surface0";
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

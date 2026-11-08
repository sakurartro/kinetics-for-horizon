# "Horizon" in white concrete, 50 blocks the way the player looks (/function kinetics:horizon_text).
# Pixels are 2x2 blocks, 7 rows: 74 x 14 blocks. At 50 blocks with the default FOV 70 the screen spans ~70 blocks
# high and ~110 wide (16:10, a 13" laptop), so it fits with room around it. It stands upright, square to the nearest
# of the four directions (fill can't draw it slanted), its middle at eye height. The blocks stay until broken.
# The letters are horizon_text/letters, rows of fills.

# The floating-text version of before, if one is still there.
kill @e[type=minecraft:text_display,tag=kinetics.horizon_text]
# Block centres (align, then +.5), so turned local offsets never round to the next block.
execute if entity @s[y_rotation=-45..44.9] at @s rotated 0 0 positioned ^ ^1.6 ^50 align xyz positioned ~.5 ~.5 ~.5 run function kinetics:horizon_text/letters
execute if entity @s[y_rotation=45..134.9] at @s rotated 90 0 positioned ^ ^1.6 ^50 align xyz positioned ~.5 ~.5 ~.5 run function kinetics:horizon_text/letters
execute if entity @s[y_rotation=135..-135.1] at @s rotated 180 0 positioned ^ ^1.6 ^50 align xyz positioned ~.5 ~.5 ~.5 run function kinetics:horizon_text/letters
execute if entity @s[y_rotation=-135..-45.1] at @s rotated -90 0 positioned ^ ^1.6 ^50 align xyz positioned ~.5 ~.5 ~.5 run function kinetics:horizon_text/letters

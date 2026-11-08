# A drill cart that rides east from the player and digs through a stone wall (BUR.md phase 5).
# Run it standing on flat ground: it clears and floors ~2..~30 east, ~-2..~2 north-south.
# Flip the lever beside the first assembler to start; the cart stops and is put down on the last assembler.

kill @e[type=minecraft:marker,tag=kinetics.drill_tmp]
fill ~2 ~ ~-2 ~30 ~4 ~2 minecraft:air
fill ~2 ~-1 ~-2 ~30 ~-1 ~2 minecraft:smooth_stone

# Track: rails ~4..~26, powered rails on redstone blocks at ~6 and ~17 (an unpowered one would stop the cart),
# assemblers at both ends. contraption.rs cart_speed: -0.005 a rail, +0.06 a powered one, start 0.1.
setblock ~6 ~-1 ~ minecraft:redstone_block
setblock ~17 ~-1 ~ minecraft:redstone_block
fill ~4 ~ ~ ~26 ~ ~ minecraft:rail[shape=east_west]
setblock ~6 ~ ~ minecraft:powered_rail[shape=east_west,powered=true]
setblock ~17 ~ ~ minecraft:powered_rail[shape=east_west,powered=true]
setblock ~3 ~ ~ kinetics:cart_assembler[shape=east_west,powered=false]
setblock ~27 ~ ~ kinetics:cart_assembler[shape=east_west,powered=false]

# The wall over the rails: the drills cut a 1x2 hole, the rest stays as the tunnel.
fill ~10 ~1 ~-1 ~13 ~3 ~1 minecraft:stone

# The cart: the anchor is the block over the assembler, the drills in front face the way it goes (east).
setblock ~3 ~1 ~ kinetics:andesite_casing
setblock ~3 ~2 ~ kinetics:andesite_casing
setblock ~4 ~1 ~ kinetics:mechanical_drill[facing=east]
setblock ~4 ~2 ~ kinetics:mechanical_drill[facing=east]
setblock ~3 ~ ~1 minecraft:lever[face=floor,facing=south,powered=false]

# Glue ~3 ~1 ~ .. ~4 ~2 ~ : a kinetics.glue marker with absolute min/max (glue_use.rs). /data can't write a
# marker in Horizon, so the corners go through storage into a macro.
execute align xyz run summon minecraft:marker ~3 ~1 ~ {Tags:["kinetics.drill_tmp","kinetics.drill_min"]}
execute align xyz run summon minecraft:marker ~4 ~2 ~ {Tags:["kinetics.drill_tmp","kinetics.drill_max"]}
execute store result storage kinetics:drill_cart x int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_min,limit=1] Pos[0]
execute store result storage kinetics:drill_cart y int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_min,limit=1] Pos[1]
execute store result storage kinetics:drill_cart z int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_min,limit=1] Pos[2]
execute store result storage kinetics:drill_cart mx int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_max,limit=1] Pos[0]
execute store result storage kinetics:drill_cart my int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_max,limit=1] Pos[1]
execute store result storage kinetics:drill_cart mz int 1 run data get entity @e[type=minecraft:marker,tag=kinetics.drill_max,limit=1] Pos[2]
function kinetics:drill_cart/glue with storage kinetics:drill_cart
kill @e[type=minecraft:marker,tag=kinetics.drill_tmp]

tellraw @s "Drill cart built. Flip the lever by the first assembler."

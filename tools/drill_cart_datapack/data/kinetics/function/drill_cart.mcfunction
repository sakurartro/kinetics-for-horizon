# A drill cart as Create builds one (BUR.md phase 5): a minecart rolls over a powered cart assembler, takes what is
# glued above it and rides east, its drills cutting through a stone wall; an assembler with no signal puts it down.
# Run it standing on flat ground: it clears and floors ~1..~30 east, ~-2..~2 north-south.
# Flip the lever by the minecart to start it.

kill @e[type=minecraft:marker,tag=kinetics.drill_tmp]
fill ~1 ~ ~-2 ~30 ~4 ~2 minecraft:air
fill ~1 ~-1 ~-2 ~30 ~-1 ~2 minecraft:smooth_stone

# Track: a powered rail with a block behind it (the lever kicks the cart off it, as vanilla does), the assembler on a
# redstone block (powered: it mounts the drill and sends the cart off at full speed), rails with two powered boosters,
# and an assembler with no signal at the end. Both arrows point east (backwards=true), the way the drills face.
setblock ~1 ~ ~ minecraft:stone
setblock ~3 ~-1 ~ minecraft:redstone_block
setblock ~7 ~-1 ~ minecraft:redstone_block
setblock ~18 ~-1 ~ minecraft:redstone_block
fill ~4 ~ ~ ~26 ~ ~ minecraft:rail[shape=east_west]
setblock ~2 ~ ~ minecraft:powered_rail[shape=east_west,powered=false]
setblock ~2 ~ ~1 minecraft:lever[face=floor,facing=south,powered=false]
setblock ~7 ~ ~ minecraft:powered_rail[shape=east_west,powered=true]
setblock ~18 ~ ~ minecraft:powered_rail[shape=east_west,powered=true]
setblock ~3 ~ ~ kinetics:cart_assembler[shape=east_west,backwards=true,powered=true]
setblock ~27 ~ ~ kinetics:cart_assembler[shape=east_west,backwards=true,powered=false]

# The wall over the rails: the drills cut a 1x2 hole, the rest stays as the tunnel.
fill ~10 ~1 ~-1 ~13 ~3 ~1 minecraft:stone

# What the cart takes: the block over the assembler and what is glued to it, the drills in front facing east.
setblock ~3 ~1 ~ kinetics:andesite_casing
setblock ~3 ~2 ~ kinetics:andesite_casing
setblock ~4 ~1 ~ kinetics:mechanical_drill[facing=east]
setblock ~4 ~2 ~ kinetics:mechanical_drill[facing=east]
execute align xyz run summon minecraft:minecart ~2.5 ~ ~0.5

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

tellraw @s "Drill cart built. Flip the lever by the minecart."

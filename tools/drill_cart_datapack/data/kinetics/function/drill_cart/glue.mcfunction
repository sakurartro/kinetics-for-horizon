$summon minecraft:marker $(x) $(y) $(z) {Tags:["kinetics.glue"],data:{min:[I;$(x),$(y),$(z)],max:[I;$(mx),$(my),$(mz)]}}
$tellraw @s "glued $(x) $(y) $(z) .. $(mx) $(my) $(mz)"

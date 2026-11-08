# A big "Horizon" 50 blocks the way the player looks (/function kinetics:horizon_text); running it again moves it.
# Scale 60: "Horizon" is 38 font pixels wide, 1/40 block each, so ~57 x 14 blocks. At 50 blocks with the default
# FOV 70 the screen spans ~70 blocks high and ~110 wide (16:10, a 13" laptop), so it fits with room around it.
# It always faces the camera (billboard), lit at full brightness, with no background box.

kill @e[type=minecraft:text_display,tag=kinetics.horizon_text]
execute at @s anchored eyes positioned ^ ^ ^50 run summon minecraft:text_display ~ ~-7 ~ {Tags:["kinetics.horizon_text"],text:"Horizon",billboard:"center",background:0,shadow:1b,view_range:2f,brightness:{sky:15,block:15},transformation:{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],translation:[0f,0f,0f],scale:[60f,60f,60f]}}

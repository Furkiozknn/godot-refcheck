extends Node2D

func spawn(name: String) -> void:
	var packed := load("res://levels/%s.tscn" % name)
	var other := load("res://levels/" + name + ".tscn")
	var third := load("res://art/tile.png")
	print(packed, other, third)
	# load("res://dead/never.tscn")

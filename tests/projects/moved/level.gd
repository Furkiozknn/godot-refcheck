extends Node2D

const PLAYER := preload("res://scenes/player.tscn")

func _ready() -> void:
	add_child(PLAYER.instantiate())

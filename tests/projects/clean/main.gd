extends Node2D

const PANEL := preload("res://ui/panel.tscn")
const ICON := preload("uid://ciconaaaaaaaaa")

func _ready() -> void:
	# preload("res://not_a_real_file.tscn") is only a comment
	var text := "preload(\"res://also_not_real.tscn\")"
	print(text, PANEL, ICON)
	var locale := "en"
	var extra = load("res://translations/text.%s.translation" % locale)
	print(extra)
